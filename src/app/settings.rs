use super::*;
use crate::config::MotionPreference;
use crate::i18n::Language;
use crate::i18n::messages;
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
                .placeholder(messages().mirror_placeholder())
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
                        .child(messages().settings()),
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
                            .label(messages().done())
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
                    .child(div().font_semibold().child(messages().language()))
                    .child(
                        controls::segmented("settings-language")
                            .w(px(300.))
                            .selected_index(match self.config.language {
                                Language::System => 0,
                                Language::Chinese => 1,
                                Language::English => 2,
                            })
                            .child(Tab::new().flex_1().label(messages().system()))
                            .child(Tab::new().flex_1().label("简体中文"))
                            .child(Tab::new().flex_1().label("English"))
                            .on_click(cx.listener(|this, index: &usize, _, cx| {
                                this.config.language =
                                    [Language::System, Language::Chinese, Language::English]
                                        [*index];
                                this.save(cx);
                                cx.notify();
                            })),
                    ),
            )
            .when(self.config.language.resolve() != messages(), |view| {
                view.child(
                    div()
                        .text_sm()
                        .text_color(cx.theme().muted_foreground)
                        .child(messages().language_restart()),
                )
            })
            .child(
                h_flex()
                    .justify_between()
                    .gap(px(16.))
                    .child(div().font_semibold().child(messages().appearance()))
                    .child(
                        controls::segmented("settings-appearance")
                            .w(px(260.))
                            .selected_index(usize::from(dark))
                            .child(Tab::new().flex_1().label(messages().light()))
                            .child(Tab::new().flex_1().label(messages().dark()))
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
                            .child(div().font_semibold().child(messages().motion()))
                            .child(
                                controls::segmented("settings-motion")
                                    .w(px(260.))
                                    .selected_index(match self.config.motion {
                                        MotionPreference::System => 0,
                                        MotionPreference::Subtle => 1,
                                        MotionPreference::Off => 2,
                                    })
                                    .child(Tab::new().flex_1().label(messages().system()))
                                    .child(Tab::new().flex_1().label(messages().subtle()))
                                    .child(Tab::new().flex_1().label(messages().off()))
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
                            .child(messages().motion_help()),
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
                                        messages().detecting().to_string()
                                    } else {
                                        self.git
                                            .as_ref()
                                            .map(|git| git.version.clone())
                                            .unwrap_or_else(|| messages().git_missing().into())
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
                            .child(
                                git_path
                                    .unwrap_or_else(|| messages().select_installed_git().into()),
                            ),
                    )
                    .child(self.git_controls(cx))
                    .when(self.config.git_path.is_some(), |view| {
                        view.child(
                            controls::button("system-git")
                                .ghost()
                                .small()
                                .label(messages().use_system_path())
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
                    .child(messages().settings_autosave())
                    .child(self.store.path.display().to_string()),
            )
            .into_any_element()
    }
}

#[cfg(all(test, feature = "ui-tests"))]
mod tests {
    use super::{Config, ConfigStore, GitPanel, Language};
    use gpui_kit::test::TestWindowExt;
    use gpui_kit::{AppContext, TestAppContext, WindowOptions};

    #[gpui_kit::test]
    fn language_selector_saves_the_preference(cx: &mut TestAppContext) {
        cx.executor().allow_parking();
        cx.update(gpui_kit::init);
        let directory = tempfile::tempdir().unwrap();
        let store = ConfigStore {
            path: directory.path().join("config.json"),
        };
        let (window, panel) = cx.update(|cx| {
            gpui_kit::open_window(WindowOptions::default(), cx, |window, cx| {
                cx.new(|cx| GitPanel::new(store.clone(), Config::default(), None, None, window, cx))
            })
            .unwrap()
        });
        cx.update_window(window, |_, window, cx| {
            panel.update(cx, |panel, cx| panel.open_settings(window, cx));
            window.render_frame(cx);
            window.within("settings-language").click(2_usize, cx);
            assert_eq!(panel.read(cx).config.language, Language::English);
        })
        .unwrap();
        cx.run_until_parked();
        assert_eq!(store.load().unwrap().language, Language::English);
    }
}
