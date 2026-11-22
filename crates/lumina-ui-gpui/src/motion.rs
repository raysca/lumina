//! Motion (v3.1 motion sheet): three durations, nothing over 220 ms.
//! Entrances ease out; exits are faster and ease in, so closing never feels
//! sticky. Reduce Motion keeps fades and drops movement and scale.
//! Overlays and the fullscreen viewer fade in with [`overlay_in`]; overlays
//! and toasts fade out through [`Exits`] and [`fade_out`]. Stepping between
//! viewer items does not animate. Every animation goes through this file
//! (a theme lint enforces it).

use std::any::Any;
use std::collections::HashMap;
use std::time::{Duration, Instant};

use gpui::*;

/// Exits, hover-revealed controls, icon swaps, fresh thumbnails.
pub const MOTION_FAST: Duration = Duration::from_millis(120);
/// Overlay entrances, indicators, cross-fades.
pub const MOTION_BASE: Duration = Duration::from_millis(160);
/// Panel widths, the viewer zoom, slideshow slides.
pub const MOTION_SLOW: Duration = Duration::from_millis(220);

/// Menus, dialogs, sheets and the palette.
const OVERLAY_IN: Duration = MOTION_BASE;
/// Every close: overlays, menus and toasts.
pub const EXIT: Duration = MOTION_FAST;
/// A toast rises 8px while it fades in.
pub const TOAST_IN: Duration = Duration::from_millis(180);
/// A freshly decoded grid thumbnail; cached ones appear at once.
pub const THUMB_IN: Duration = MOTION_FAST;
/// The immersive viewer HUD coming back.
pub const HUD_IN: Duration = MOTION_FAST;
/// The star or heart the user just set.
const POP: Duration = Duration::from_millis(140);
/// How long a just-set mark still pops. Tests get a generous window: a
/// loaded parallel run can spend longer than `POP * 2` on one render.
const POP_LIVE: Duration = if cfg!(test) {
    Duration::from_secs(60)
} else {
    Duration::from_millis(280)
};

fn entrance(duration: Duration) -> Animation {
    Animation::new(duration).with_easing(ease_out_quint())
}

/// Exits accelerate away: slow start, quick finish.
fn ease_in_cubic(t: f32) -> f32 {
    t * t * t
}

fn exit() -> Animation {
    Animation::new(EXIT).with_easing(ease_in_cubic)
}

/// Whether closes fade out. Always in the app; off by default in tests,
/// which read resting layout and would otherwise see overlays linger.
pub fn exits_on() -> bool {
    #[cfg(test)]
    {
        EXITS.with(|on| on.get())
    }
    #[cfg(not(test))]
    {
        true
    }
}

#[cfg(test)]
thread_local! {
    /// Off by default in tests; exit tests turn it on.
    pub(crate) static EXITS: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// The system's Reduce Motion setting, read once.
pub fn reduce_motion() -> bool {
    #[cfg(test)]
    {
        REDUCE_MOTION.with(|on| on.get())
    }
    #[cfg(not(test))]
    {
        static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
        *ON.get_or_init(system_reduce_motion)
    }
}

#[cfg(all(not(test), target_os = "macos"))]
fn system_reduce_motion() -> bool {
    extern "C" {
        fn lumina_reduce_motion() -> std::os::raw::c_int;
    }
    // SAFETY: takes no arguments and only reads an NSWorkspace property.
    unsafe { lumina_reduce_motion() != 0 }
}

#[cfg(all(not(test), not(target_os = "macos")))]
fn system_reduce_motion() -> bool {
    false
}

#[cfg(test)]
thread_local! {
    /// On by default in tests: the toast rise is wall-clock timed and moves
    /// layout, and tests measure resting positions. Motion tests turn it off.
    pub(crate) static REDUCE_MOTION: std::cell::Cell<bool> = const { std::cell::Cell::new(true) };
}

/// Fades `el` in when it first renders under `id`. A fade is not motion, so
/// it plays under Reduce Motion too.
pub fn fade_in<E: IntoElement + Styled + 'static>(
    el: E,
    id: impl Into<ElementId>,
    duration: Duration,
) -> AnimationElement<E> {
    el.with_animation(id, entrance(duration), |el, t| el.opacity(t))
}

/// A full-window overlay (menu, dialog, sheet, palette) that fades in each
/// time it opens. The wrapper fills the window and has no hitbox, so it
/// changes neither layout nor clicks.
pub fn overlay_in(open: bool, el: impl IntoElement, id: &'static str) -> AnyElement {
    if !open {
        return el.into_any_element();
    }
    fade_in(div().absolute().inset_0().child(el), id, OVERLAY_IN).into_any_element()
}

/// Fades a closed element out over [`EXIT`], replaying for each new `id`.
/// A layer over it swallows the pointer, so the copy fading out takes no
/// clicks or hovers: the close has already taken effect.
pub fn fade_out(el: impl IntoElement, id: impl Into<ElementId>) -> AnyElement {
    div()
        .relative()
        .child(el)
        .child(div().absolute().inset_0().occlude())
        .with_animation(id, exit(), |el, t| el.opacity(1.0 - t))
        .into_any_element()
}

/// A full-window overlay fading out after it closed (see [`Exits`]).
pub fn overlay_out(el: impl IntoElement, id: ElementId) -> AnyElement {
    div()
        .absolute()
        .inset_0()
        .child(el)
        .child(div().absolute().inset_0().occlude())
        .with_animation(id, exit(), |el, t| el.opacity(1.0 - t))
        .into_any_element()
}

/// What an overlay last showed, so it can fade out after its state is
/// gone. Each frame an open overlay leaves a copy of its state; the frame
/// it closes, that copy is rendered once more, fading out for [`EXIT`].
#[derive(Default)]
pub struct Exits {
    slots: HashMap<&'static str, ExitSlot>,
}

#[derive(Default)]
struct ExitSlot {
    last_open: Option<Box<dyn Any>>,
    closing: Option<(Box<dyn Any>, Instant)>,
    generation: u64,
}

impl Exits {
    /// Call once a frame per overlay with its state while open, `None`
    /// once closed. Returns the state to fade out, the fade's id, and
    /// whether the close happened this frame, while the overlay is closing.
    pub fn track<T: Clone + 'static>(
        &mut self,
        id: &'static str,
        open: Option<T>,
    ) -> Option<(T, ElementId, bool)> {
        let slot = self.slots.entry(id).or_default();
        if let Some(state) = open {
            slot.last_open = Some(Box::new(state));
            slot.closing = None;
            return None;
        }
        let mut started = false;
        if let Some(last) = slot.last_open.take() {
            if exits_on() {
                slot.generation += 1;
                slot.closing = Some((last, Instant::now()));
                started = true;
            }
        }
        let (state, at) = slot.closing.as_ref()?;
        if at.elapsed() >= EXIT {
            slot.closing = None;
            return None;
        }
        let state = state.downcast_ref::<T>()?.clone();
        Some((
            state,
            ElementId::NamedInteger(format!("{id}-out").into(), slot.generation),
            started,
        ))
    }
}

/// Dips `el` to half opacity and back in over [`MOTION_BASE`] each time
/// `generation` changes: the grid after a zoom or a long jump, so the new
/// layout reads as a change rather than a jump cut. Nothing at generation 0.
pub fn dip_in<E: IntoElement + Styled + 'static>(
    el: E,
    id: &'static str,
    generation: u64,
) -> AnyElement {
    if generation == 0 {
        return el.into_any_element();
    }
    el.with_animation(
        ElementId::NamedInteger(id.into(), generation),
        entrance(MOTION_BASE),
        |el, t| el.opacity(0.5 + 0.5 * t),
    )
    .into_any_element()
}

/// How far a programmatic scroll has got at `t` in 0..=1 (entrance curve).
pub fn scroll_progress(t: f32) -> f32 {
    ease_out_quint()(t.clamp(0.0, 1.0))
}

/// The sidebar and right column sliding to new widths. Their targets
/// change at once; for [`MOTION_SLOW`] after, each is still `slack` wider
/// (or narrower, if negative) than its target, easing to zero. The grid
/// lays out against the slid widths, so it resizes with the panels.
/// Under Reduce Motion panels snap.
#[derive(Clone, Copy, Default)]
pub struct PanelSlide {
    from: (f32, f32),
    at: Option<Instant>,
}

impl PanelSlide {
    /// Starts a slide from the (sidebar, right column) widths `before` to
    /// `after`, carrying on from wherever a running slide has reached.
    pub fn start(&mut self, before: (f32, f32), after: (f32, f32)) {
        if reduce_motion() {
            *self = Self::default();
            return;
        }
        let now = self.slack();
        self.from = (before.0 + now.0 - after.0, before.1 + now.1 - after.1);
        self.at = Some(Instant::now());
    }

    /// How much wider than its target each panel still is.
    pub fn slack(&self) -> (f32, f32) {
        let Some(at) = self.at else {
            return (0.0, 0.0);
        };
        let t = at.elapsed().as_secs_f32() / MOTION_SLOW.as_secs_f32();
        if t >= 1.0 {
            return (0.0, 0.0);
        }
        let left = 1.0 - ease_out_quint()(t);
        (self.from.0 * left, self.from.1 * left)
    }

    /// True while a slide is playing.
    pub fn active(&self) -> bool {
        self.at.is_some_and(|at| at.elapsed() < MOTION_SLOW)
    }
}

/// A segmented control's selected-segment pill gliding to a newly
/// selected segment: its left edge and width ease from where the pill
/// was to the new segment over [`MOTION_BASE`]. Bounds are relative to the
/// control, so moving the whole control never slides the pill. Under
/// Reduce Motion it snaps.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PillSlide {
    from: Bounds<Pixels>,
    to: Bounds<Pixels>,
    at: Option<Instant>,
}

impl PillSlide {
    /// A pill resting on `to`.
    pub fn new(to: Bounds<Pixels>) -> Self {
        Self {
            from: to,
            to,
            at: None,
        }
    }

    /// Moves the target to `to` (starting a slide from wherever the pill is
    /// if it changed) and returns where the pill is at `now`.
    pub fn place(&mut self, to: Bounds<Pixels>, now: Instant) -> Bounds<Pixels> {
        let moved = |a: Pixels, b: Pixels| (f32::from(a) - f32::from(b)).abs() > 0.5;
        if moved(to.origin.x, self.to.origin.x)
            || moved(to.origin.y, self.to.origin.y)
            || moved(to.size.width, self.to.size.width)
            || moved(to.size.height, self.to.size.height)
        {
            self.from = self.at(now);
            self.at = (!reduce_motion()).then_some(now);
        }
        self.to = to;
        self.at(now)
    }

    /// Where the pill is at `now`.
    pub fn at(&self, now: Instant) -> Bounds<Pixels> {
        let Some(at) = self.at else {
            return self.to;
        };
        let t = now.saturating_duration_since(at).as_secs_f32() / MOTION_BASE.as_secs_f32();
        if t >= 1.0 {
            return self.to;
        }
        let k = ease_out_quint()(t);
        let mix = |a: Pixels, b: Pixels| px(f32::from(a) + (f32::from(b) - f32::from(a)) * k);
        Bounds {
            origin: point(
                mix(self.from.origin.x, self.to.origin.x),
                mix(self.from.origin.y, self.to.origin.y),
            ),
            size: size(
                mix(self.from.size.width, self.to.size.width),
                mix(self.from.size.height, self.to.size.height),
            ),
        }
    }

    /// True while a slide is playing at `now`.
    pub fn active(&self, now: Instant) -> bool {
        self.at
            .is_some_and(|at| now.saturating_duration_since(at) < MOTION_BASE)
    }
}

/// Where a control's selected segment is, filled during prepaint.
type SegmentSlot = std::rc::Rc<std::cell::Cell<Option<Bounds<Pixels>>>>;

thread_local! {
    /// Each segmented control's pill, by the control's id.
    static PILLS: std::cell::RefCell<HashMap<ElementId, PillSlide>> =
        std::cell::RefCell::new(HashMap::new());
    /// Where the control being prepainted wants its selected segment's
    /// bounds; [`segment_mark`] fills it.
    static PILL_SLOT: std::cell::RefCell<Option<SegmentSlot>> =
        const { std::cell::RefCell::new(None) };
}

/// Paints a segmented control's selected-segment pill. Goes first among
/// the control's children (it is absolute, so it takes no room); the
/// selected segment carries a [`segment_mark`]. Prepaint runs over the
/// whole window before paint, so by the time this paints, the mark has
/// recorded where the selected segment is. `radius` picks the corner
/// radius from the segment's height.
pub fn segment_pill(
    control: ElementId,
    color: impl Into<Background> + 'static,
    radius: fn(Pixels) -> Pixels,
) -> impl IntoElement {
    canvas(
        |frame, _window, _cx| {
            let slot = std::rc::Rc::new(std::cell::Cell::new(None));
            PILL_SLOT.with(|s| *s.borrow_mut() = Some(slot.clone()));
            (frame, slot)
        },
        move |_, (frame, slot), window, _cx| {
            let Some(segment) = slot.get() else {
                // Nothing selected: the next selection appears in place.
                PILLS.with(|pills| pills.borrow_mut().remove(&control));
                return;
            };
            let to = Bounds {
                origin: point(
                    segment.origin.x - frame.origin.x,
                    segment.origin.y - frame.origin.y,
                ),
                size: segment.size,
            };
            let now = Instant::now();
            let (here, sliding) = PILLS.with(|pills| {
                let mut pills = pills.borrow_mut();
                let pill = pills.entry(control).or_insert_with(|| PillSlide::new(to));
                (pill.place(to, now), pill.active(now))
            });
            if sliding {
                window.request_animation_frame();
            }
            let bounds = Bounds {
                origin: point(
                    here.origin.x + frame.origin.x,
                    here.origin.y + frame.origin.y,
                ),
                size: here.size,
            };
            window.paint_quad(fill(bounds, color).corner_radii(radius(here.size.height)));
        },
    )
    .absolute()
    .inset_0()
}

/// Marks the selected segment of a segmented control so the control's
/// [`segment_pill`] knows where to go.
pub fn segment_mark() -> impl IntoElement {
    canvas(
        |bounds, _window, _cx| {
            PILL_SLOT.with(|s| {
                if let Some(slot) = s.borrow().as_ref() {
                    slot.set(Some(bounds));
                }
            });
        },
        |_, _, _, _| {},
    )
    .absolute()
    .inset_0()
}

/// A segmented control's pill, for tests.
#[cfg(test)]
pub(crate) fn pill(control: impl Into<ElementId>) -> Option<PillSlide> {
    let control = control.into();
    PILLS.with(|pills| pills.borrow().get(&control).copied())
}

#[cfg(test)]
impl PillSlide {
    /// Where the last slide started and ends, relative to the control, and
    /// whether it is animated.
    pub(crate) fn ends(&self) -> (Bounds<Pixels>, Bounds<Pixels>, bool) {
        (self.from, self.to, self.at.is_some())
    }
}

/// Fades `el` in while it rises 8px into place; only fades under Reduce
/// Motion. `el` must not set its own `top`.
pub fn rise_in<E: IntoElement + Styled + 'static>(
    el: E,
    id: impl Into<ElementId>,
    duration: Duration,
) -> AnimationElement<E> {
    let rise = rise_px();
    el.with_animation(id, entrance(duration), move |el, t| {
        el.opacity(t).relative().top(px(rise * (1.0 - t)))
    })
}

/// How far a toast rises: none under Reduce Motion.
fn rise_px() -> f32 {
    if reduce_motion() {
        0.0
    } else {
        8.0
    }
}

/// Scale through a pop: 1 → 1.18 → 1 over `t` in 0..=1.
fn pop_scale(t: f32) -> f32 {
    1.0 + 0.18 * (1.0 - (2.0 * t - 1.0).abs())
}

/// The star or heart the user just set, so only that mark pops. Each set
/// bumps `generation`, which gives the pop a fresh id and replays it. The pop
/// is only offered for `POP_LIVE` after the set, so coming back to the item
/// later does not replay it.
#[derive(Clone, Default)]
pub struct MarkPop {
    item: Option<lumina_core::MediaId>,
    star: u8,
    favorite: bool,
    generation: u64,
    at: Option<Instant>,
}

impl MarkPop {
    pub fn rated(&mut self, item: Option<lumina_core::MediaId>, star: u8) {
        *self = Self {
            item,
            star,
            favorite: false,
            generation: self.generation + 1,
            at: Some(Instant::now()),
        };
    }

    pub fn favorited(&mut self, item: Option<lumina_core::MediaId>) {
        *self = Self {
            item,
            star: 0,
            favorite: true,
            generation: self.generation + 1,
            at: Some(Instant::now()),
        };
    }

    /// True for a just-set mark on `item`.
    fn live(&self, item: Option<lumina_core::MediaId>) -> bool {
        self.item == item && self.at.is_some_and(|at| at.elapsed() < POP_LIVE)
    }

    /// The star just set on `item` and its pop id.
    pub fn star(&self, item: Option<lumina_core::MediaId>) -> Option<(u8, ElementId)> {
        (self.star > 0 && self.live(item)).then(|| {
            (
                self.star,
                ElementId::NamedInteger("star-pop".into(), self.generation),
            )
        })
    }

    /// The pop id for the heart on `item`, if it was just toggled.
    pub fn heart(&self, item: Option<lumina_core::MediaId>) -> Option<ElementId> {
        (self.favorite && self.live(item))
            .then(|| ElementId::NamedInteger("heart-pop".into(), self.generation))
    }
}

/// The pop to play, if any: none under Reduce Motion.
fn pop_id(id: Option<ElementId>) -> Option<ElementId> {
    id.filter(|_| !reduce_motion())
}

/// Pops an icon once when `id` is set (see `MarkPop`). Static under Reduce
/// Motion.
pub fn pop(icon: Svg, id: Option<ElementId>) -> AnyElement {
    let Some(id) = pop_id(id) else {
        return icon.into_any_element();
    };
    icon.with_animation(id, entrance(POP), |icon, t| {
        let k = pop_scale(t);
        icon.with_transformation(Transformation::scale(size(k, k)))
    })
    .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[core::prelude::v1::test]
    fn a_pop_grows_then_settles_back() {
        assert_eq!(pop_scale(0.0), 1.0, "starts at rest");
        assert!((pop_scale(0.5) - 1.18).abs() < 1e-6, "peaks at 1.18");
        assert_eq!(pop_scale(1.0), 1.0, "ends at rest, so it never sticks");
    }

    #[core::prelude::v1::test]
    fn only_the_mark_just_set_pops_and_each_set_replays_it() {
        use lumina_core::MediaId;
        let (a, b) = (Some(MediaId(1)), Some(MediaId(2)));
        let mut pop = MarkPop::default();
        assert!(pop.star(a).is_none(), "nothing pops before a rating is set");

        pop.rated(a, 3);
        let (star, first) = pop.star(a).expect("the star just set pops");
        assert_eq!(star, 3, "only the touched star");
        assert!(pop.star(b).is_none(), "another item's stars stay still");
        assert!(pop.heart(a).is_none(), "a rating does not pop the heart");

        pop.rated(a, 3);
        let (_, again) = pop.star(a).unwrap();
        assert_ne!(first, again, "setting it again replays the pop");

        pop.favorited(a);
        assert!(pop.heart(a).is_some(), "the heart pops when toggled");
        assert!(pop.star(a).is_none(), "and the stars do not");

        pop.at = pop.at.map(|at| at - POP_LIVE);
        assert!(
            pop.heart(a).is_none(),
            "coming back to the item later does not replay the pop"
        );
    }

    #[core::prelude::v1::test]
    fn reduce_motion_keeps_fades_but_drops_rise_and_pop() {
        let id = || Some(ElementId::Name("star".into()));
        REDUCE_MOTION.with(|on| on.set(false));
        assert_eq!(rise_px(), 8.0);
        assert!(pop_id(id()).is_some());
        REDUCE_MOTION.with(|on| on.set(true));
        assert_eq!(rise_px(), 0.0, "a toast only fades under Reduce Motion");
        assert!(
            pop_id(id()).is_none(),
            "marks do not pop under Reduce Motion"
        );
    }

    #[core::prelude::v1::test]
    fn nothing_runs_longer_than_220ms() {
        for d in [
            OVERLAY_IN,
            TOAST_IN,
            THUMB_IN,
            HUD_IN,
            POP,
            EXIT,
            MOTION_SLOW,
        ] {
            assert!(d <= Duration::from_millis(220), "{d:?} is over budget");
        }
    }

    #[core::prelude::v1::test]
    fn panels_slide_from_their_old_width_to_the_new_one() {
        REDUCE_MOTION.with(|on| on.set(false));
        let mut slide = PanelSlide::default();
        assert_eq!(slide.slack(), (0.0, 0.0), "at rest");
        slide.start((244.0, 0.0), (48.0, 324.0));
        let (sidebar, right) = slide.slack();
        assert!(
            (sidebar - 196.0).abs() < 2.0 && (right + 324.0).abs() < 2.0,
            "starts at the old widths: {sidebar} {right}"
        );
        assert!(slide.active());
        slide.at = slide.at.map(|at| at - MOTION_SLOW);
        assert_eq!(slide.slack(), (0.0, 0.0), "lands on the new widths");
        assert!(!slide.active());

        REDUCE_MOTION.with(|on| on.set(true));
        slide.start((244.0, 0.0), (48.0, 324.0));
        assert_eq!(slide.slack(), (0.0, 0.0), "snaps under Reduce Motion");
    }

    #[core::prelude::v1::test]
    fn exits_are_faster_than_entrances_and_ease_in() {
        assert!(EXIT < OVERLAY_IN, "closing is quicker than opening");
        assert!(ease_in_cubic(0.5) < 0.5, "an exit starts slow");
        assert_eq!(ease_in_cubic(1.0), 1.0, "and lands fully closed");
    }

    #[core::prelude::v1::test]
    fn a_closed_overlay_fades_its_last_state_then_goes() {
        EXITS.with(|on| on.set(true));
        let mut exits = Exits::default();
        assert!(exits.track("menu", Some(1u8)).is_none(), "open: no exit");
        assert!(exits.track("menu", Some(2u8)).is_none());
        let (state, first, started) = exits.track::<u8>("menu", None).expect("fades out");
        assert_eq!(state, 2, "the last state it showed");
        assert!(started, "the close is reported once");
        let (_, _, started) = exits.track::<u8>("menu", None).expect("still fading");
        assert!(!started);
        exits
            .slots
            .get_mut("menu")
            .unwrap()
            .closing
            .as_mut()
            .unwrap()
            .1 -= EXIT;
        assert!(exits.track::<u8>("menu", None).is_none(), "gone after EXIT");

        exits.track("menu", Some(3u8));
        let (_, again, _) = exits.track::<u8>("menu", None).unwrap();
        assert_ne!(first, again, "each close replays the fade");
        assert!(
            exits.track("menu", Some(4u8)).is_none(),
            "reopening cancels it"
        );
        assert!(exits.track::<u8>("menu", None).is_some());

        EXITS.with(|on| on.set(false));
        exits.track("menu", Some(5u8));
        assert!(
            exits.track::<u8>("menu", None).is_none(),
            "with exits off a close is instant"
        );
    }
}
#[core::prelude::v1::test]
fn the_segment_pill_glides_to_a_new_segment_and_snaps_under_reduce_motion() {
    let seg = |x: f32, w: f32| Bounds {
        origin: point(px(x), px(3.0)),
        size: size(px(w), px(28.0)),
    };
    let start = Instant::now();
    REDUCE_MOTION.with(|on| on.set(false));
    let mut pill = PillSlide::new(seg(3.0, 40.0));
    assert_eq!(pill.place(seg(3.0, 40.0), start), seg(3.0, 40.0));
    assert!(!pill.active(start), "a resting pill does not animate");
    assert_eq!(
        pill.place(seg(3.2, 40.0), start),
        seg(3.2, 40.0),
        "a sub-pixel relayout is not a slide"
    );
    assert!(!pill.active(start));

    let at = pill.place(seg(45.0, 60.0), start);
    assert_eq!(
        at,
        seg(3.2, 40.0),
        "a new selection starts where the pill was"
    );
    assert!(pill.active(start));
    let mid = pill.at(start + MOTION_BASE / 2);
    let x = f32::from(mid.origin.x);
    let w = f32::from(mid.size.width);
    assert!(x > 3.2 && x < 45.0, "halfway it is between: {x}");
    assert!(w > 40.0 && w < 60.0, "and so is its width: {w}");
    assert!(x > 24.0, "easing out, most of the way there: {x}");
    let done = start + MOTION_BASE;
    assert_eq!(pill.at(done), seg(45.0, 60.0));
    assert!(!pill.active(done));

    // Reselecting mid-slide carries on from where the pill is.
    let mut pill = PillSlide::new(seg(3.0, 40.0));
    pill.place(seg(45.0, 60.0), start);
    let half = start + MOTION_BASE / 2;
    let here = pill.at(half);
    assert_eq!(pill.place(seg(3.0, 40.0), half), here);

    REDUCE_MOTION.with(|on| on.set(true));
    let mut pill = PillSlide::new(seg(3.0, 40.0));
    assert_eq!(pill.place(seg(45.0, 60.0), start), seg(45.0, 60.0));
    assert!(!pill.active(start), "Reduce Motion snaps");
}
