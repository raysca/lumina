//! The blinking caret of the one-line text fields (see app/text_input.rs).
//! Fields draw the caret as a trailing glyph; while one takes typing the
//! glyph blinks, and it stays solid while the text is changing.

use std::cell::Cell;
use std::time::Duration;

use gpui::*;

use crate::app::LuminaApp;

/// Half a blink: how long the caret stays shown, then hidden.
pub(crate) const CARET_BLINK: Duration = Duration::from_millis(500);

/// Blink state, reset whenever no field takes typing.
#[derive(Default)]
pub(crate) struct CaretBlink {
    hidden: bool,
    /// The field's text at the last tick; a change keeps the caret solid.
    last_text: Option<String>,
    task: Option<Task<()>>,
}

// ponytail: the field renderers get destructured state, not the app, so
// render publishes this frame's glyph here; the UI runs on one thread.
thread_local! {
    static SHOWN: Cell<bool> = const { Cell::new(true) };
}

/// This frame's caret: "|" or, while blinked off, a space of about its width.
pub(crate) fn caret() -> &'static str {
    if SHOWN.with(Cell::get) {
        "|"
    } else {
        " "
    }
}

impl LuminaApp {
    /// Runs at the top of render: starts the blink when a field takes
    /// typing, stops it when none does, and publishes the glyph.
    pub(crate) fn update_caret(&mut self, cx: &mut Context<Self>) {
        if self.text_target().is_none() {
            self.caret = CaretBlink::default();
        } else if self.caret.task.is_none() {
            self.caret.task = Some(cx.spawn(async move |this, cx| loop {
                cx.background_executor().timer(CARET_BLINK).await;
                if this.update(cx, |app, cx| app.blink_caret(cx)).is_err() {
                    break;
                }
            }));
        }
        SHOWN.with(|shown| shown.set(!self.caret.hidden));
    }

    fn blink_caret(&mut self, cx: &mut Context<Self>) {
        let text = self
            .text_target()
            .and_then(|target| self.text_buffer(target))
            .cloned();
        if text.is_some() && text == self.caret.last_text {
            self.caret.hidden = !self.caret.hidden;
        } else {
            self.caret.hidden = false;
            self.caret.last_text = text;
        }
        cx.notify();
    }
}
