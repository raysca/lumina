#import <Foundation/Foundation.h>
#import <AppKit/AppKit.h>
#import <AVFoundation/AVFoundation.h>
#import <CoreMedia/CoreMedia.h>
#import <CoreVideo/CoreVideo.h>
#include <stdlib.h>
#include <string.h>

@interface LuminaPlayerBridge : NSObject
@property (nonatomic, strong) AVPlayer *player;
@property (nonatomic, strong) AVPlayerItem *playerItem;
@property (nonatomic, strong) AVPlayerItemVideoOutput *videoOutput;
@property (nonatomic, assign) BOOL isLooping;
@property (nonatomic, assign) double duration;
@property (nonatomic, assign) int naturalWidth;
@property (nonatomic, assign) int naturalHeight;
@property (nonatomic, assign) int scaledWidth;
@property (nonatomic, assign) int scaledHeight;
@property (nonatomic, assign) float targetRate;
@property (nonatomic, assign) BOOL isPlayingState;
@property (nonatomic, assign) float volume;
@property (nonatomic, assign) BOOL isMuted;
/// Alternative audio tracks of the loaded file; nil when it offers none.
@property (nonatomic, strong) AVMediaSelectionGroup *audioGroup;
/// Chapter starts (NSNumber seconds) and titles, read once per load.
@property (nonatomic, strong) NSArray<NSNumber *> *chapterStarts;
@property (nonatomic, strong) NSArray<NSString *> *chapterTitles;
@end

@implementation LuminaPlayerBridge {
    float _volume;
    BOOL _isMuted;
    /// Tokens of the end-of-item and failed-to-play-to-end observers,
    /// removed in teardown.
    id _endObserver;
    id _failedObserver;
    /// A frame too big for the caller's buffer, kept for the retry with a
    /// bigger one so it is not lost.
    CVPixelBufferRef _heldFrame;
    /// The mid-clip playback error not yet taken by the app. Written by the
    /// notification on AVFoundation's thread, read on the UI thread.
    NSString *_playbackError;
    /// The item's failed status was already reported, so it is not again.
    BOOL _statusErrorTaken;
}

- (instancetype)init {
    self = [super init];
    if (self) {
        _isLooping = YES;
        _targetRate = 1.0f;
        _isPlayingState = NO;
        _volume = 1.0f;
        _isMuted = NO;
        _duration = 0.0;
        _naturalWidth = 0;
        _naturalHeight = 0;
        _scaledWidth = 0;
        _scaledHeight = 0;
    }
    return self;
}

- (void)dealloc {
    [self teardown];
}

- (void)teardown {
    if (_endObserver) {
        [[NSNotificationCenter defaultCenter] removeObserver:_endObserver];
        _endObserver = nil;
    }
    if (_heldFrame) {
        CVPixelBufferRelease(_heldFrame);
        _heldFrame = NULL;
    }
    if (_failedObserver) {
        [[NSNotificationCenter defaultCenter] removeObserver:_failedObserver];
        _failedObserver = nil;
    }
    @synchronized (self) {
        _playbackError = nil;
    }
    _statusErrorTaken = NO;
    if (_player) {
        [_player pause];
        _player = nil;
    }
    _playerItem = nil;
    _videoOutput = nil;
    _isPlayingState = NO;
    _audioGroup = nil;
    _chapterStarts = nil;
    _chapterTitles = nil;
}

/// Loads `path` and waits up to `waitSecs` for the item to be ready to play.
/// Runs on a background thread: AVFoundation reports the item status through
/// the main queue, so a wait on the main thread could never finish (it is
/// skipped there, as with `waitSecs` 0). On failure `*outError` describes the
/// cause for the log.
- (BOOL)loadFile:(NSString *)path
        waitSecs:(double)waitSecs
       duration:(double *)outDuration
          width:(int *)outWidth
         height:(int *)outHeight
           error:(NSString **)outError {
    [self teardown];

    if (!path || path.length == 0) {
        return NO;
    }
    if (![[NSFileManager defaultManager] fileExistsAtPath:path]) {
        return NO;
    }

    NSURL *url = [NSURL fileURLWithPath:path];
    if (!url) {
        return NO;
    }

    AVURLAsset *asset = [AVURLAsset URLAssetWithURL:url options:nil];
    if (!asset) {
        return NO;
    }

    // Verify video track exists
    NSArray<AVAssetTrack *> *tracks = [asset tracksWithMediaType:AVMediaTypeVideo];
    if (tracks.count == 0) {
        return NO;
    }

    // Retrieve asset duration
    CMTime dur = [asset duration];
    if (!CMTIME_IS_VALID(dur) || dur.timescale <= 0) {
        return NO;
    }
    _duration = CMTimeGetSeconds(dur);
    if (_duration <= 0.0) {
        return NO;
    }

    // Retrieve video dimensions and handle track rotation (e.g. portrait iPhone videos)
    AVVideoComposition *composition = [AVVideoComposition videoCompositionWithPropertiesOfAsset:asset];
    if (composition) {
        _naturalWidth = (int)composition.renderSize.width;
        _naturalHeight = (int)composition.renderSize.height;
    } else {
        AVAssetTrack *track = tracks[0];
        CGSize sz = [track naturalSize];
        CGAffineTransform t = [track preferredTransform];
        if (t.b != 0.0 || t.c != 0.0) {
            _naturalWidth = (int)sz.height;
            _naturalHeight = (int)sz.width;
        } else {
            _naturalWidth = (int)sz.width;
            _naturalHeight = (int)sz.height;
        }
    }

    // Cap output frame dimensions to 1080p to prevent memory exhaustion and texture atlas blowup
    int targetW = _naturalWidth;
    int targetH = _naturalHeight;
    const int MAX_W = 1920;
    const int MAX_H = 1080;
    if (targetW > MAX_W || targetH > MAX_H) {
        float scale = fminf((float)MAX_W / targetW, (float)MAX_H / targetH);
        targetW = (int)roundf(targetW * scale);
        targetH = (int)roundf(targetH * scale);
    }
    targetW = (targetW + 1) & ~1;
    targetH = (targetH + 1) & ~1;
    _scaledWidth = targetW;
    _scaledHeight = targetH;

    NSDictionary *pixAttrs = @{
        (id)kCVPixelBufferPixelFormatTypeKey: @(kCVPixelFormatType_32BGRA),
        (id)kCVPixelBufferWidthKey: @(_scaledWidth),
        (id)kCVPixelBufferHeightKey: @(_scaledHeight)
    };
    _videoOutput = [[AVPlayerItemVideoOutput alloc] initWithPixelBufferAttributes:pixAttrs];

    _playerItem = [AVPlayerItem playerItemWithAsset:asset];
    if (composition) {
        _playerItem.videoComposition = composition;
    }
    [_playerItem addOutput:_videoOutput];

    // The UI thread (the main thread) drives the player; the loop seek and
    // the playing state must not race it, so the end is handled there.
    __weak LuminaPlayerBridge *weakSelf = self;
    _endObserver = [[NSNotificationCenter defaultCenter]
        addObserverForName:AVPlayerItemDidPlayToEndTimeNotification
                    object:_playerItem
                     queue:[NSOperationQueue mainQueue]
                usingBlock:^(NSNotification *__unused note) {
                    [weakSelf onItemEnd];
                }];

    _failedObserver = [[NSNotificationCenter defaultCenter]
        addObserverForName:AVPlayerItemFailedToPlayToEndTimeNotification
                    object:_playerItem
                     queue:nil
                usingBlock:^(NSNotification *note) {
                    NSError *err = note.userInfo[AVPlayerItemFailedToPlayToEndTimeErrorKey];
                    [weakSelf notePlaybackError:err];
                }];

    _player = [AVPlayer playerWithPlayerItem:_playerItem];
    _player.actionAtItemEnd = AVPlayerActionAtItemEndNone;

    // The asset reads above can succeed for a file the item still cannot
    // play (an unsupported container, a damaged index): wait for its verdict.
    if (waitSecs > 0 && ![NSThread isMainThread]) {
        NSDate *deadline = [NSDate dateWithTimeIntervalSinceNow:waitSecs];
        while (_playerItem.status == AVPlayerItemStatusUnknown) {
            if (deadline.timeIntervalSinceNow <= 0) {
                if (outError) *outError = @"the player item was not ready in time";
                [self teardown];
                return NO;
            }
            usleep(5000);
        }
    }
    if (_playerItem.status == AVPlayerItemStatusFailed) {
        if (outError) *outError = _playerItem.error.description ?: @"the player item failed";
        [self teardown];
        return NO;
    }

    // Synchronous reads, like duration and tracks above: the asset is local.
    _audioGroup = [asset mediaSelectionGroupForMediaCharacteristic:AVMediaCharacteristicAudible];
    [self readChaptersOfAsset:asset];
    _player.volume = _volume;
    _player.muted = _isMuted;

    if (outDuration) *outDuration = _duration;
    if (outWidth) *outWidth = _scaledWidth;
    if (outHeight) *outHeight = _scaledHeight;

    return YES;
}

- (void)readChaptersOfAsset:(AVAsset *)asset {
    NSMutableArray<NSNumber *> *starts = [NSMutableArray array];
    NSMutableArray<NSString *> *titles = [NSMutableArray array];
    NSArray<AVTimedMetadataGroup *> *groups =
        [asset chapterMetadataGroupsBestMatchingPreferredLanguages:[NSLocale preferredLanguages]];
    for (AVTimedMetadataGroup *group in groups) {
        CMTime start = group.timeRange.start;
        if (!CMTIME_IS_NUMERIC(start)) continue;
        NSArray<AVMetadataItem *> *items =
            [AVMetadataItem metadataItemsFromArray:group.items
                              filteredByIdentifier:AVMetadataCommonIdentifierTitle];
        NSString *title = items.firstObject.stringValue ?: @"";
        [starts addObject:@(CMTimeGetSeconds(start))];
        [titles addObject:title];
    }
    _chapterStarts = starts;
    _chapterTitles = titles;
}

- (void)notePlaybackError:(NSError *)err {
    @synchronized (self) {
        _playbackError = err.description ?: @"playback failed";
    }
}

/// A playback error after a successful load, once: the failed-to-play-to-end
/// notification, or the item's failed status. Nil when there is none.
- (NSString *)takePlaybackError {
    NSString *taken = nil;
    @synchronized (self) {
        taken = _playbackError;
        _playbackError = nil;
    }
    if (!taken && !_statusErrorTaken && _playerItem.status == AVPlayerItemStatusFailed) {
        _statusErrorTaken = YES;
        taken = _playerItem.error.description ?: @"the player item failed";
    }
    return taken;
}

- (NSInteger)selectedAudioIndex {
    if (!_audioGroup || !_playerItem) return -1;
    AVMediaSelectionOption *option =
        [_playerItem.currentMediaSelection selectedMediaOptionInMediaSelectionGroup:_audioGroup];
    if (!option) return -1;
    NSUInteger index = [_audioGroup.options indexOfObject:option];
    return index == NSNotFound ? -1 : (NSInteger)index;
}

- (void)selectAudioIndex:(NSInteger)index {
    if (!_audioGroup || !_playerItem) return;
    if (index < 0 || (NSUInteger)index >= _audioGroup.options.count) return;
    [_playerItem selectMediaOption:_audioGroup.options[index] inMediaSelectionGroup:_audioGroup];
}

- (void)onItemEnd {
    if (_isLooping && _player) {
        [_player seekToTime:kCMTimeZero toleranceBefore:kCMTimeZero toleranceAfter:kCMTimeZero];
        if (_isPlayingState) {
            [_player play];
            _player.rate = _targetRate;
        }
    } else {
        _isPlayingState = NO;
        if (_player) {
            [_player pause];
        }
    }
}

- (void)play {
    if (_player) {
        _isPlayingState = YES;
        [_player play];
        _player.rate = _targetRate;
    }
}

- (void)pause {
    if (_player) {
        _isPlayingState = NO;
        [_player pause];
    }
}

- (BOOL)isPlaying {
    if (!_player) return NO;
    return _player.rate > 0.0f;
}

- (void)seekTo:(double)seconds {
    if (!_player) return;
    CMTime t = CMTimeMakeWithSeconds(seconds, 60000);
    [_player seekToTime:t toleranceBefore:kCMTimeZero toleranceAfter:kCMTimeZero];
}

- (void)stepFrames:(NSInteger)frames {
    if (!_player || !_player.currentItem) return;
    [_player pause];
    [_player.currentItem stepByCount:frames];
}

- (double)currentTime {
    if (!_player) return 0.0;
    CMTime t = [_player currentTime];
    if (CMTIME_IS_VALID(t) && t.timescale > 0) {
        return CMTimeGetSeconds(t);
    }
    return 0.0;
}

- (void)setPlaybackRate:(float)rate {
    _targetRate = rate;
    if (_player && _isPlayingState) {
        _player.rate = rate;
    }
}

- (float)playbackRate {
    return _targetRate;
}

- (void)setVolume:(float)vol {
    _volume = vol;
    if (_player) {
        _player.volume = vol;
    }
}

- (float)volume {
    return _player ? _player.volume : _volume;
}

- (void)setMuted:(BOOL)m {
    _isMuted = m;
    if (_player) {
        _player.muted = m;
    }
}

- (BOOL)isMuted {
    return _player ? _player.isMuted : _isMuted;
}

/// Whether a frame newer than the last one copied is ready (cheap: no copy).
- (BOOL)hasNewFrame {
    if (_heldFrame) return YES;
    if (!_videoOutput || !_playerItem) return NO;
    return [_videoOutput hasNewPixelBufferForItemTime:[_playerItem currentTime]];
}

/// Copies the newest frame, tightly packed BGRA, into `dst` and reports its
/// size. 1: copied. 0: no new frame. -1: `capacity` is too small for
/// `*outWidth` x `*outHeight`; the frame is kept for a retry with a bigger
/// buffer.
- (int)copyFrameInto:(uint8_t *)dst
            capacity:(size_t)capacity
               width:(int *)outWidth
              height:(int *)outHeight {
    CVPixelBufferRef buf = _heldFrame;
    _heldFrame = NULL;
    if (!buf) {
        if (!_videoOutput || !_playerItem) return 0;
        CMTime itemTime = [_playerItem currentTime];
        // Only hand back a frame AVFoundation has not vended yet. Without
        // this the caller re-uploads the same picture on every poll, and
        // after a seek it could take the pre-seek frame as the destination.
        if (![_videoOutput hasNewPixelBufferForItemTime:itemTime]) return 0;
        buf = [_videoOutput copyPixelBufferForItemTime:itemTime itemTimeForDisplay:NULL];
        if (!buf) return 0;
    }
    size_t w = CVPixelBufferGetWidth(buf);
    size_t h = CVPixelBufferGetHeight(buf);
    *outWidth = (int)w;
    *outHeight = (int)h;
    size_t tightStride = w * 4;
    if (w == 0 || h == 0) {
        CVPixelBufferRelease(buf);
        return 0;
    }
    if (!dst || capacity < tightStride * h) {
        _heldFrame = buf;
        return -1;
    }
    if (CVPixelBufferLockBaseAddress(buf, kCVPixelBufferLock_ReadOnly) != kCVReturnSuccess) {
        CVPixelBufferRelease(buf);
        return 0;
    }
    size_t stride = CVPixelBufferGetBytesPerRow(buf);
    uint8_t *src = (uint8_t *)CVPixelBufferGetBaseAddress(buf);
    int copied = 0;
    if (src) {
        if (stride == tightStride) {
            memcpy(dst, src, tightStride * h);
        } else {
            for (size_t row = 0; row < h; row++) {
                memcpy(dst + row * tightStride, src + row * stride, tightStride);
            }
        }
        copied = 1;
    }
    CVPixelBufferUnlockBaseAddress(buf, kCVPixelBufferLock_ReadOnly);
    CVPixelBufferRelease(buf);
    return copied;
}

@end

// C FFI Interface
void* lumina_player_create(void) {
    @autoreleasepool {
        LuminaPlayerBridge *bridge = [[LuminaPlayerBridge alloc] init];
        return (__bridge_retained void *)bridge;
    }
}

void lumina_player_destroy(void* player) {
    if (!player) return;
    @autoreleasepool {
        LuminaPlayerBridge *bridge = (__bridge_transfer LuminaPlayerBridge *)player;
        [bridge teardown];
        // bridge will be deallocated at end of autoreleasepool
    }
}

static char *lumina_copy_utf8(NSString *text);

/// `*out_error`, when set, is a malloc'd description for the log; Rust frees
/// it with lumina_player_free_string.
int lumina_player_load(void* player, const char* path, double wait_secs, double* out_duration,
                       int* out_width, int* out_height, char** out_error) {
    if (out_error) *out_error = NULL;
    if (!player || !path || strlen(path) == 0) return 0;
    @autoreleasepool {
        LuminaPlayerBridge *bridge = (__bridge LuminaPlayerBridge *)player;
        NSString *str = [NSString stringWithUTF8String:path];
        NSString *error = nil;
        BOOL ok = str && [bridge loadFile:str
                                 waitSecs:wait_secs
                                 duration:out_duration
                                    width:out_width
                                   height:out_height
                                    error:&error];
        if (!ok && error && out_error) *out_error = lumina_copy_utf8(error);
        return ok ? 1 : 0;
    }
}

/// The playback error not yet taken, malloc'd, or NULL when there is none.
char* lumina_player_take_error(void* player) {
    if (!player) return NULL;
    @autoreleasepool {
        LuminaPlayerBridge *bridge = (__bridge LuminaPlayerBridge *)player;
        NSString *error = [bridge takePlaybackError];
        return error ? lumina_copy_utf8(error) : NULL;
    }
}

void lumina_player_play(void* player) {
    if (!player) return;
    @autoreleasepool {
        LuminaPlayerBridge *bridge = (__bridge LuminaPlayerBridge *)player;
        [bridge play];
    }
}

void lumina_player_pause(void* player) {
    if (!player) return;
    @autoreleasepool {
        LuminaPlayerBridge *bridge = (__bridge LuminaPlayerBridge *)player;
        [bridge pause];
    }
}

int lumina_player_is_playing(void* player) {
    if (!player) return 0;
    @autoreleasepool {
        LuminaPlayerBridge *bridge = (__bridge LuminaPlayerBridge *)player;
        return [bridge isPlaying] ? 1 : 0;
    }
}

void lumina_player_seek(void* player, double seconds) {
    if (!player) return;
    @autoreleasepool {
        LuminaPlayerBridge *bridge = (__bridge LuminaPlayerBridge *)player;
        [bridge seekTo:seconds];
    }
}

void lumina_player_step(void* player, int frames) {
    if (!player) return;
    @autoreleasepool {
        LuminaPlayerBridge *bridge = (__bridge LuminaPlayerBridge *)player;
        [bridge stepFrames:frames];
    }
}

double lumina_player_get_position(void* player) {
    if (!player) return 0.0;
    @autoreleasepool {
        LuminaPlayerBridge *bridge = (__bridge LuminaPlayerBridge *)player;
        return [bridge currentTime];
    }
}

void lumina_player_set_rate(void* player, float rate) {
    if (!player) return;
    @autoreleasepool {
        LuminaPlayerBridge *bridge = (__bridge LuminaPlayerBridge *)player;
        [bridge setPlaybackRate:rate];
    }
}

float lumina_player_get_rate(void* player) {
    if (!player) return 1.0f;
    @autoreleasepool {
        LuminaPlayerBridge *bridge = (__bridge LuminaPlayerBridge *)player;
        return [bridge playbackRate];
    }
}

void lumina_player_set_volume(void* player, float volume) {
    if (!player) return;
    @autoreleasepool {
        LuminaPlayerBridge *bridge = (__bridge LuminaPlayerBridge *)player;
        [bridge setVolume:volume];
    }
}

float lumina_player_get_volume(void* player) {
    if (!player) return 1.0f;
    @autoreleasepool {
        LuminaPlayerBridge *bridge = (__bridge LuminaPlayerBridge *)player;
        return [bridge volume];
    }
}

void lumina_player_set_muted(void* player, int muted) {
    if (!player) return;
    @autoreleasepool {
        LuminaPlayerBridge *bridge = (__bridge LuminaPlayerBridge *)player;
        [bridge setMuted:(muted != 0)];
    }
}

int lumina_player_is_muted(void* player) {
    if (!player) return 0;
    @autoreleasepool {
        LuminaPlayerBridge *bridge = (__bridge LuminaPlayerBridge *)player;
        return [bridge isMuted] ? 1 : 0;
    }
}

void lumina_player_set_looping(void* player, int looping) {
    if (!player) return;
    @autoreleasepool {
        LuminaPlayerBridge *bridge = (__bridge LuminaPlayerBridge *)player;
        bridge.isLooping = (looping != 0);
    }
}

int lumina_player_is_looping(void* player) {
    if (!player) return 0;
    @autoreleasepool {
        LuminaPlayerBridge *bridge = (__bridge LuminaPlayerBridge *)player;
        return bridge.isLooping ? 1 : 0;
    }
}

int lumina_player_has_new_frame(void* player) {
    if (!player) return 0;
    @autoreleasepool {
        LuminaPlayerBridge *bridge = (__bridge LuminaPlayerBridge *)player;
        return [bridge hasNewFrame] ? 1 : 0;
    }
}

int lumina_player_copy_frame(void* player, uint8_t* dst, size_t capacity, int* out_width, int* out_height) {
    if (!player || !out_width || !out_height) return 0;
    @autoreleasepool {
        LuminaPlayerBridge *bridge = (__bridge LuminaPlayerBridge *)player;
        return [bridge copyFrameInto:dst capacity:capacity width:out_width height:out_height];
    }
}

// Strings below are malloc'd UTF-8; Rust frees them with lumina_player_free_string.
static char *lumina_copy_utf8(NSString *text) {
    const char *utf8 = (text ?: @"").UTF8String;
    return strdup(utf8 ? utf8 : "");
}

int lumina_player_audio_track_count(void* player) {
    if (!player) return 0;
    @autoreleasepool {
        LuminaPlayerBridge *bridge = (__bridge LuminaPlayerBridge *)player;
        return (int)bridge.audioGroup.options.count;
    }
}

char* lumina_player_audio_track_name(void* player, int index) {
    if (!player || index < 0) return NULL;
    @autoreleasepool {
        LuminaPlayerBridge *bridge = (__bridge LuminaPlayerBridge *)player;
        NSArray<AVMediaSelectionOption *> *options = bridge.audioGroup.options;
        if ((NSUInteger)index >= options.count) return NULL;
        return lumina_copy_utf8(options[index].displayName);
    }
}

int lumina_player_selected_audio_track(void* player) {
    if (!player) return -1;
    @autoreleasepool {
        LuminaPlayerBridge *bridge = (__bridge LuminaPlayerBridge *)player;
        return (int)[bridge selectedAudioIndex];
    }
}

void lumina_player_select_audio_track(void* player, int index) {
    if (!player) return;
    @autoreleasepool {
        LuminaPlayerBridge *bridge = (__bridge LuminaPlayerBridge *)player;
        [bridge selectAudioIndex:index];
    }
}

int lumina_player_chapter_count(void* player) {
    if (!player) return 0;
    @autoreleasepool {
        LuminaPlayerBridge *bridge = (__bridge LuminaPlayerBridge *)player;
        return (int)bridge.chapterStarts.count;
    }
}

double lumina_player_chapter_start(void* player, int index) {
    if (!player || index < 0) return 0.0;
    @autoreleasepool {
        LuminaPlayerBridge *bridge = (__bridge LuminaPlayerBridge *)player;
        if ((NSUInteger)index >= bridge.chapterStarts.count) return 0.0;
        return bridge.chapterStarts[index].doubleValue;
    }
}

char* lumina_player_chapter_title(void* player, int index) {
    if (!player || index < 0) return NULL;
    @autoreleasepool {
        LuminaPlayerBridge *bridge = (__bridge LuminaPlayerBridge *)player;
        if ((NSUInteger)index >= bridge.chapterTitles.count) return NULL;
        return lumina_copy_utf8(bridge.chapterTitles[index]);
    }
}

void lumina_player_free_string(char* s) {
    free(s);
}

// Read once at startup by motion::reduce_motion().
int lumina_reduce_motion(void) {
    @autoreleasepool {
        return [[NSWorkspace sharedWorkspace] accessibilityDisplayShouldReduceMotion] ? 1 : 0;
    }
}

/// GPUI opens floating windows as NSPanels, which AppKit hides whenever the
/// app is inactive. A PiP must stay up while the user works in another app,
/// so turn that off, and show the panel if it opened while Lumina was
/// inactive (it was ordered front already, then hidden straight away).
/// The card draws its own Close and Expand, so the window buttons are hidden.
void lumina_window_set_up_pip(void *ns_view) {
    NSWindow *window = [(__bridge NSView *)ns_view window];
    if (window == nil) {
        return;
    }
    NSWindowButton kinds[] = {NSWindowCloseButton, NSWindowMiniaturizeButton, NSWindowZoomButton};
    for (size_t i = 0; i < sizeof kinds / sizeof *kinds; i++) {
        [[window standardWindowButton:kinds[i]] setHidden:YES];
    }
    window.hidesOnDeactivate = NO;
    if (!window.isVisible) {
        [window orderFront:nil];
    }
}
