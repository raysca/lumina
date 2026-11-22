#import "macos_frames.h"
#include <mach/mach_time.h>
#include <os/lock.h>
#include <stdatomic.h>
#include <string.h>

/// Poll period while frames are flowing: under half a 60 fps frame.
static const uint64_t PULL_PERIOD_NS = 8 * NSEC_PER_MSEC;
/// Snapshot period while playing (about two a second).
static const double SNAPSHOT_PERIOD_SECS = 0.5;
/// Empty polls before the pump sleeps until AVFoundation says new frames are
/// coming (about half a second), so a paused video costs no wakeups.
static const int IDLE_POLLS = 60;

static void *const kFrameQueueKey = (void *)&kFrameQueueKey;

static double machSeconds(uint64_t ticks) {
    static mach_timebase_info_data_t tb;
    if (tb.denom == 0) mach_timebase_info(&tb);
    return (double)ticks * tb.numer / tb.denom / 1e9;
}

@implementation LuminaFramePump {
    AVPlayerItemVideoOutput *_surfaceOutput;
    AVPlayerItemVideoOutput *_snapshotOutput;
    dispatch_queue_t _queue;
    dispatch_source_t _timer;
    /// Guards the two untaken frames below.
    os_unfair_lock _lock;
    CVPixelBufferRef _surface;
    CVPixelBufferRef _snapshot;
    /// Bumped by discardPending; a pull that started before it is dropped.
    atomic_uint _generation;
    atomic_bool _playing;
    // Queue-only state.
    uint64_t _lastSnapshot;
    int _idlePolls;
    BOOL _asleep;
}

- (instancetype)initWithItem:(AVPlayerItem *)item
               snapshotWidth:(int)width
                      height:(int)height {
    self = [super init];
    if (!self) return nil;
    _lock = OS_UNFAIR_LOCK_INIT;
    atomic_init(&_generation, 0);
    atomic_init(&_playing, false);

    // GPUI's surface shader decodes full-range 4:2:0 with the BT.601 matrix.
    // Video is almost always BT.709, so ask AVFoundation to re-encode with the
    // 601 matrix (primaries and transfer stay 709): colours then decode right.
    // HDR is tone-mapped to SDR here; the 8-bit surface cannot carry it.
    NSDictionary *surfaceSettings = @{
        (id)kCVPixelBufferPixelFormatTypeKey: @(kCVPixelFormatType_420YpCbCr8BiPlanarFullRange),
        (id)kCVPixelBufferIOSurfacePropertiesKey: @{},
        (id)kCVPixelBufferMetalCompatibilityKey: @YES,
        AVVideoColorPropertiesKey: @{
            AVVideoColorPrimariesKey: AVVideoColorPrimaries_ITU_R_709_2,
            AVVideoTransferFunctionKey: AVVideoTransferFunction_ITU_R_709_2,
            AVVideoYCbCrMatrixKey: AVVideoYCbCrMatrix_ITU_R_601_4,
        },
    };
    _surfaceOutput = [[AVPlayerItemVideoOutput alloc] initWithOutputSettings:surfaceSettings];
    _snapshotOutput = [[AVPlayerItemVideoOutput alloc] initWithPixelBufferAttributes:@{
        (id)kCVPixelBufferPixelFormatTypeKey: @(kCVPixelFormatType_32BGRA),
        (id)kCVPixelBufferWidthKey: @(width),
        (id)kCVPixelBufferHeightKey: @(height),
    }];
    [item addOutput:_surfaceOutput];
    [item addOutput:_snapshotOutput];

    _queue = dispatch_queue_create("lumina.video.frames", DISPATCH_QUEUE_SERIAL);
    dispatch_queue_set_specific(_queue, kFrameQueueKey, kFrameQueueKey, NULL);
    [_surfaceOutput setDelegate:self queue:_queue];
    _timer = dispatch_source_create(DISPATCH_SOURCE_TYPE_TIMER, 0, 0, _queue);
    __weak LuminaFramePump *weakSelf = self;
    dispatch_source_set_event_handler(_timer, ^{
        [weakSelf pull];
    });
    dispatch_source_set_timer(_timer, DISPATCH_TIME_NOW, PULL_PERIOD_NS, NSEC_PER_MSEC);
    dispatch_resume(_timer);
    return self;
}

- (void)dealloc {
    [self stop];
}

- (void)stop {
    dispatch_source_t timer = _timer;
    _timer = nil;
    if (timer) {
        dispatch_source_cancel(timer);
        // Wait out a pull in flight (it reads the outputs). Not from the
        // queue itself: a pull's temporary reference can be the last one.
        if (dispatch_get_specific(kFrameQueueKey) != kFrameQueueKey) {
            dispatch_sync(_queue, ^{});
        }
    }
    os_unfair_lock_lock(&_lock);
    CVPixelBufferRef surface = _surface, snapshot = _snapshot;
    _surface = NULL;
    _snapshot = NULL;
    os_unfair_lock_unlock(&_lock);
    if (surface) CVPixelBufferRelease(surface);
    if (snapshot) CVPixelBufferRelease(snapshot);
}

/// Restarts polling after a sleep (any thread).
- (void)wake {
    __weak LuminaFramePump *weakSelf = self;
    dispatch_async(_queue, ^{
        LuminaFramePump *pump = weakSelf;
        if (!pump || !pump->_timer) return;
        pump->_idlePolls = 0;
        if (pump->_asleep) {
            pump->_asleep = NO;
            dispatch_source_set_timer(pump->_timer, DISPATCH_TIME_NOW, PULL_PERIOD_NS, NSEC_PER_MSEC);
        }
    });
}

- (void)outputMediaDataWillChange:(AVPlayerItemOutput *)sender {
    [self wake];
}

- (void)setPlaying:(BOOL)playing {
    atomic_store(&_playing, playing);
    [self wake];
}

- (void)discardPending {
    atomic_fetch_add(&_generation, 1);
    os_unfair_lock_lock(&_lock);
    CVPixelBufferRef surface = _surface, snapshot = _snapshot;
    _surface = NULL;
    _snapshot = NULL;
    os_unfair_lock_unlock(&_lock);
    if (surface) CVPixelBufferRelease(surface);
    if (snapshot) CVPixelBufferRelease(snapshot);
    [self wake];
}

/// One poll, on the queue: copies the newest surface and, when due, the
/// newest snapshot. AVFoundation converts during the copy, so this is where
/// the per-frame cost lands, off the UI thread.
- (void)pull {
    if (!_timer || _asleep) return;
    unsigned generation = atomic_load(&_generation);
    uint64_t now = mach_absolute_time();
    CVPixelBufferRef surface = NULL, snapshot = NULL;

    CMTime surfaceTime = [_surfaceOutput itemTimeForMachAbsoluteTime:now];
    if ([_surfaceOutput hasNewPixelBufferForItemTime:surfaceTime]) {
        surface = [_surfaceOutput copyPixelBufferForItemTime:surfaceTime itemTimeForDisplay:NULL];
    }
    BOOL due = !atomic_load(&_playing) || machSeconds(now - _lastSnapshot) >= SNAPSHOT_PERIOD_SECS;
    if (due) {
        CMTime snapshotTime = [_snapshotOutput itemTimeForMachAbsoluteTime:now];
        if ([_snapshotOutput hasNewPixelBufferForItemTime:snapshotTime]) {
            snapshot = [_snapshotOutput copyPixelBufferForItemTime:snapshotTime itemTimeForDisplay:NULL];
            if (snapshot) _lastSnapshot = now;
        }
    }

    if (!surface && !snapshot) {
        if (++_idlePolls >= IDLE_POLLS) {
            _asleep = YES;
            dispatch_source_set_timer(_timer, DISPATCH_TIME_FOREVER, PULL_PERIOD_NS, NSEC_PER_MSEC);
            [_surfaceOutput requestNotificationOfMediaDataChangeWithAdvanceInterval:0.05];
        }
        return;
    }
    _idlePolls = 0;

    // Both land in one critical section, so a caller that takes the snapshot
    // and then the surface never sees a surface older than its snapshot.
    os_unfair_lock_lock(&_lock);
    if (generation == atomic_load(&_generation)) {
        if (surface) {
            CVPixelBufferRef old = _surface;
            _surface = surface;
            surface = old;
        }
        if (snapshot) {
            CVPixelBufferRef old = _snapshot;
            _snapshot = snapshot;
            snapshot = old;
        }
    }
    os_unfair_lock_unlock(&_lock);
    // Replaced or stale frames.
    if (surface) CVPixelBufferRelease(surface);
    if (snapshot) CVPixelBufferRelease(snapshot);
}

- (CVPixelBufferRef)takeSurface {
    os_unfair_lock_lock(&_lock);
    CVPixelBufferRef surface = _surface;
    _surface = NULL;
    os_unfair_lock_unlock(&_lock);
    return surface;
}

- (BOOL)hasSnapshot {
    os_unfair_lock_lock(&_lock);
    BOOL waiting = _snapshot != NULL;
    os_unfair_lock_unlock(&_lock);
    return waiting;
}

- (int)copySnapshotInto:(uint8_t *)dst
               capacity:(size_t)capacity
                  width:(int *)outWidth
                 height:(int *)outHeight {
    os_unfair_lock_lock(&_lock);
    CVPixelBufferRef buf = _snapshot;
    _snapshot = NULL;
    os_unfair_lock_unlock(&_lock);
    if (!buf) return 0;
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
        // Keep it for the retry, unless a newer one already arrived.
        os_unfair_lock_lock(&_lock);
        if (!_snapshot) {
            _snapshot = buf;
            buf = NULL;
        }
        os_unfair_lock_unlock(&_lock);
        if (buf) CVPixelBufferRelease(buf);
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
