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
            CGImageRelease(image);
            return 0;
        }
        CGContextDrawImage(context, CGRectMake(0, 0, width, height), image);
        CGContextRelease(context);
        CGImageRelease(image);

        *out_width = (int)width;
        *out_height = (int)height;
        *out_data = data;
        return 1;
    }
}

void lumina_video_frame_free(uint8_t *data) { free(data); }
