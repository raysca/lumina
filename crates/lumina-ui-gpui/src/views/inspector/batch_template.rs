//! TEMPLATE section of the batch inspector: pick a metadata template and
//! apply it to the whole selection. The keyboard path is the palette's
//! Apply Metadata Template….

use gpui::*;

use crate::i18n::t;
use crate::theme;
use crate::LuminaApp;

pub fn render_template_section(app: &LuminaApp, weak_app: WeakEntity<LuminaApp>) -> Div {
    let templates = &app.config.templates;
    let row = div().flex().flex_row().items_center().gap_2();
    let button = |id: &'static str, label: String| {
        div()
            .id(id)
            .debug_selector(move || id.to_string())
            .role(Role::Button)
            .h(px(26.0))
            .px_2()
            .rounded(theme::RADIUS_MD)
            .bg(theme::bg_2())
            .hover(|s| s.bg(theme::bg_3()))
            .cursor_pointer()
            .flex()
            .items_center()
            .text_size(theme::TEXT_SMALL)
            .text_color(theme::t1())
            .child(label)
    };
    let manage = weak_app.clone();
    let manage_button = button("batch-template-manage", t("command.manage_templates"))
        .aria_label(t("command.manage_templates"))
        .tooltip(theme::tooltip_label(t("command.manage_templates")))
        .on_click(move |_e, _w, cx| {
            if let Some(app) = manage.upgrade() {
                app.update(cx, |this, cx| this.open_template_editor(cx));
            }
        });
    if templates.is_empty() {
        return row
            .child(
                div()
                    .text_size(theme::TEXT_SMALL)
                    .text_color(theme::t2())
                    .child(t("template.none_yet")),
            )
            .child(manage_button);
    }
    let index = app.templates.picked.min(templates.len() - 1);
    let picked = &templates[index];
    let (cycle, apply) = (weak_app.clone(), weak_app);
    row.child(
        button("batch-template-picker", picked.name.clone())
            .flex_1()
            .min_w_0()
            .aria_label(t("template.picker_label").replace("{name}", &picked.name))
            .tooltip(theme::tooltip_label(
                t("template.picker_label").replace("{name}", &picked.name),
            ))
            .on_click(move |_e, _w, cx| {
                if let Some(app) = cycle.upgrade() {
                    app.update(cx, |this, cx| this.cycle_picked_template(cx));
                }
            }),
    )
    .child(
        button("batch-template-apply", t("template.apply"))
            .aria_label(t("template.apply"))
            .tooltip(theme::tooltip_label(t("command.apply_template")))
            .on_click(move |_e, _w, cx| {
                if let Some(app) = apply.upgrade() {
                    app.update(cx, |this, cx| this.apply_template(index, cx));
                }
            }),
    )
    .child(manage_button)
}
