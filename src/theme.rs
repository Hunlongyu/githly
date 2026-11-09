//! Native workbench palette based on VS Code's 2026 Light / Dark themes.
//! Source and attribution: docs/native-development.md.
use crate::config::Appearance;
use gpui_kit::component::{Theme, ThemeMode};
use gpui_kit::{App, Hsla, Window, WindowAppearance, px, rgb, rgba};

impl Appearance {
    pub fn resolve(self, system: WindowAppearance) -> ThemeMode {
        match self {
            Self::System => system.into(),
            Self::Light => ThemeMode::Light,
            Self::Dark => ThemeMode::Dark,
        }
    }
}

pub fn apply(mode: ThemeMode, window: Option<&mut Window>, cx: &mut App) {
    Theme::change(mode, window, cx);
    Theme::update(cx, |theme| {
        let dark = mode.is_dark();
        let color = |dark_value, light_value| -> Hsla {
            rgb(if dark { dark_value } else { light_value }).into()
        };
        let editor = color(0x121314, 0xffffff);
        let surface = color(0x191a1b, 0xfafafd);
        let elevated = color(0x202122, 0xfafafd);
        let border = color(0x303234, 0xdedfe3);
        let text = color(0xbfbfbf, 0x202020);
        let muted = color(0x8c8c8c, 0x606060);
        let hover = color(0x292a2b, 0xededf0);
        let selected = color(0x333435, 0xdedee1);
        let accent = color(0x3994bc, 0x0069cc);
        let primary = color(0x297aa0, 0x0069cc);
        let primary_hover = color(0x2b7da3, 0x0063c1);
        let c = &mut theme.colors;
        c.background = editor;
        c.foreground = text;
        c.border = border;
        c.title_bar = surface;
        c.title_bar_border = border;
        c.sidebar = surface;
        c.sidebar_border = border;
        c.sidebar_foreground = text;
        c.sidebar_accent = selected;
        c.sidebar_accent_foreground = text;
        c.sidebar_primary = primary;
        c.sidebar_primary_foreground = rgb(0xffffff).into();
        c.muted = color(0x242526, 0xeaeaea);
        c.muted_foreground = muted;
        c.accent = selected;
        c.accent_foreground = text;
        c.popover = elevated;
        c.popover_foreground = text;
        c.input = border;
        c.caret = text;
        c.ring = accent;
        c.selection = color(0x276782, 0xbfd9f2);
        c.list = editor;
        c.list_head = surface;
        c.list_even = editor;
        c.list_hover = hover;
        c.list_active = selected;
        c.list_active_border = accent;
        c.primary = primary;
        c.primary_hover = primary_hover;
        c.primary_active = primary_hover;
        c.primary_foreground = rgb(0xffffff).into();
        c.button_primary = primary;
        c.button_primary_hover = primary_hover;
        c.button_primary_active = primary_hover;
        c.button_primary_foreground = rgb(0xffffff).into();
        c.button = surface;
        c.button_foreground = text;
        c.button_hover = hover;
        c.button_active = selected;
        c.secondary = surface;
        c.secondary_foreground = text;
        c.secondary_hover = hover;
        c.secondary_active = selected;
        c.button_secondary = surface;
        c.button_secondary_foreground = text;
        c.button_secondary_hover = hover;
        c.button_secondary_active = selected;
        c.tab = surface;
        c.tab_bar = surface;
        c.tab_bar_segmented = color(0x242627, 0xeff0f2);
        c.tab_foreground = muted;
        c.tab_active = color(0x383a3c, 0xffffff);
        c.tab_active_foreground = text;
        c.table = editor;
        c.table_head = surface;
        c.table_head_foreground = muted;
        c.table_hover = hover;
        c.table_active = selected;
        c.table_active_border = accent;
        c.table_row_border = border;
        c.link = accent;
        c.link_hover = accent;
        c.link_active = accent;
        c.success = color(0x72c892, 0x587c0c);
        c.danger = color(0xf28772, 0xad0707);
        c.warning = color(0xe5ba7d, 0x895503);
        c.info = accent;
        c.drag_border = accent;
        c.chart_1 = accent;
        c.chart_2 = c.success;
        c.chart_3 = c.warning;
        c.chart_4 = color(0xb8a4dd, 0x8250a6);
        c.chart_5 = c.danger;
        c.scrollbar = rgba(0).into();
        c.scrollbar_thumb = color(0x565859, 0xb0b0b3);
        c.scrollbar_thumb_hover = color(0x757778, 0x909093);
        c.overlay = rgba(if dark { 0x00000080 } else { 0x00000033 }).into();
        theme.font_size = px(14.);
        theme.notification.placement = gpui_kit::Anchor::BottomRight;
        theme.notification.width = px(320.);
        theme.mono_font_size = px(12.);
        theme.radius = px(4.);
        theme.radius_lg = px(8.);
        theme.focus_ring = false;
        // Inset controls use one border, without a second shadow edge.
        // Floating menus and dialogs retain their own elevation shadows.
        theme.shadow = false;
        // Short, critically damped movement: no bounce or overshoot.
        theme.motion.spring_move =
            gpui_kit::base::Spring::new(std::time::Duration::from_millis(220)).with_epsilon(0.1);
        theme.motion.duration_normal = std::time::Duration::from_millis(180);
    });
}
