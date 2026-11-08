// Share sheet bridge (macOS only). Compiled with -fobjc-arc by build.rs,
// like the player bridge. Lumina makes no network call here: the picker's
// services are the system's.
#import <AppKit/AppKit.h>
#import <Foundation/Foundation.h>
#include <math.h>
#include <stdbool.h>

@interface LuminaSharePickerDelegate : NSObject <NSSharingServicePickerDelegate>
@end

// The picker showing now, and its delegate (the picker holds it weakly).
// Kept until the delegate reports dismissal, so neither is released while
// the picker is on screen.
static NSSharingServicePicker *gLuminaSharePicker = nil;
static LuminaSharePickerDelegate *gLuminaShareDelegate = nil;

@implementation LuminaSharePickerDelegate
// Called when the picker closes, with nil when the user chose nothing.
- (void)sharingServicePicker:(NSSharingServicePicker *)picker
     didChooseSharingService:(NSSharingService *)service {
    (void)service;
    if (picker == gLuminaSharePicker) {
        gLuminaSharePicker = nil;
        gLuminaShareDelegate = nil;
    }
}
@end

// Shows the system share picker for `count` NUL-terminated file paths at
// (x, y) in window points with a top-left origin; NaN means the centre of
// the window. Main thread only. Returns false only when there is no key
// window to show it in (the caller says so); every other early return has
// nothing to tell the user. No ObjC exception may cross this extern "C"
// boundary (it would abort the process), so misuse logs and returns.
bool lumina_share_files(const char *const *paths, int count, double x, double y) {
    if (![NSThread isMainThread]) {
        NSLog(@"lumina_share_files called off the main thread; ignored");
        return true;
    }
    if (paths == NULL || count <= 0) {
        return true;
    }
    NSView *view = [[NSApp keyWindow] contentView];
    if (view == nil) {
        return false;
    }
    NSMutableArray<NSURL *> *urls = [NSMutableArray arrayWithCapacity:(NSUInteger)count];
    for (int i = 0; i < count; i++) {
        if (paths[i] == NULL) {
            continue;
        }
        // Raw file system bytes, so a non-UTF-8 name still makes a URL.
        NSURL *url = [NSURL fileURLWithFileSystemRepresentation:paths[i]
                                                    isDirectory:NO
                                                  relativeToURL:nil];
        if (url != nil) {
            [urls addObject:url];
        }
    }
    if (urls.count == 0) {
        return true;
    }

    NSRect bounds = view.bounds;
    NSPoint at = NSMakePoint(NSMidX(bounds), NSMidY(bounds));
    if (!isnan(x) && !isnan(y)) {
        // GPUI points have a top-left origin; an unflipped view's is bottom-left.
        at = NSMakePoint(x, view.isFlipped ? y : NSHeight(bounds) - y);
    }
    NSRect anchor = NSMakeRect(at.x, at.y, 1, 1);

    // A second request replaces the one showing. -close is macOS 13+; on 12
    // the one showing stays and the new request is ignored.
    if (gLuminaSharePicker != nil) {
        if (@available(macOS 13.0, *)) {
            [gLuminaSharePicker close];
        } else {
            return true;
        }
    }
    gLuminaSharePicker = [[NSSharingServicePicker alloc] initWithItems:urls];
    gLuminaShareDelegate = [[LuminaSharePickerDelegate alloc] init];
    gLuminaSharePicker.delegate = gLuminaShareDelegate;
    [gLuminaSharePicker showRelativeToRect:anchor ofView:view preferredEdge:NSRectEdgeMinY];
    return true;
}
