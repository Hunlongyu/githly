//! Shared dimensions for the compact native workbench.
use gpui_kit::component::{
    Icon, IconName, Sizable,
    button::{Button, ButtonVariants},
    input::{Input, InputState},
    tab::TabBar,
};
use gpui_kit::*;

pub(super) const CONTROL_HEIGHT: f32 = 28.;
pub(super) const TOOLBAR_HEIGHT: f32 = 40.;

pub(super) fn search_input(state: &Entity<InputState>) -> Input {
    Input::new(state)
        .small()
        // Input::h only sizes multiline editors; constrain the single-line frame.
        .min_h(px(CONTROL_HEIGHT))
        .max_h(px(CONTROL_HEIGHT))
        .py(px(3.))
        .text_size(px(12.))
        .line_height(px(18.))
        .prefix(Icon::new(IconName::Search).size(px(14.)))
        .cleanable(true)
}

pub(super) fn button(id: impl Into<ElementId>) -> Button {
    Button::new(id).small().h(px(CONTROL_HEIGHT)).px(px(10.))
}

pub(super) fn icon_button(id: impl Into<ElementId>, icon: impl Into<Icon>) -> Button {
    button(id)
        .ghost()
        .icon(Icon::new(icon))
        .w(px(CONTROL_HEIGHT))
        .px_0()
}

pub(super) fn segmented(id: impl Into<ElementId>) -> TabBar {
    TabBar::new(id).segmented().with_size(px(CONTROL_HEIGHT))
}
