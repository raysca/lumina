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
