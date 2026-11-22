#import <AVFoundation/AVFoundation.h>
#import <CoreVideo/CoreVideo.h>

/// Pulls a player item's frames on a background queue so the UI thread
/// never copies or converts pixels. Two outputs share the item's decode:
///
/// - the surface: IOSurface-backed 4:2:0 buffers at native size, which GPUI
///   paints with its surface element straight from the IOSurface (no copy);
/// - the snapshot: a BGRA copy capped at 1080p for views that need an image
///   (navigator, inspector preview, the PiP handoff), taken at most every
///   half second while playing and for every frame while paused.
///
/// The UI thread takes the newest of each; frames it never took are dropped.
@interface LuminaFramePump : NSObject <AVPlayerItemOutputPullDelegate>

/// Adds both outputs to `item`. The snapshot is `width` x `height`.
- (instancetype)initWithItem:(AVPlayerItem *)item
               snapshotWidth:(int)width
                      height:(int)height;

/// Stops pulling and releases untaken frames. Safe to call more than once.
- (void)stop;

/// Playing pulls snapshots at a low rate; paused pulls every one, so a seek
/// or step shows its frame everywhere.
- (void)setPlaying:(BOOL)playing;

/// Drops untaken frames and any frame being pulled, so a seek or step never
/// hands back a frame from before it.
- (void)discardPending;

/// The newest surface not yet taken (+1 retained), or NULL.
- (CVPixelBufferRef)takeSurface CF_RETURNS_RETAINED;

/// Whether a snapshot is waiting.
- (BOOL)hasSnapshot;

/// Copies the waiting snapshot, tightly packed BGRA, into `dst`. 1: copied.
/// 0: none waiting. -1: `capacity` is too small for `*outWidth` x
/// `*outHeight`; the snapshot is kept for a retry with a bigger buffer.
- (int)copySnapshotInto:(uint8_t *)dst
               capacity:(size_t)capacity
                  width:(int *)outWidth
                 height:(int *)outHeight;

@end
