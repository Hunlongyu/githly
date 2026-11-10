use bstr::ByteSlice;
use gpui_kit::component::ActiveTheme;
#[cfg(target_os = "windows")]
use gpui_kit::component::{Icon, IconName, h_flex};
#[cfg(target_os = "windows")]
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;

pub(super) const TITLE_BAR_HEIGHT: f32 = 40.;

pub(super) fn name_label(name: &str) -> String {
    const MAX_CHARACTERS: usize = 32;
    match name.as_bytes().grapheme_indices().nth(MAX_CHARACTERS) {
        Some((end, _, _)) => format!("{}…", &name[..end]),
        None => name.to_owned(),
    }
}

// Caption buttons have their own colors. In particular, the close button must
// not inherit the semantic danger color used for removed lines and error text.
#[cfg(target_os = "windows")]
pub(super) fn title_bar(content: impl IntoElement, window: &Window, cx: &App) -> AnyElement {
    h_flex()
        .id("window-title-bar")
        .test_support()
        .h(px(TITLE_BAR_HEIGHT))
        .w_full()
        .flex_shrink_0()
        .border_b_1()
        .border_color(cx.theme().title_bar_border)
        .bg(cx.theme().title_bar)
        .child(
            h_flex()
                .flex_1()
                .min_w_0()
                .h_full()
                // Balance the bottom divider so controls sit on the title bar's centerline.
                .pt(px(1.))
                .pl(px(12.))
                .window_control_area(WindowControlArea::Drag)
                .child(content),
        )
        .when(!window.is_fullscreen(), |bar| {
            bar.child(caption_button(
                "window-minimize",
                IconName::WindowMinimize,
                WindowControlArea::Min,
                false,
                cx,
            ))
            .child(caption_button(
                "window-maximize",
                if window.is_maximized() {
                    IconName::WindowRestore
                } else {
                    IconName::WindowMaximize
                },
                WindowControlArea::Max,
                false,
                cx,
            ))
            .child(caption_button(
                "window-close",
                IconName::WindowClose,
                WindowControlArea::Close,
                true,
                cx,
            ))
        })
        .into_any_element()
}

#[cfg(target_os = "windows")]
fn caption_button(
    id: &'static str,
    icon: IconName,
    area: WindowControlArea,
    close: bool,
    cx: &App,
) -> impl IntoElement {
    // VS Code titlebarpart.css: close hover rgba(232, 17, 35, .9), white glyph.
    let hover = if close {
        rgba(0xe81123e6)
    } else if cx.theme().mode.is_dark() {
        rgba(0xffffff1a)
    } else {
        rgba(0x0000001a)
    };
    let foreground = if close {
        Hsla::white()
    } else {
        cx.theme().foreground
    };
    h_flex()
        .id(id)
        .w(px(46.))
        .h_full()
        .flex_shrink_0()
        .justify_center()
        .text_color(cx.theme().foreground)
        .hover(move |style| style.bg(hover).text_color(foreground))
        .active(move |style| style.bg(hover).text_color(foreground))
        // Windows handles click, drag, double click, and snap hit testing.
        .window_control_area(area)
        .child(Icon::new(icon).size(px(14.)))
}

#[cfg(not(target_os = "windows"))]
pub(super) fn title_bar(content: impl IntoElement, _: &Window, cx: &App) -> AnyElement {
    gpui_kit::component::TitleBar::new()
        .h(px(TITLE_BAR_HEIGHT))
        .bg(cx.theme().title_bar)
        .child(content)
        .into_any_element()
}

#[cfg(test)]
mod tests {
    #[test]
    fn name_label_shortens_only_after_32_complete_characters() {
        let at_limit = format!("{}👩‍💻", "项".repeat(31));
        assert_eq!(super::name_label(&at_limit), at_limit);
        assert_eq!(
            super::name_label(&format!("{at_limit}目")),
            format!("{at_limit}…")
        );
    }
}
