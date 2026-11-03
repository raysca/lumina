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
@end

@implementation LuminaPlayerBridge {
    float _volume;
    BOOL _isMuted;
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
    if (_playerItem) {
        [[NSNotificationCenter defaultCenter] removeObserver:self
                                                         name:AVPlayerItemDidPlayToEndTimeNotification
                                                       object:_playerItem];
    }
    if (_player) {
        [_player pause];
        _player = nil;
    }
    _playerItem = nil;
    _videoOutput = nil;
    _isPlayingState = NO;
}

- (BOOL)loadFile:(NSString *)path
       duration:(double *)outDuration
          width:(int *)outWidth
         height:(int *)outHeight {
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

    [[NSNotificationCenter defaultCenter] addObserver:self
                                             selector:@selector(onItemEnd:)
                                                 name:AVPlayerItemDidPlayToEndTimeNotification
                                               object:_playerItem];

    _player = [AVPlayer playerWithPlayerItem:_playerItem];
    _player.actionAtItemEnd = AVPlayerActionAtItemEndNone;
    _player.volume = _volume;
    _player.muted = _isMuted;

    if (outDuration) *outDuration = _duration;
    if (outWidth) *outWidth = _scaledWidth;
    if (outHeight) *outHeight = _scaledHeight;

    return YES;
}

- (void)onItemEnd:(NSNotification *)notif {
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

- (BOOL)copyFrameIntoWidth:(int *)outWidth
                    height:(int *)outHeight
                      data:(uint8_t **)outData
                       len:(int *)outLen {
    if (!_videoOutput || !_playerItem) {
        return NO;
    }

    CMTime itemTime = [_playerItem currentTime];
    // Only hand back a frame AVFoundation has not vended yet. Without this the
    // caller re-uploads the same picture on every poll, and after a seek it
    // could take the pre-seek frame as the destination.
    if (![_videoOutput hasNewPixelBufferForItemTime:itemTime]) {
        return NO;
    }
    CVPixelBufferRef buf = [_videoOutput copyPixelBufferForItemTime:itemTime itemTimeForDisplay:NULL];
    if (!buf) {
        return NO;
    }

    CVReturn lockErr = CVPixelBufferLockBaseAddress(buf, kCVPixelBufferLock_ReadOnly);
    if (lockErr != kCVReturnSuccess) {
        CVPixelBufferRelease(buf);
        return NO;
    }

    size_t w = CVPixelBufferGetWidth(buf);
    size_t h = CVPixelBufferGetHeight(buf);
    size_t stride = CVPixelBufferGetBytesPerRow(buf);
    uint8_t *src = (uint8_t *)CVPixelBufferGetBaseAddress(buf);

    if (src && w > 0 && h > 0) {
        size_t tightStride = w * 4;
        size_t totalBytes = tightStride * h;
        uint8_t *dst = (uint8_t *)malloc(totalBytes);
        if (dst) {
            if (stride == tightStride) {
                memcpy(dst, src, totalBytes);
            } else {
                for (size_t row = 0; row < h; row++) {
                    memcpy(dst + row * tightStride, src + row * stride, tightStride);
                }
            }
            CVPixelBufferUnlockBaseAddress(buf, kCVPixelBufferLock_ReadOnly);
            CVPixelBufferRelease(buf);

            *outWidth = (int)w;
            *outHeight = (int)h;
            *outData = dst;
            *outLen = (int)totalBytes;
            return YES;
        }
    }

    CVPixelBufferUnlockBaseAddress(buf, kCVPixelBufferLock_ReadOnly);
    CVPixelBufferRelease(buf);
    return NO;
}

- (BOOL)copyFrameDirect:(uint8_t *)dst
                 maxLen:(int)maxLen
               outWidth:(int *)outWidth
              outHeight:(int *)outHeight
                 outLen:(int *)outLen {
    if (!_videoOutput || !_playerItem || !dst || !outWidth || !outHeight || !outLen) {
        return NO;
    }

    CMTime itemTime = [_playerItem currentTime];
    if (![_videoOutput hasNewPixelBufferForItemTime:itemTime]) {
        return NO;
    }
    CVPixelBufferRef buf = [_videoOutput copyPixelBufferForItemTime:itemTime itemTimeForDisplay:NULL];
    if (!buf) {
        return NO;
    }

    CVReturn lockErr = CVPixelBufferLockBaseAddress(buf, kCVPixelBufferLock_ReadOnly);
    if (lockErr != kCVReturnSuccess) {
        CVPixelBufferRelease(buf);
        return NO;
    }

    size_t w = CVPixelBufferGetWidth(buf);
    size_t h = CVPixelBufferGetHeight(buf);
    size_t stride = CVPixelBufferGetBytesPerRow(buf);
    uint8_t *src = (uint8_t *)CVPixelBufferGetBaseAddress(buf);

    if (src && w > 0 && h > 0) {
        size_t tightStride = w * 4;
        size_t totalBytes = tightStride * h;
        *outWidth = (int)w;
        *outHeight = (int)h;
        *outLen = (int)totalBytes;

        if ((size_t)maxLen >= totalBytes) {
            if (stride == tightStride) {
                memcpy(dst, src, totalBytes);
            } else {
                for (size_t row = 0; row < h; row++) {
                    memcpy(dst + row * tightStride, src + row * stride, tightStride);
                }
            }
            CVPixelBufferUnlockBaseAddress(buf, kCVPixelBufferLock_ReadOnly);
            CVPixelBufferRelease(buf);
            return YES;
        }
    }

    CVPixelBufferUnlockBaseAddress(buf, kCVPixelBufferLock_ReadOnly);
    CVPixelBufferRelease(buf);
    return NO;
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

int lumina_player_load(void* player, const char* path, double* out_duration, int* out_width, int* out_height) {
    if (!player || !path || strlen(path) == 0) return 0;
    @autoreleasepool {
        LuminaPlayerBridge *bridge = (__bridge LuminaPlayerBridge *)player;
        NSString *str = [NSString stringWithUTF8String:path];
        if (!str || ![[NSFileManager defaultManager] fileExistsAtPath:str]) {
            return 0;
        }
        return [bridge loadFile:str duration:out_duration width:out_width height:out_height] ? 1 : 0;
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

double lumina_player_get_duration(void* player) {
    if (!player) return 0.0;
    @autoreleasepool {
        LuminaPlayerBridge *bridge = (__bridge LuminaPlayerBridge *)player;
        return bridge.duration;
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

int lumina_player_copy_frame(void* player, int* out_width, int* out_height, uint8_t** out_data, int* out_len) {
    if (!player || !out_width || !out_height || !out_data || !out_len) return 0;
    @autoreleasepool {
        LuminaPlayerBridge *bridge = (__bridge LuminaPlayerBridge *)player;
        return [bridge copyFrameIntoWidth:out_width height:out_height data:out_data len:out_len] ? 1 : 0;
    }
}

int lumina_player_copy_frame_direct(void* player, uint8_t* dst, int max_len, int* out_width, int* out_height, int* out_len) {
    if (!player || !dst || !out_width || !out_height || !out_len) return 0;
    @autoreleasepool {
        LuminaPlayerBridge *bridge = (__bridge LuminaPlayerBridge *)player;
        return [bridge copyFrameDirect:dst maxLen:max_len outWidth:out_width outHeight:out_height outLen:out_len] ? 1 : 0;
    }
}

void lumina_player_free_frame(uint8_t* data) {
    if (data) {
        free(data);
    }
}


// Read once at startup by motion::reduce_motion().
int lumina_reduce_motion(void) {
    @autoreleasepool {
        return [[NSWorkspace sharedWorkspace] accessibilityDisplayShouldReduceMotion] ? 1 : 0;
    }
}
