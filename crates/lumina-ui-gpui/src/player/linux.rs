//! GStreamer playback backend used by the Linux viewer.

use super::backend::{VideoFrame, VideoMetadata, VideoPlayerBackend};
use anyhow::{anyhow, Context, Result};
use gst_video::prelude::*;
use gstreamer as gst;
use gstreamer::prelude::*;
use gstreamer_app as gst_app;
use gstreamer_video as gst_video;
use std::cell::Cell;
use std::os::unix::ffi::OsStrExt;
use std::path::Path;
use std::sync::OnceLock;
use std::time::{Duration, Instant};

static GST_INIT: OnceLock<std::result::Result<(), String>> = OnceLock::new();
const STATE_CHANGE_TIMEOUT: Duration = Duration::from_secs(10);
/// Shown in the playback-failed card when a load fails. The cause, which
/// names the playback library, goes to the log only.
const LOAD_FAILED_MESSAGE: &str =
    "This video could not be opened. It may be damaged or use a format this system can't play.";
/// Shown in the playback-failed card when decoding fails mid-clip.
const PLAYBACK_FAILED_MESSAGE: &str =
    "This video stopped playing because part of it could not be decoded.";

fn initialize_gstreamer() -> Result<()> {
    GST_INIT
        .get_or_init(|| gst::init().map_err(|error| error.to_string()))
        .as_ref()
        .map_err(|error| anyhow!("Could not initialize GStreamer: {error}"))
        .map(|_| ())
}

fn file_uri(path: &Path) -> Result<String> {
    let path = std::fs::canonicalize(path)
        .with_context(|| format!("Could not resolve video path {}", path.display()))?;
    let mut uri = String::from("file://");
    for &byte in path.as_os_str().as_bytes() {
        if byte.is_ascii_alphanumeric() || b"/-._~:".contains(&byte) {
            uri.push(char::from(byte));
        } else {
            uri.push_str(&format!("%{byte:02X}"));
        }
    }
    Ok(uri)
}

fn wait_for_state(element: &gst::Element, requested: gst::State, timeout: Duration) -> Result<()> {
    let started = Instant::now();
    loop {
        let remaining = timeout.saturating_sub(started.elapsed());
        if remaining.is_zero() {
            return Err(anyhow!("GStreamer timed out while entering {requested:?}"));
        }
        let timeout_ns = remaining.as_nanos().min(u64::MAX as u128) as u64;
        let (result, current, _) = element.state(gst::ClockTime::from_nseconds(timeout_ns));
        match result {
            Err(error) => return Err(anyhow!("GStreamer failed to enter {requested:?}: {error}")),
            Ok(gst::StateChangeSuccess::Success | gst::StateChangeSuccess::NoPreroll)
                if current == requested =>
            {
                return Ok(());
            }
            Ok(gst::StateChangeSuccess::Success | gst::StateChangeSuccess::NoPreroll)
            | Ok(gst::StateChangeSuccess::Async) => {}
        }
    }
}

/// Linux video player backed by GStreamer's playbin and an appsink video output.
pub struct GStreamerPlayer {
    playbin: Option<gst::Element>,
    sink: Option<gst_app::AppSink>,
    bus: Option<gst::Bus>,
    duration_secs: f64,
    width: u32,
    height: u32,
    frame_rate: Option<(u32, u32)>,
    rate: f32,
    volume: f32,
    muted: bool,
    looping: bool,
    playing: Cell<bool>,
    /// A decode error from the bus, set by `is_playing` and taken by the UI
    /// through `take_playback_error`.
    playback_error: Cell<Option<String>>,
    pending_frame: Option<VideoFrame>,
    /// Target of the last seek whose destination frame has not been read
    /// yet. `position()` can still report the old time until the seek lands,
    /// so a rate change or frame step starts from here instead.
    pending_seek: Option<f64>,
    #[cfg(test)]
    pub test_selected_audio: Option<usize>,
}

impl GStreamerPlayer {
    pub fn new() -> Self {
        Self {
            playbin: None,
            sink: None,
            bus: None,
            duration_secs: 0.0,
            width: 0,
            height: 0,
            frame_rate: None,
            rate: 1.0,
            volume: 1.0,
            muted: false,
            looping: false,
            playing: Cell::new(false),
            playback_error: Cell::new(None),
            pending_frame: None,
            pending_seek: None,
            #[cfg(test)]
            test_selected_audio: None,
        }
    }

    fn set_state(&self, state: gst::State) {
        if let Some(playbin) = self.playbin.as_ref() {
            if let Err(error) = playbin.set_state(state) {
                log::warn!("GStreamer state change to {state:?} failed: {error}");
            }
        }
    }

    /// A flushing, accurate seek to `seconds` at the chosen rate. Every seek
    /// carries `self.rate`: a seek's rate replaces the pipeline's, so a plain
    /// `seek_simple` (always 1.0) would silently drop a 1.5x choice.
    fn seek_to(&self, seconds: f64) -> bool {
        let Some(playbin) = self.playbin.as_ref() else {
            return false;
        };
        if !seconds.is_finite() || seconds < 0.0 {
            return false;
        }
        let nanos = (seconds * 1_000_000_000.0).min(u64::MAX as f64) as u64;
        match playbin.seek(
            f64::from(self.rate),
            gst::SeekFlags::FLUSH | gst::SeekFlags::ACCURATE,
            gst::SeekType::Set,
            gst::ClockTime::from_nseconds(nanos),
            gst::SeekType::None,
            gst::ClockTime::MAX,
        ) {
            Ok(()) => true,
            Err(error) => {
                log::warn!("GStreamer seek to {seconds:.3}s failed: {error}");
                false
            }
        }
    }

    /// Where the playhead is going: a pending seek's target, else the
    /// pipeline's position.
    fn seek_origin(&self) -> f64 {
        self.pending_seek.unwrap_or_else(|| self.position())
    }

    fn read_frame(&mut self) -> Option<VideoFrame> {
        if self.pending_frame.is_some() {
            return self.pending_frame.take();
        }
        let sink = self.sink.as_ref()?;
        if self.pending_seek.is_some() {
            if let Some(sample) = sink.try_pull_preroll(gst::ClockTime::ZERO) {
                self.pending_seek = None;
                return Self::frame_from_sample(&sample);
            }
            return None;
        }
        let sample = sink.try_pull_sample(gst::ClockTime::ZERO)?;
        Self::frame_from_sample(&sample)
    }

    fn frame_from_sample(sample: &gst::Sample) -> Option<VideoFrame> {
        let caps = sample.caps()?;
        let info = gst_video::VideoInfo::from_caps(caps).ok()?;
        if info.format() != gst_video::VideoFormat::Bgra {
            return None;
        }
        let width = info.width();
        let height = info.height();
        let buffer = sample.buffer()?;
        // Map through GstVideoFrame so per-buffer GstVideoMeta layout is
        // honored when a decoder supplies custom plane offsets or strides.
        let frame = gst_video::VideoFrameRef::from_buffer_ref_readable(buffer, &info).ok()?;
        if frame.format() != gst_video::VideoFormat::Bgra {
            return None;
        }
        let stride = frame.plane_stride().first().copied()?;
        Some(VideoFrame {
            width,
            height,
            data: copy_bgra_rows(frame.plane_data(0).ok()?, width, height, stride, 0)?,
        })
    }
}

/// Copies the visible BGRA pixels from one padded GStreamer plane.
fn copy_bgra_rows(
    data: &[u8],
    width: u32,
    height: u32,
    stride: i32,
    offset: usize,
) -> Option<Vec<u8>> {
    let row_bytes = usize::try_from(width).ok()?.checked_mul(4)?;
    let stride = usize::try_from(stride).ok()?;
    if stride < row_bytes {
        return None;
    }
    let output_size = row_bytes.checked_mul(height as usize)?;
    let mut output = Vec::with_capacity(output_size);
    for row in 0..height as usize {
        let start = offset.checked_add(row.checked_mul(stride)?)?;
        output.extend_from_slice(data.get(start..start.checked_add(row_bytes)?)?);
    }
    Some(output)
}

/// A `secs`-long 64x48 test clip written with ffmpeg (a Linux test
/// dependency, as for the preview tests), for resume points the 2 s fixture
/// is too short to have. The caller removes it.
#[cfg(test)]
pub(crate) fn test_clip(secs: u32) -> std::path::PathBuf {
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let path = std::env::temp_dir().join(format!(
        "lumina_test_clip_{}_{}.mp4",
        std::process::id(),
        NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    ));
    let status = std::process::Command::new("ffmpeg")
        .args(["-nostdin", "-v", "error", "-y", "-f", "lavfi", "-i"])
        .arg(format!("testsrc=size=64x48:rate=10:duration={secs}"))
        .args(["-c:v", "mpeg4"])
        .arg(&path)
        .status()
        .expect("ffmpeg runs to write the test clip");
    assert!(status.success(), "ffmpeg writes the test clip");
    path
}

#[cfg(test)]
impl GStreamerPlayer {
    /// Posts a decode error on the loaded pipeline's bus, as a failing
    /// decoder would mid-clip.
    /// The rate of the pipeline's current segment, as the sinks play it.
    pub(crate) fn segment_rate(&self) -> Option<f64> {
        let playbin = self.playbin.as_ref()?;
        let mut query = gst::query::Segment::new(gst::Format::Time);
        playbin.query(&mut query).then(|| query.result().0)
    }

    /// Waits up to 3 s for the segment rate to become `rate`, returning the
    /// last rate seen.
    pub(crate) fn wait_for_segment_rate(&self, rate: f64) -> Option<f64> {
        let deadline = Instant::now() + Duration::from_secs(3);
        loop {
            let seen = self.segment_rate();
            if seen.is_some_and(|seen| (seen - rate).abs() < 1e-6) || Instant::now() >= deadline {
                return seen;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    pub(crate) fn post_test_error(&self) {
        let bus = self.bus.as_ref().expect("a loaded player has a bus");
        bus.post(gst::message::Error::new(
            gst::CoreError::Failed,
            "test decode failure",
        ))
        .expect("the bus accepts the test error");
    }
}

impl Default for GStreamerPlayer {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for GStreamerPlayer {
    fn drop(&mut self) {
        self.set_state(gst::State::Null);
    }
}

impl GStreamerPlayer {
    /// [`VideoPlayerBackend::load`] with errors that name GStreamer, for
    /// the log.
    fn load_pipeline(&mut self, path: &Path) -> Result<VideoMetadata> {
        self.set_state(gst::State::Null);
        self.playbin = None;
        self.sink = None;
        self.bus = None;
        self.duration_secs = 0.0;
        self.width = 0;
        self.height = 0;
        self.frame_rate = None;
        // A new pipeline plays at normal speed until `set_rate` changes it.
        self.rate = 1.0;
        self.playing.set(false);
        self.playback_error.set(None);
        self.pending_frame = None;
        self.pending_seek = None;
        initialize_gstreamer()?;

        let playbin = gst::ElementFactory::make("playbin")
            .build()
            .context("GStreamer playbin is unavailable")?;
        let caps = gst::Caps::builder("video/x-raw")
            .field("format", "BGRA")
            .build();
        let sink = gst_app::AppSink::builder()
            .caps(&caps)
            .max_buffers(1)
            .drop(true)
            .sync(true)
            .build();
        playbin.set_property("video-sink", &sink);
        playbin.set_property("uri", file_uri(path)?);
        playbin.set_property("volume", f64::from(self.volume));
        playbin.set_property("mute", self.muted);

        let bus = playbin
            .bus()
            .context("GStreamer player has no message bus")?;
        playbin
            .set_state(gst::State::Paused)
            .context("GStreamer could not open this video")?;

        // get_state may return Async when its wait expires, not only on error.
        // Keep waiting for preroll on this background worker, with a hard cap.
        if let Err(error) = wait_for_state(&playbin, gst::State::Paused, STATE_CHANGE_TIMEOUT) {
            playbin.set_state(gst::State::Null).ok();
            return Err(anyhow!("GStreamer could not decode this video: {error:#}"));
        }
        let duration = playbin
            .query_duration::<gst::ClockTime>()
            .map(|duration| duration.nseconds() as f64 / 1_000_000_000.0)
            .unwrap_or(0.0);
        if duration <= 0.0 || !duration.is_finite() {
            playbin.set_state(gst::State::Null).ok();
            return Err(anyhow!("GStreamer could not read the video duration"));
        }
        let preroll = sink.try_pull_preroll(gst::ClockTime::from_mseconds(50));
        let sample = preroll.or_else(|| sink.try_pull_sample(gst::ClockTime::from_mseconds(50)));
        let first_frame = sample.as_ref().and_then(Self::frame_from_sample);
        let info = sample
            .as_ref()
            .and_then(|sample| sample.caps())
            .and_then(|caps| gst_video::VideoInfo::from_caps(caps).ok());

        self.duration_secs = duration;
        self.width = info.as_ref().map_or(0, gst_video::VideoInfo::width);
        self.height = info.as_ref().map_or(0, gst_video::VideoInfo::height);
        self.frame_rate = info.and_then(|info| {
            let fps = info.fps();
            let (numer, denom) = (fps.numer(), fps.denom());
            (numer > 0 && denom > 0).then_some((numer as u32, denom as u32))
        });
        self.pending_frame = first_frame;
        self.sink = Some(sink);
        self.bus = Some(bus);
        self.playbin = Some(playbin);
        Ok(VideoMetadata {
            duration_secs: duration,
            width: self.width,
            height: self.height,
        })
    }
}

impl VideoPlayerBackend for GStreamerPlayer {
    fn load(&mut self, path: &Path) -> Result<VideoMetadata> {
        self.load_pipeline(path).map_err(|error| {
            log::warn!("GStreamer could not load {}: {error:#}", path.display());
            anyhow!(LOAD_FAILED_MESSAGE)
        })
    }

    fn take_playback_error(&mut self) -> Option<String> {
        self.playback_error.take()
    }

    fn play(&mut self) {
        self.playing.set(true);
        self.set_state(gst::State::Playing);
    }

    fn pause(&mut self) {
        self.playing.set(false);
        self.set_state(gst::State::Paused);
    }

    fn is_playing(&self) -> bool {
        let Some(playbin) = self.playbin.as_ref() else {
            #[cfg(test)]
            {
                // UI tests use an unloaded player as a deterministic stub.
                // Preserve its play/pause intent without a GStreamer pipeline.
                return self.playing.get();
            }
            #[cfg(not(test))]
            return false;
        };
        if let Some(bus) = self.bus.as_ref() {
            // An ERROR left on the bus would leave the clip "playing" with
            // no frames; the UI takes it through `take_playback_error`.
            if let Some(message) =
                bus.pop_filtered(&[gst::MessageType::Eos, gst::MessageType::Error])
            {
                if let gst::MessageView::Error(error) = message.view() {
                    log::error!(
                        "GStreamer playback error: {} ({:?})",
                        error.error(),
                        error.debug()
                    );
                    self.playing.set(false);
                    self.set_state(gst::State::Paused);
                    self.playback_error
                        .set(Some(PLAYBACK_FAILED_MESSAGE.into()));
                    return false;
                }
                if self.looping {
                    self.seek_to(0.0);
                    self.set_state(gst::State::Playing);
                    self.playing.set(true);
                    return true;
                }
                self.playing.set(false);
                return false;
            }
        }
        self.playing.get() && playbin.current_state() != gst::State::Null
    }

    fn seek(&mut self, seconds: f64) {
        // A seek flushes the sink's queued samples. Drop the separately held
        // preroll too, or the viewer would briefly show time zero after seek.
        self.pending_frame = None;
        self.pending_seek = self.seek_to(seconds).then_some(seconds);
    }

    fn step_frames(&mut self, frames: i32) {
        let Some((numer, denom)) = self.frame_rate.filter(|_| self.playbin.is_some()) else {
            return;
        };
        if frames == 0 {
            return;
        }
        let frame_duration = f64::from(denom) / f64::from(numer);
        let target =
            (self.seek_origin() + f64::from(frames) * frame_duration).clamp(0.0, self.duration());
        self.seek(target);
    }

    fn position(&self) -> f64 {
        self.playbin
            .as_ref()
            .and_then(|playbin| playbin.query_position::<gst::ClockTime>())
            .map(|position| position.nseconds() as f64 / 1_000_000_000.0)
            .unwrap_or(0.0)
    }

    fn duration(&self) -> f64 {
        self.playbin
            .as_ref()
            .and_then(|playbin| playbin.query_duration::<gst::ClockTime>())
            .map(|duration| duration.nseconds() as f64 / 1_000_000_000.0)
            .filter(|duration| duration.is_finite() && *duration > 0.0)
            .unwrap_or(self.duration_secs)
    }

    fn set_rate(&mut self, rate: f32) {
        // A rate change is a flushing seek to the current position; skip it
        // when nothing changes, or it can undo a seek still in flight.
        let rate = rate.clamp(0.25, 4.0);
        if rate == self.rate {
            return;
        }
        self.rate = rate;
        if self.playbin.is_some() {
            // From a seek still in flight, not the lagging position, or the
            // playhead would jump back to where it was before that seek.
            // The held preroll frame stays valid: the rate changes, not
            // the time.
            let origin = self.seek_origin();
            self.pending_seek = self.seek_to(origin).then_some(origin);
        }
    }

    fn rate(&self) -> f32 {
        self.rate
    }

    fn set_volume(&mut self, volume: f32) {
        self.volume = volume.clamp(0.0, 1.0);
        if let Some(playbin) = self.playbin.as_ref() {
            playbin.set_property("volume", f64::from(self.volume));
        }
    }

    fn volume(&self) -> f32 {
        self.volume
    }

    fn set_muted(&mut self, muted: bool) {
        self.muted = muted;
        if let Some(playbin) = self.playbin.as_ref() {
            playbin.set_property("mute", muted);
        }
    }

    fn is_muted(&self) -> bool {
        self.muted
    }

    fn set_looping(&mut self, looping: bool) {
        self.looping = looping;
    }

    fn is_looping(&self) -> bool {
        self.looping
    }

    fn next_frame(&mut self) -> Option<VideoFrame> {
        self.read_frame()
    }

    fn selected_audio_track(&self) -> Option<usize> {
        #[cfg(test)]
        if self.test_selected_audio.is_some() {
            return self.test_selected_audio;
        }
        self.playbin
            .as_ref()
            .and_then(|playbin| usize::try_from(playbin.property::<i32>("current-audio")).ok())
    }

    fn audio_tracks(&self) -> Vec<String> {
        self.playbin
            .as_ref()
            .map(|playbin| {
                let count = playbin.property::<i32>("n-audio").max(0) as usize;
                (0..count)
                    .map(|index| format!("Audio track {}", index + 1))
                    .collect()
            })
            .unwrap_or_default()
    }

    fn select_audio_track(&mut self, index: usize) {
        #[cfg(test)]
        {
            self.test_selected_audio = Some(index);
        }
        let Some(playbin) = self.playbin.as_ref() else {
            return;
        };
        if let Ok(index) = i32::try_from(index) {
            let count = playbin.property::<i32>("n-audio");
            if index >= 0 && index < count {
                playbin.set_property("current-audio", index);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[core::prelude::v1::test]
    fn state_wait_keeps_waiting_after_an_async_timeout() {
        initialize_gstreamer().expect("GStreamer should initialize");
        let source = gst::ElementFactory::make("fakesrc")
            .property("num-buffers", 1i32)
            .build()
            .expect("the fakesrc test element should be installed");
        let delay = gst::ElementFactory::make("identity")
            .property("sleep-time", 1_000_000u32)
            .build()
            .expect("the identity test element should be installed");
        let sink = gst::ElementFactory::make("fakesink")
            .build()
            .expect("the fakesink test element should be installed");
        let pipeline = gst::Pipeline::new();
        pipeline
            .add_many([&source, &delay, &sink])
            .expect("the test elements should be added");
        gst::Element::link_many([&source, &delay, &sink]).expect("the test elements should link");
        pipeline
            .set_state(gst::State::Paused)
            .expect("the test pipeline should start prerolling");

        let started = Instant::now();
        wait_for_state(
            pipeline.upcast_ref(),
            gst::State::Paused,
            Duration::from_secs(3),
        )
        .expect("the delayed preroll should complete within the overall timeout");
        assert!(
            started.elapsed() >= Duration::from_millis(750),
            "the test pipeline should outlast the old 750 ms wait"
        );
        pipeline
            .set_state(gst::State::Null)
            .expect("the test pipeline should shut down");
    }

    #[core::prelude::v1::test]
    fn gstreamer_loads_and_returns_the_first_video_frame() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/clip.mp4");
        let mut player = GStreamerPlayer::new();
        let metadata = player
            .load(&path)
            .expect("GStreamer should load the H.264 fixture");
        assert!(metadata.duration_secs > 0.0);
        assert_eq!((metadata.width, metadata.height), (64, 48));
        let frame = player.next_frame().expect("the preroll frame is retained");
        assert_eq!((frame.width, frame.height), (64, 48));
        assert_eq!(frame.data.len(), 64 * 48 * 4);
        assert_eq!(player.frame_rate, Some((10, 1)));
    }

    #[core::prelude::v1::test]
    fn playback_advances_and_pause_holds_the_gstreamer_position() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/clip.mp4");
        let mut player = GStreamerPlayer::new();
        player
            .load(&path)
            .expect("GStreamer loads the H.264 fixture");
        let playbin = player
            .playbin
            .as_ref()
            .expect("load creates playbin")
            .clone();
        let before = player.position();

        player.play();
        wait_for_state(&playbin, gst::State::Playing, Duration::from_secs(3))
            .expect("playbin should enter the playing state");
        std::thread::sleep(Duration::from_millis(120));
        let after = player.position();
        assert!(
            after > before + 0.05,
            "video position should advance during playback: {before} -> {after}"
        );

        player.pause();
        wait_for_state(&playbin, gst::State::Paused, Duration::from_secs(3))
            .expect("playbin should enter the paused state");
        let paused_at = player.position();
        std::thread::sleep(Duration::from_millis(120));
        assert!(
            (player.position() - paused_at).abs() < 0.03,
            "paused position should hold: {paused_at} -> {}",
            player.position()
        );
        assert!(!player.is_playing(), "paused pipeline reports not playing");
    }

    #[core::prelude::v1::test]
    fn accurate_seek_discards_the_preroll_and_lands_near_the_target() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/clip.mp4");
        let mut player = GStreamerPlayer::new();
        player
            .load(&path)
            .expect("GStreamer loads the H.264 fixture");
        let first_frame = player.next_frame().expect("load retains the preroll frame");

        player.seek(1.3);
        let deadline = Instant::now() + Duration::from_secs(2);
        while Instant::now() < deadline && (player.position() - 1.3).abs() > 0.016 {
            std::thread::sleep(Duration::from_millis(10));
        }
        assert!(
            (player.position() - 1.3).abs() <= 0.016,
            "accurate seek should land near the requested time, got {}",
            player.position()
        );

        let deadline = Instant::now() + Duration::from_secs(2);
        let mut target_frame = player.next_frame();
        while target_frame.is_none() && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(10));
            target_frame = player.next_frame();
        }
        let target_frame = target_frame.expect("seek should produce a destination frame");
        assert_ne!(
            target_frame.data, first_frame.data,
            "the destination frame must replace the stale time-zero preroll"
        );
    }

    #[core::prelude::v1::test]
    fn frame_copy_skips_padding_between_bgra_rows() {
        let mut bytes = vec![0xEE; 4];
        bytes.extend(1u8..=12);
        bytes.extend([0xDD; 4]);
        bytes.extend(21u8..=32);
        bytes.extend([0xCC; 4]);
        let copied = copy_bgra_rows(&bytes, 3, 2, 16, 4)
            .expect("both padded rows contain three complete BGRA pixels");
        assert_eq!(copied, (1u8..=12).chain(21u8..=32).collect::<Vec<_>>());
    }

    #[core::prelude::v1::test]
    fn frame_mapping_honors_buffer_video_meta_layout() {
        initialize_gstreamer().expect("GStreamer should initialize");
        let info = gst_video::VideoInfo::builder(gst_video::VideoFormat::Bgra, 2, 2)
            .build()
            .expect("valid BGRA video info");
        let caps = info.to_caps().expect("video info converts to caps");
        let mut buffer = gst::Buffer::with_size(28).expect("custom padded frame buffer");
        {
            let mut map = buffer
                .get_mut()
                .expect("new buffer is writable")
                .map_writable()
                .expect("frame buffer maps writable");
            let mut bytes = vec![0xEE; 28];
            bytes[4..12].copy_from_slice(&(1u8..=8).collect::<Vec<_>>());
            bytes[16..24].copy_from_slice(&(21u8..=28).collect::<Vec<_>>());
            map.as_mut_slice().copy_from_slice(&bytes);
        }
        gst_video::VideoMeta::add_full(
            buffer.get_mut().expect("buffer stays writable"),
            gst_video::VideoFrameFlags::empty(),
            gst_video::VideoFormat::Bgra,
            2,
            2,
            &[4],
            &[12],
        )
        .expect("buffer accepts its custom video layout");
        let sample = gst::Sample::builder().caps(&caps).buffer(&buffer).build();

        let frame = GStreamerPlayer::frame_from_sample(&sample)
            .expect("sample maps using its VideoMeta layout");
        assert_eq!((frame.width, frame.height), (2, 2));
        assert_eq!(frame.data, (1u8..=8).chain(21u8..=28).collect::<Vec<_>>());
    }

    /// Regression: load errors named GStreamer in the playback-failed card.
    #[core::prelude::v1::test]
    fn load_errors_shown_to_the_user_are_plain_sentences() {
        let path =
            std::env::temp_dir().join(format!("lumina_not_a_video_{}.mp4", std::process::id()));
        std::fs::write(&path, b"this is not a video").expect("fixture writes");
        let error = GStreamerPlayer::new()
            .load(&path)
            .expect_err("a text file is not a video");
        let _ = std::fs::remove_file(&path);
        assert_eq!(
            error.to_string(),
            LOAD_FAILED_MESSAGE,
            "the user sees a plain sentence; the cause goes to the log"
        );
        assert!(
            !format!("{error:#}").contains("GStreamer"),
            "UI copy names no tool"
        );
    }

    /// Regression: `is_playing` popped only EOS, so a decode ERROR stayed on
    /// the bus and the clip kept "playing".
    #[core::prelude::v1::test]
    fn bus_error_stops_playback_and_is_reported_once() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/clip.mp4");
        let mut player = GStreamerPlayer::new();
        player
            .load(&path)
            .expect("GStreamer loads the H.264 fixture");
        player.play();
        assert!(player.is_playing(), "the clip plays before the error");
        player.post_test_error();
        assert!(!player.is_playing(), "a bus error stops playback");
        assert_eq!(
            player.take_playback_error().as_deref(),
            Some(PLAYBACK_FAILED_MESSAGE),
            "the error is reported as a plain sentence"
        );
        assert_eq!(
            player.take_playback_error(),
            None,
            "the error is reported once"
        );
        assert!(!player.is_playing(), "the stopped clip stays stopped");
    }

    /// Regression: `set_rate` always did a flushing seek to `position()`, so
    /// re-applying an unchanged rate right after a seek could undo it.
    #[core::prelude::v1::test]
    fn unchanged_rate_does_not_undo_a_seek_in_flight() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/clip.mp4");
        let mut player = GStreamerPlayer::new();
        player
            .load(&path)
            .expect("GStreamer loads the H.264 fixture");
        player.seek(1.3);
        player.set_rate(1.0);
        let deadline = Instant::now() + Duration::from_secs(2);
        while Instant::now() < deadline && (player.position() - 1.3).abs() > 0.05 {
            std::thread::sleep(Duration::from_millis(10));
        }
        std::thread::sleep(Duration::from_millis(200));
        assert!(
            (player.position() - 1.3).abs() <= 0.05,
            "an unchanged rate keeps the seek: {}",
            player.position()
        );
        player.set_rate(2.0);
        assert_eq!(player.rate(), 2.0, "a changed rate is applied");
    }

    /// Regression: `seek_to` used `seek_simple`, which always plays at 1.0,
    /// so a scrub or arrow key at 1.5x dropped to 1x while the menu said 1.5x.
    #[core::prelude::v1::test]
    fn seeks_keep_the_chosen_rate() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/clip.mp4");
        let mut player = GStreamerPlayer::new();
        player
            .load(&path)
            .expect("GStreamer loads the H.264 fixture");
        player.set_rate(1.5);
        assert_eq!(
            player.wait_for_segment_rate(1.5),
            Some(1.5),
            "1.5x is applied"
        );
        player.seek(0.5);
        let _ = player.next_frame();
        assert_eq!(
            player.wait_for_segment_rate(1.5),
            Some(1.5),
            "a seek keeps playing at the chosen rate"
        );
        player.step_frames(1);
        assert_eq!(
            player.wait_for_segment_rate(1.5),
            Some(1.5),
            "a frame step keeps playing at the chosen rate"
        );
    }

    /// Regression: a rate change sought to `position()`, which still reports
    /// the old time right after a seek, so the playhead jumped back.
    #[core::prelude::v1::test]
    fn rate_change_right_after_a_seek_keeps_the_seek() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/clip.mp4");
        let mut player = GStreamerPlayer::new();
        player
            .load(&path)
            .expect("GStreamer loads the H.264 fixture");
        player.seek(1.3);
        player.set_rate(1.5);
        let deadline = Instant::now() + Duration::from_secs(2);
        while Instant::now() < deadline && (player.position() - 1.3).abs() > 0.05 {
            std::thread::sleep(Duration::from_millis(10));
        }
        std::thread::sleep(Duration::from_millis(200));
        assert!(
            (player.position() - 1.3).abs() <= 0.05,
            "the rate change keeps the seek: {}",
            player.position()
        );
        assert_eq!(
            player.wait_for_segment_rate(1.5),
            Some(1.5),
            "1.5x is applied"
        );
    }

    #[core::prelude::v1::test]
    fn frame_step_uses_the_video_frame_rate() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/clip.mp4");
        let mut player = GStreamerPlayer::new();
        player
            .load(&path)
            .expect("GStreamer loads the H.264 fixture");
        let before = player.position();
        player.step_frames(1);
        let deadline = Instant::now() + Duration::from_secs(2);
        let mut after = player.position();
        while Instant::now() < deadline && (after - before).abs() < 0.05 {
            std::thread::sleep(Duration::from_millis(10));
            after = player.position();
        }
        assert!(
            (after - before - 0.1).abs() < 0.03,
            "one frame at 10 fps should seek about 100 ms: {before} -> {after}"
        );
    }
}
