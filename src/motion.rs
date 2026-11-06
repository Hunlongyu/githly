//! App-local motion policy; never changes the operating system's preferences.
use crate::config::MotionPreference;
use gpui_kit::{App, Global};

struct SystemMotion(bool);
impl Global for SystemMotion {}

pub fn init(preference: MotionPreference, cx: &mut App) {
    // Kit initialization has already read the platform's accessibility setting.
    cx.set_global(SystemMotion(cx.reduce_motion()));
    apply(preference, cx);
}

pub fn apply(preference: MotionPreference, cx: &mut App) {
    let reduced = match preference {
        MotionPreference::System => cx.global::<SystemMotion>().0,
        MotionPreference::Subtle => false,
        MotionPreference::Off => true,
    };
    cx.set_reduce_motion(reduced);
    cx.refresh_windows();
}
