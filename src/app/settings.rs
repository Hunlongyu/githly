use super::*;
use crate::config::MotionPreference;
use gpui_kit::component::dialog::DialogEntrance;
use gpui_kit::component::{Icon, tab::Tab};

struct SettingsView {
    panel: WeakEntity<GitPanel>,
}

impl Render for SettingsView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.panel
            .update(cx, |panel, cx| panel.settings_content(cx))
            .unwrap_or_else(|_| div().into_any_element())
    }
}

impl GitPanel {
    pub(super) fn open_settings(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.updates.mirror_input = Some(cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("Release 镜像 HTTPS 前缀（可选）")
                .default_value(self.config.updates.mirror.clone())
        }));
        let parent = cx.entity();
        let content = cx.new(|cx| {
            cx.observe(&parent, |_, _, cx| cx.notify()).detach();
            SettingsView {
                panel: parent.downgrade(),
            }
        });
        window.open_dialog(cx, move |dialog, _, _| {
            dialog
                .title(
                    h_flex()
                        .gap(px(8.))
                        .child(Icon::new(IconName::Settings).size_4())
                        .child("设置"),
                )
                .w(px(600.))
                .centered()
                .entrance(DialogEntrance::Fade)
                .overlay_closable(false)
                .keyboard(true)
                .child(
                    div()
                        .id("settings-scroll")
                        .max_h(px(420.))
                        .overflow_y_scroll()
                        .child(content.clone()),
                )
                .footer(
                    h_flex().w_full().justify_end().child(
                        controls::button("settings-done")
                            .primary()
                            .min_w(px(64.))
                            .label("完成")
                            .on_click(|_, window, cx| window.close_dialog(cx)),
                    ),
                )
        });
    }

    fn settings_content(&self, cx: &mut Context<Self>) -> AnyElement {
        let dark = cx.theme().mode.is_dark();
        let git_path = self
            .git
            .as_ref()
            .map(|git| git.executable.display().to_string());
        v_flex()
            .w_full()
            .text_size(px(13.))
            .line_height(px(18.))
            .gap(px(20.))
            .child(
                h_flex()
                    .justify_between()
                    .gap(px(16.))
                    .child(div().font_semibold().child("外观"))
                    .child(
                        controls::segmented("settings-appearance")
                            .w(px(260.))
                            .selected_index(usize::from(dark))
                            .child(Tab::new().flex_1().label("浅色"))
                            .child(Tab::new().flex_1().label("深色"))
                            .on_click(cx.listener(|this, index: &usize, window, cx| {
                                if cx.theme().mode.is_dark() != (*index == 1) {
                                    this.theme(window, cx);
                                }
                            })),
                    ),
            )
            .child(
                v_flex()
                    .gap(px(8.))
                    .child(
                        h_flex()
                            .justify_between()
                            .gap(px(16.))
                            .child(div().font_semibold().child("界面动效"))
                            .child(
                                controls::segmented("settings-motion")
                                    .w(px(260.))
                                    .selected_index(match self.config.motion {
                                        MotionPreference::System => 0,
                                        MotionPreference::Subtle => 1,
                                        MotionPreference::Off => 2,
                                    })
                                    .child(Tab::new().flex_1().label("跟随系统"))
                                    .child(Tab::new().flex_1().label("轻微"))
                                    .child(Tab::new().flex_1().label("关闭"))
                                    .on_click(cx.listener(|this, index: &usize, _, cx| {
                                        let preference = [
                                            MotionPreference::System,
                                            MotionPreference::Subtle,
                                            MotionPreference::Off,
                                        ][*index];
                                        if this.config.motion != preference {
                                            this.config.motion = preference;
                                            crate::motion::apply(preference, cx);
                                            this.save(cx);
                                            cx.notify();
                                        }
                                    })),
                            ),
                    )
                    .child(
                        div()
                            .text_size(px(12.))
                            .text_color(cx.theme().muted_foreground)
                            .child("轻微动效用于按钮切换、关系图和弹窗。仅影响 Githly。"),
                    ),
            )
            .child(div().h(px(1.)).w_full().bg(cx.theme().border))
            .child(
                v_flex()
                    .gap(px(12.))
                    .child(
                        h_flex()
                            .gap(px(8.))
                            .child(div().font_semibold().child("Git"))
                            .child(div().flex_1())
                            .child(
                                div()
                                    .text_size(px(12.))
                                    .text_color(cx.theme().muted_foreground)
                                    .child(if self.detecting {
                                        "正在检测…".to_string()
                                    } else {
                                        self.git
                                            .as_ref()
                                            .map(|git| git.version.clone())
                                            .unwrap_or_else(|| "未找到 Git".into())
                                    }),
                            ),
                    )
                    .child(
                        div()
                            .w_full()
                            .px(px(10.))
                            .py(px(8.))
                            .rounded(px(4.))
                            .border_1()
                            .border_color(cx.theme().input)
                            .bg(cx.theme().background)
                            .text_size(px(12.))
                            .child(git_path.unwrap_or_else(|| "请选择已安装的 Git 程序".into())),
                    )
                    .child(self.git_controls(cx))
                    .when(self.config.git_path.is_some(), |view| {
                        view.child(
                            controls::button("system-git")
                                .ghost()
                                .small()
                                .label("改用系统 PATH")
                                .on_click(
                                    cx.listener(|this, _, _, cx| this.detect_git(None, true, cx)),
                                ),
                        )
                    })
                    .when_some(self.error.clone(), |view, error| {
                        view.child(
                            div()
                                .text_size(px(12.))
                                .text_color(cx.theme().danger)
                                .child(error),
                        )
                    }),
            )
            .child(self.update_settings(cx))
            .child(
                v_flex()
                    .gap(px(4.))
                    .text_size(px(11.))
                    .text_color(cx.theme().muted_foreground)
                    .child("设置自动保存")
                    .child(self.store.path.display().to_string()),
            )
            .into_any_element()
    }
}
