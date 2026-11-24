// In-process image decoding through ImageIO for decoder.rs: scaled,
// oriented and colour matched to sRGB, replacing the sips process.

#import <CoreGraphics/CoreGraphics.h>
#import <Foundation/Foundation.h>
#import <ImageIO/ImageIO.h>
#include <stdlib.h>
#include <string.h>

static CGImageSourceRef open_source(const char *path) {
    CFURLRef url = CFURLCreateFromFileSystemRepresentation(NULL, (const UInt8 *)path,
                                                           (CFIndex)strlen(path), false);
    if (!url) return NULL;
    // Not cached: the source is read once, for one thumbnail.
    NSDictionary *options = @{(id)kCGImageSourceShouldCache : @NO};
    CGImageSourceRef source = CGImageSourceCreateWithURL(url, (__bridge CFDictionaryRef)options);
    CFRelease(url);
    return source;
}

// Asks the caller for a zeroed buffer of `len` bytes; NULL when it cannot.
typedef uint8_t *(*lumina_alloc_fn)(void *ctx, size_t len);

// The first image of `path` fitted inside `max_size` (never enlarged),
// turned by its EXIF orientation and drawn into sRGB as BGRA8 with straight
// alpha, into a width * height * 4 buffer from `alloc(ctx, len)` (the
// caller owns it, so the pixels are never copied). Returns 1, else 0.
int lumina_image_decode(const char *path, int max_size, int *out_width, int *out_height,
                        lumina_alloc_fn alloc, void *ctx) {
    if (max_size <= 0) return 0;
    @autoreleasepool {
        CGImageSourceRef source = open_source(path);
        if (!source) return 0;
        // A thumbnail from the full image (an embedded preview can be tiny),
        // decoded at the reduced size, so a large file's full-resolution
        // pixels are never held.
        NSDictionary *options = @{
            (id)kCGImageSourceCreateThumbnailFromImageAlways : @YES,
            (id)kCGImageSourceCreateThumbnailWithTransform : @YES,
            (id)kCGImageSourceThumbnailMaxPixelSize : @(max_size),
            (id)kCGImageSourceShouldCache : @NO,
        };
        CGImageRef image =
            CGImageSourceCreateThumbnailAtIndex(source, 0, (__bridge CFDictionaryRef)options);
        CFRelease(source);
        if (!image) return 0;

        size_t width = CGImageGetWidth(image), height = CGImageGetHeight(image);
        uint8_t *data = (width && height && width <= SIZE_MAX / 4 / height)
                            ? alloc(ctx, width * height * 4)
                            : NULL;
        CGColorSpaceRef space = CGColorSpaceCreateWithName(kCGColorSpaceSRGB);
        CGContextRef context =
            data ? CGBitmapContextCreate(data, width, height, 8, width * 4, space,
                                         kCGImageAlphaPremultipliedFirst | kCGBitmapByteOrder32Little)
                 : NULL;
        CGColorSpaceRelease(space);
        if (!context) {
            // The caller frees its buffer.
            CGImageRelease(image);
            return 0;
        }
        // Drawing into an sRGB context colour matches from the source profile.
        CGContextDrawImage(context, CGRectMake(0, 0, width, height), image);
        CGContextRelease(context);
        CGImageRelease(image);

        // Frames hold straight alpha, as the image crate decodes it.
        for (size_t i = 0; i < width * height * 4; i += 4) {
            uint8_t alpha = data[i + 3];
            if (alpha != 0 && alpha != 255) {
                for (int c = 0; c < 3; c++) {
                    unsigned value = (data[i + c] * 255u + alpha / 2) / alpha;
                    data[i + c] = value > 255 ? 255 : (uint8_t)value;
                }
            }
        }
        *out_width = (int)width;
        *out_height = (int)height;
        return 1;
    }
}

// The stored pixel size of the first image of `path` (before orientation).
// Returns 1 when ImageIO reports both, else 0.
int lumina_image_pixel_size(const char *path, int *out_width, int *out_height) {
    @autoreleasepool {
        CGImageSourceRef source = open_source(path);
        if (!source) return 0;
        CFDictionaryRef properties = CGImageSourceCopyPropertiesAtIndex(source, 0, NULL);
        CFRelease(source);
        if (!properties) return 0;
        NSDictionary *props = (__bridge_transfer NSDictionary *)properties;
        NSNumber *width = props[(id)kCGImagePropertyPixelWidth];
        NSNumber *height = props[(id)kCGImagePropertyPixelHeight];
        if (![width isKindOfClass:[NSNumber class]] || ![height isKindOfClass:[NSNumber class]] ||
            width.intValue <= 0 || height.intValue <= 0)
            return 0;
        *out_width = width.intValue;
        *out_height = height.intValue;
        return 1;
    }
}
