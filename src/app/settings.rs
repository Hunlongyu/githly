use super::*;
use crate::config::MotionPreference;
use crate::i18n::Language;
use crate::i18n::messages;
use gpui_kit::component::dialog::DialogEntrance;
use gpui_kit::component::{Icon, Selectable, tab::Tab};

#[derive(Clone, Copy, PartialEq, Eq)]
enum SettingsPage {
    General,
    About,
}

struct SettingsView {
    panel: WeakEntity<GitPanel>,
    page: SettingsPage,
}

impl Render for SettingsView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let page = self.page;
        let section = self
            .panel
            .update(cx, |panel, cx| match page {
                SettingsPage::General => panel.general_settings(cx),
                SettingsPage::About => about_page(cx),
            })
            .unwrap_or_else(|_| div().into_any_element());
        let navigation = [
            (
                SettingsPage::General,
                "settings-general",
                messages().general(),
                IconName::Settings2,
            ),
            (
                SettingsPage::About,
                "settings-about",
                messages().about(),
                IconName::Info,
            ),
        ];
        h_flex()
            .h(px(304.))
            .w_full()
            .items_stretch()
            .child(
                v_flex()
                    .w(px(138.))
                    .flex_shrink_0()
                    .pr_3()
                    .gap_1()
                    .border_r_1()
                    .border_color(cx.theme().border)
                    .children(navigation.into_iter().map(|(target, id, label, icon)| {
                        controls::button(id)
                            .ghost()
                            .selected(target == page)
                            .h(px(34.))
                            .w_full()
                            .accessibility_label(label)
                            .child(
                                h_flex()
                                    .w_full()
                                    .gap_2()
                                    .child(Icon::new(icon).size_4())
                                    .child(label),
                            )
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.page = target;
                                cx.notify();
                            }))
                    })),
            )
            .child(
                div()
                    .id("settings-scroll")
                    .test_support()
                    .flex_1()
                    .min_w_0()
                    .pl(px(24.))
                    .pr_1()
                    .overflow_y_scroll()
                    .text_size(px(13.))
                    .line_height(px(18.))
                    .child(section),
            )
    }
}

impl GitPanel {
    fn set_language(&mut self, language: Language, window: &mut Window, cx: &mut Context<Self>) {
        self.config.language = language;
        let locale = crate::i18n::set_language(language);
        self.file_search.update(cx, |input, cx| {
            input.set_placeholder(locale.filter_paths(), window, cx)
        });
        self.history_search.update(cx, |input, cx| {
            input.set_placeholder(locale.search_history(), window, cx)
        });
        self.branch_picker
            .update(cx, |picker, cx| picker.update_language(window, cx));
        self.save(cx);
        cx.notify();
        cx.refresh_windows();
    }

    pub(super) fn open_settings(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let parent = cx.entity();
        let content = cx.new(|cx| {
            cx.observe(&parent, |_, _, cx| cx.notify()).detach();
            SettingsView {
                panel: parent.downgrade(),
                page: SettingsPage::General,
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
                .w(px(670.))
                .centered()
                .entrance(DialogEntrance::Fade)
                .overlay_closable(false)
                .keyboard(true)
                .child(content.clone())
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

    fn general_settings(&self, cx: &mut Context<Self>) -> AnyElement {
        let dark = cx.theme().mode.is_dark();
        v_flex()
            .w_full()
            .text_size(px(13.))
            .line_height(px(18.))
            .gap(px(14.))
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
                            .on_click(cx.listener(|this, index: &usize, window, cx| {
                                this.set_language(
                                    [Language::System, Language::Chinese, Language::English]
                                        [*index],
                                    window,
                                    cx,
                                );
                            })),
                    ),
            )
            .child(
                h_flex()
                    .justify_between()
                    .gap(px(16.))
                    .child(div().font_semibold().child(messages().appearance()))
                    .child(
                        controls::segmented("settings-appearance")
                            .w(px(300.))
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
                h_flex()
                    .justify_between()
                    .gap(px(16.))
                    .child(div().font_semibold().child(messages().motion()))
                    .child(
                        controls::segmented("settings-motion")
                            .w(px(300.))
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
            .child(div().h(px(1.)).bg(cx.theme().border))
            .child(self.git_settings(cx))
            .into_any_element()
    }

    fn git_settings(&self, cx: &mut Context<Self>) -> AnyElement {
        let git_path = self
            .git
            .as_ref()
            .map(|git| git.executable.display().to_string());
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
                                    .map(|git| {
                                        git.version.trim_start_matches("git version ").to_owned()
                                    })
                                    .unwrap_or_else(|| messages().git_missing().into())
                            }),
                    ),
            )
            .when_some(git_path, |view, path| {
                view.child(
                    div()
                        .w_full()
                        .px(px(10.))
                        .py(px(8.))
                        .rounded(px(4.))
                        .border_1()
                        .border_color(cx.theme().input)
                        .bg(cx.theme().background)
                        .text_size(px(12.))
                        .child(path),
                )
            })
            .child(self.git_controls(cx))
            .when(self.config.git_path.is_some(), |view| {
                view.child(
                    controls::button("system-git")
                        .ghost()
                        .small()
                        .label(messages().use_system_path())
                        .on_click(cx.listener(|this, _, _, cx| this.detect_git(None, true, cx))),
                )
            })
            .when_some(self.git_error.clone(), |view, error| {
                view.child(
                    div()
                        .text_size(px(12.))
                        .text_color(cx.theme().danger)
                        .child(error.to_string()),
                )
            })
            .into_any_element()
    }
}

fn about_page(cx: &App) -> AnyElement {
    let links = [
        (
            "about-repository",
            messages().repository_link(),
            "Hunlongyu/githly",
            env!("CARGO_PKG_REPOSITORY"),
        ),
        (
            "about-feedback",
            messages().feedback(),
            "GitHub Issues",
            concat!(env!("CARGO_PKG_REPOSITORY"), "/issues"),
        ),
    ];
    v_flex()
        .gap(px(22.))
        .child(
            h_flex()
                .gap(px(14.))
                .child(crate::app_assets::logo(px(56.)))
                .child(
                    h_flex()
                        .gap_2()
                        .child(
                            div()
                                .text_size(px(22.))
                                .line_height(px(28.))
                                .font_semibold()
                                .child("Githly"),
                        )
                        .child(
                            div()
                                .id("about-version")
                                .test_support()
                                .text_size(px(12.))
                                .text_color(cx.theme().muted_foreground)
                                .child(concat!("v", env!("CARGO_PKG_VERSION"))),
                        ),
                ),
        )
        .child(
            v_flex()
                .border_1()
                .border_color(cx.theme().border)
                .rounded_md()
                .child(
                    h_flex()
                        .px_3()
                        .h(px(44.))
                        .justify_between()
                        .child(
                            div()
                                .text_color(cx.theme().muted_foreground)
                                .child(messages().developer()),
                        )
                        .child(env!("CARGO_PKG_AUTHORS")),
                )
                .children(links.into_iter().map(|(id, heading, label, url)| {
                    h_flex()
                        .px_3()
                        .h(px(44.))
                        .justify_between()
                        .border_t_1()
                        .border_color(cx.theme().border)
                        .child(div().text_color(cx.theme().muted_foreground).child(heading))
                        .child(
                            controls::button(id)
                                .link()
                                .icon(IconName::ExternalLink)
                                .label(label)
                                .tooltip(url)
                                .on_click(move |_, _, cx| cx.open_url(url)),
                        )
                })),
        )
        .into_any_element()
}

#[cfg(all(test, feature = "ui-tests"))]
mod tests {
    use super::{Config, ConfigStore, GitPanel, Language};
    use gpui_kit::test::TestWindowExt;
    use gpui_kit::{AppContext, TestAppContext, WindowOptions};

    #[gpui_kit::test]
    fn settings_pages_save_preferences_and_open_the_project_repository(cx: &mut TestAppContext) {
        // Both our locale and the component locale are process-wide. Other
        // tests can be rendering or reporting errors while this one switches.
        const CHILD: &str = "GITHLY_TEST_LANGUAGE_SWITCH";
        if std::env::var_os(CHILD).is_none() {
            let result = std::process::Command::new(std::env::current_exe().unwrap())
                .args(["--exact", "app::settings::tests::settings_pages_save_preferences_and_open_the_project_repository"])
                .env(CHILD, "1").output().unwrap();
            assert!(
                result.status.success(),
                "{}{}",
                String::from_utf8_lossy(&result.stdout),
                String::from_utf8_lossy(&result.stderr)
            );
            return;
        }
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
            let search = panel.read(cx).history_search.clone();
            search.update(cx, |input, cx| input.set_value("feature/分支", window, cx));
            let diagnostic = super::LocalizedText::diagnostic(
                anyhow::anyhow!("remote: feature/分支")
                    .context(super::LocalizedText::new(super::Locale::start_git)),
            );
            panel.update(cx, |panel, cx| panel.open_settings(window, cx));
            window.render_frame(cx);
            assert!(window.try_find("about-version").is_none());
            window.within("settings-language").click(1_usize, cx);
            assert_eq!(window.find("settings-about").label(), Some("关于"));
            assert_eq!(window.find("settings").label(), Some("设置"));
            assert_eq!(
                diagnostic.to_string(),
                "无法启动 Git 程序: remote: feature/分支"
            );
            assert_eq!(&*gpui_kit::component::locale(), "zh-CN");
            assert_eq!(
                search.read(cx).presentation().placeholder().as_ref(),
                "搜索说明、作者、哈希"
            );
            window.within("settings-language").click(0_usize, cx);
            assert_eq!(crate::i18n::messages(), Language::System.resolve());
            window.within("settings-language").click(2_usize, cx);
            assert_eq!(window.find("settings-about").label(), Some("About"));
            assert_eq!(window.find("settings").label(), Some("Settings"));
            assert_eq!(
                diagnostic.to_string(),
                "Could not start Git: remote: feature/分支"
            );
            assert_eq!(&*gpui_kit::component::locale(), "en");
            assert_eq!(
                search.read(cx).presentation().placeholder().as_ref(),
                "Search messages, authors, or hashes"
            );
            assert_eq!(search.read(cx).value().as_ref(), "feature/分支");
            assert_eq!(panel.read(cx).config.language, Language::English);
            assert!(window.try_find("settings-git").is_none());
            assert!(
                window
                    .within("settings-scroll")
                    .find("choose-git")
                    .visible()
            );
            window.click("settings-about", cx);
            assert!(
                window
                    .within("settings-scroll")
                    .find("about-version")
                    .visible()
            );
            assert!(window.find("about-repository").visible());
            window.click("about-repository", cx);
        })
        .unwrap();
        assert_eq!(
            cx.opened_url().as_deref(),
            Some("https://github.com/Hunlongyu/githly")
        );
        cx.update_window(window, |_, window, cx| {
            assert!(window.find("about-feedback").visible());
            window.click("about-feedback", cx);
            window.click("settings-general", cx);
            assert!(window.try_find("about-version").is_none());
            assert!(window.within("settings-motion").find(0_usize).visible());
            assert!(window.try_find("about-repository").is_none());
        })
        .unwrap();
        cx.run_until_parked();
        assert_eq!(
            cx.opened_url().as_deref(),
            Some("https://github.com/Hunlongyu/githly/issues")
        );
        assert_eq!(store.load().unwrap().language, Language::English);
    }
}
