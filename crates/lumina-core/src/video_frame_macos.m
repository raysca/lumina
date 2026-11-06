// Video thumbnails through AVFoundation, in process (see video_frame.rs).
#import <AVFoundation/AVFoundation.h>
#import <CoreGraphics/CoreGraphics.h>
#import <Foundation/Foundation.h>
#include <stdint.h>
#include <stdlib.h>

// Frames the generator hands back, one slot per requested time. The completion
// block keeps this alive, so a call that gave up waiting never touches it again
// and the images are released whenever the generator finishes.
@interface LuminaFrameGrab : NSObject {
  @public
    CGImageRef images[2];
}
@end

@implementation LuminaFrameGrab
- (void)dealloc {
    for (int i = 0; i < 2; i++) {
        if (images[i]) {
            CGImageRelease(images[i]);
        }
    }
}
@end

// Draws `image` into a malloc'd BGRA8 buffer of width * height * 4 bytes
// (free with lumina_video_frame_free), or returns NULL.
static uint8_t *lumina_bgra(CGImageRef image, int *out_width, int *out_height) {
    size_t width = CGImageGetWidth(image);
    size_t height = CGImageGetHeight(image);
    uint8_t *data = (width && height) ? calloc(width * height, 4) : NULL;
    CGColorSpaceRef space = CGColorSpaceCreateWithName(kCGColorSpaceSRGB);
    // Little-endian, alpha first: B, G, R, A in memory, the texture format.
    CGContextRef context =
        data ? CGBitmapContextCreate(data, width, height, 8, width * 4, space,
                                     kCGImageAlphaPremultipliedFirst | kCGBitmapByteOrder32Little)
             : NULL;
    CGColorSpaceRelease(space);
    if (!context) {
        free(data);
        return NULL;
    }
    CGContextDrawImage(context, CGRectMake(0, 0, width, height), image);
    CGContextRelease(context);
    *out_width = (int)width;
    *out_height = (int)height;
    return data;
}

// Decodes a frame of the video at `path` (UTF-8), fitted inside
// max_size x max_size with the track's rotation applied, as BGRA8. Prefers
// the frame near 1 s (past a fade from black) over the first one. Gives up
// after timeout_seconds. Returns 1 and a malloc'd buffer of
// width * height * 4 bytes in *out_data (free with lumina_video_frame_free),
// or 0.
int lumina_video_frame(const char *path, int max_size, double timeout_seconds,
                       int *out_width, int *out_height, uint8_t **out_data) {
    @autoreleasepool {
        NSString *file = path ? [NSString stringWithUTF8String:path] : nil;
        if (!file || max_size <= 0) {
            return 0;
        }
        AVURLAsset *asset = [AVURLAsset URLAssetWithURL:[NSURL fileURLWithPath:file] options:nil];
        AVAssetImageGenerator *generator =
            [AVAssetImageGenerator assetImageGeneratorWithAsset:asset];
        generator.appliesPreferredTrackTransform = YES;
        generator.maximumSize = CGSizeMake(max_size, max_size);
        // Unlimited tolerance could snap the 1 s request back to a keyframe
        // at 0, the black frame it means to skip.
        generator.requestedTimeToleranceBefore = CMTimeMakeWithSeconds(0.5, 600);

        LuminaFrameGrab *grab = [LuminaFrameGrab new];
        dispatch_semaphore_t done = dispatch_semaphore_create(0);
        NSArray<NSValue *> *times = @[
            [NSValue valueWithCMTime:CMTimeMakeWithSeconds(1.0, 600)],
            [NSValue valueWithCMTime:kCMTimeZero],
        ];
        [generator generateCGImagesAsynchronouslyForTimes:times
                                        completionHandler:^(CMTime requested, CGImageRef image,
                                                            CMTime actual,
                                                            AVAssetImageGeneratorResult result,
                                                            NSError *error) {
                                            (void)actual;
                                            (void)error;
                                            if (result == AVAssetImageGeneratorSucceeded && image) {
                                                int slot = CMTimeCompare(requested, kCMTimeZero) == 0;
                                                @synchronized(grab) {
                                                    grab->images[slot] = CGImageRetain(image);
                                                }
                                            }
                                            dispatch_semaphore_signal(done);
                                        }];

        // `generator` must outlive the generation; it does, since it lives
        // until this function returns and we wait for both callbacks (or
        // cancel) first.
        dispatch_time_t deadline =
            dispatch_time(DISPATCH_TIME_NOW, (int64_t)(timeout_seconds * NSEC_PER_SEC));
        for (NSUInteger i = 0; i < times.count; i++) {
            if (dispatch_semaphore_wait(done, deadline) != 0) {
                [generator cancelAllCGImageGeneration];
                return 0;
            }
        }

        CGImageRef image = NULL;
        @synchronized(grab) {
            image = grab->images[0] ? grab->images[0] : grab->images[1];
            if (image) {
                CGImageRetain(image);
            }
        }
        if (!image) {
            return 0;
        }

        int width = 0, height = 0;
        uint8_t *data = lumina_bgra(image, &width, &height);
        CGImageRelease(image);
        if (!data) {
            return 0;
        }
        *out_width = width;
        *out_height = height;
        *out_data = data;
        return 1;
    }
}

void lumina_video_frame_free(uint8_t *data) { free(data); }

// One slot of lumina_video_frames' output (mirrored in video_frame.rs).
typedef struct {
    double time;
    int width;
    int height;
    uint8_t *data;
} LuminaFrameOut;

// The batch's frames, one slot per requested time. As with LuminaFrameGrab,
// only the completion block writes here and it keeps this object alive, so a
// callback that fires after lumina_video_frames returned (cancel, timeout)
// lands here and is released with it, never in the caller's memory.
@interface LuminaFrameBatch : NSObject {
  @public
    int count;
    CMTime *requested;
    CMTime *actual;
    CGImageRef *images;
}
@end

@implementation LuminaFrameBatch
- (void)dealloc {
    for (int i = 0; i < count; i++) {
        if (images[i]) {
            CGImageRelease(images[i]);
        }
    }
    free(requested);
    free(actual);
    free(images);
}
@end

static int lumina_cancelled(const volatile int32_t *cancel) {
    return cancel && __atomic_load_n(cancel, __ATOMIC_RELAXED) != 0;
}

// Decodes frames of the video at `path` (UTF-8) near each of `times`
// (seconds, within tolerance_s either side), fitted inside max_size x
// max_size with the track's rotation applied, as BGRA8. One deadline of
// timeout_s covers the batch; a non-zero *cancel (read only while this call
// runs) stops it. Either way the frames completed so far are kept. Fills
// out[0..n] in ascending actual time and returns n; free each data with
// lumina_video_frame_free.
int lumina_video_frames(const char *path, int max_size, const double *times, int count,
                        double tolerance_s, double timeout_s, const volatile int32_t *cancel,
                        LuminaFrameOut *out) {
    @autoreleasepool {
        NSString *file = path ? [NSString stringWithUTF8String:path] : nil;
        if (!file || max_size <= 0 || !times || count <= 0 || !out || lumina_cancelled(cancel)) {
            return 0;
        }
        LuminaFrameBatch *batch = [LuminaFrameBatch new];
        batch->requested = calloc(count, sizeof(CMTime));
        batch->actual = calloc(count, sizeof(CMTime));
        batch->images = calloc(count, sizeof(CGImageRef));
        if (!batch->requested || !batch->actual || !batch->images) {
            return 0;
        }
        batch->count = count;
        NSMutableArray<NSValue *> *requests = [NSMutableArray arrayWithCapacity:count];
        for (int i = 0; i < count; i++) {
            batch->requested[i] = CMTimeMakeWithSeconds(times[i], 600);
            [requests addObject:[NSValue valueWithCMTime:batch->requested[i]]];
        }

        AVURLAsset *asset = [AVURLAsset URLAssetWithURL:[NSURL fileURLWithPath:file] options:nil];
        AVAssetImageGenerator *generator =
            [AVAssetImageGenerator assetImageGeneratorWithAsset:asset];
        generator.appliesPreferredTrackTransform = YES;
        generator.maximumSize = CGSizeMake(max_size, max_size);
        CMTime tolerance = CMTimeMakeWithSeconds(tolerance_s > 0 ? tolerance_s : 0, 600);
        generator.requestedTimeToleranceBefore = tolerance;
        generator.requestedTimeToleranceAfter = tolerance;

        dispatch_semaphore_t done = dispatch_semaphore_create(0);
        [generator generateCGImagesAsynchronouslyForTimes:requests
                                        completionHandler:^(CMTime req, CGImageRef image,
                                                            CMTime actual,
                                                            AVAssetImageGeneratorResult result,
                                                            NSError *error) {
                                            (void)error;
                                            if (result == AVAssetImageGeneratorSucceeded && image) {
                                                @synchronized(batch) {
                                                    for (int i = 0; i < batch->count; i++) {
                                                        if (!batch->images[i] &&
                                                            CMTimeCompare(batch->requested[i], req) == 0) {
                                                            batch->images[i] = CGImageRetain(image);
                                                            batch->actual[i] = actual;
                                                            break;
                                                        }
                                                    }
                                                }
                                            }
                                            dispatch_semaphore_signal(done);
                                        }];

        // Wait in 50 ms slices so a cancel is seen promptly. On cancel or
        // timeout, stop the generator and keep what has arrived.
        dispatch_time_t deadline =
            dispatch_time(DISPATCH_TIME_NOW, (int64_t)(timeout_s * NSEC_PER_SEC));
        int finished = 0;
        while (finished < count) {
            if (lumina_cancelled(cancel)) {
                [generator cancelAllCGImageGeneration];
                break;
            }
            dispatch_time_t slice = dispatch_time(DISPATCH_TIME_NOW, 50 * NSEC_PER_MSEC);
            if (dispatch_semaphore_wait(done, slice) == 0) {
                finished++;
            } else if (dispatch_time(DISPATCH_TIME_NOW, 0) >= deadline) {
                [generator cancelAllCGImageGeneration];
                break;
            }
        }

        // Take the finished frames out of the holder, then convert them
        // without holding its lock.
        CGImageRef *images = calloc(count, sizeof(CGImageRef));
        double *seconds = calloc(count, sizeof(double));
        int n = 0;
        if (images && seconds) {
            @synchronized(batch) {
                for (int i = 0; i < count; i++) {
                    if (batch->images[i]) {
                        images[n] = CGImageRetain(batch->images[i]);
                        seconds[n] = CMTimeGetSeconds(batch->actual[i]);
                        n++;
                    }
                }
            }
        }
        // Insertion sort by actual time: n is at most a few dozen.
        for (int i = 1; i < n; i++) {
            for (int j = i; j > 0 && seconds[j - 1] > seconds[j]; j--) {
                double t = seconds[j];
                seconds[j] = seconds[j - 1];
                seconds[j - 1] = t;
                CGImageRef image = images[j];
                images[j] = images[j - 1];
                images[j - 1] = image;
            }
        }
        int filled = 0;
        for (int i = 0; i < n; i++) {
            int width = 0, height = 0;
            uint8_t *data = lumina_bgra(images[i], &width, &height);
            CGImageRelease(images[i]);
            if (data) {
                out[filled].time = seconds[i];
                out[filled].width = width;
                out[filled].height = height;
                out[filled].data = data;
                filled++;
            }
        }
        free(images);
        free(seconds);
        return filled;
    }
}
