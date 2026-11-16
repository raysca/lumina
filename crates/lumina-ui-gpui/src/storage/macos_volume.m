// Volume capacity bridge (macOS only). Compiled with -fobjc-arc by build.rs,
// like the player and share bridges. Reads file-system attributes only.
#import <Foundation/Foundation.h>
#include <stdbool.h>
#include <stdint.h>

// The total and available bytes of the volume holding `path`. Available is
// the "important usage" figure Finder shows: free space plus what macOS
// can purge on demand. Returns false (outputs untouched) when the path or
// either value cannot be read. Any thread. No ObjC exception may cross
// this extern "C" boundary, so failures return false.
bool lumina_volume_capacity(const char *path, uint64_t *total, uint64_t *available) {
    if (path == NULL || total == NULL || available == NULL) {
        return false;
    }
    @autoreleasepool {
        NSString *string = [NSString stringWithUTF8String:path];
        if (string == nil) {
            return false;
        }
        NSURL *url = [NSURL fileURLWithPath:string];
        NSDictionary<NSURLResourceKey, id> *values = [url
            resourceValuesForKeys:@[
                NSURLVolumeTotalCapacityKey,
                NSURLVolumeAvailableCapacityForImportantUsageKey
            ]
                            error:nil];
        NSNumber *totalBytes = values[NSURLVolumeTotalCapacityKey];
        NSNumber *availableBytes = values[NSURLVolumeAvailableCapacityForImportantUsageKey];
        if (totalBytes == nil || availableBytes == nil) {
            return false;
        }
        long long t = totalBytes.longLongValue;
        long long a = availableBytes.longLongValue;
        if (t <= 0 || a < 0) {
            return false;
        }
        *total = (uint64_t)t;
        // Purgeable space can make "important usage" exceed the total on
        // some volumes; never report more free space than the volume has.
        *available = a > t ? (uint64_t)t : (uint64_t)a;
        return true;
    }
}
