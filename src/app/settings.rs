use super::*;
use crate::config::MotionPreference;
use crate::i18n::Language;
use crate::i18n::messages;
use gpui_kit::component::dialog::DialogEntrance;
use gpui_kit::component::{
    Icon,
    tab::{Tab, TabBar},
};

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
                SettingsPage::About => about_page(panel, cx),
            })
            .unwrap_or_else(|_| div().into_any_element());
        let page_tab = |icon, label: &'static str| {
            Tab::new().aria_label(label).child(
                h_flex()
                    .px_2()
                    .gap_2()
                    .child(Icon::new(icon).size(px(15.)))
                    .child(label),
            )
        };
        v_flex()
            .w_full()
            .min_h_0()
            .gap_3()
            .child(
                TabBar::new("settings-pages")
                    .underline()
                    .with_size(px(34.))
                    .selected_index(usize::from(page == SettingsPage::About))
                    .child(page_tab(IconName::Settings2, messages().general()))
                    .child(page_tab(IconName::Info, messages().about()))
                    .on_click(cx.listener(|this, index: &usize, _, cx| {
                        this.page = [SettingsPage::General, SettingsPage::About][*index];
                        cx.notify();
                    })),
            )
            .child(
                div()
                    .id("settings-scroll")
                    .test_support()
                    .w_full()
                    .h(px(280.))
                    .min_h_0()
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
        window.open_dialog(cx, move |dialog, _, cx| {
            dialog
                .title(
                    h_flex()
                        .gap_2()
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .justify_center()
                                .size(px(30.))
                                .rounded_md()
                                .bg(cx.theme().success.opacity(0.12))
                                .child(
                                    Icon::new(IconName::Settings)
                                        .size(px(17.))
                                        .text_color(cx.theme().success),
                                ),
                        )
                        .child(
                            div()
                                .text_size(px(20.))
                                .font_semibold()
                                .child(messages().settings()),
                        ),
                )
                .w(px(596.))
                .px(px(20.))
                .centered()
                .entrance(DialogEntrance::Fade)
                .overlay_closable(false)
                .keyboard(true)
                .child(content.clone())
                .footer(
                    h_flex()
                        .w_full()
                        .border_t_1()
                        .border_color(cx.theme().border)
                        .pt_3()
                        .justify_end()
                        .child(
                            controls::button("settings-done")
                                .primary()
                                .min_w(px(76.))
                                .label(messages().done())
                                .on_click(|_, window, cx| window.close_dialog(cx)),
                        ),
                )
        });
    }

    fn general_settings(&self, cx: &mut Context<Self>) -> AnyElement {
        let language = controls::segmented("settings-language")
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
                    [Language::System, Language::Chinese, Language::English][*index],
                    window,
                    cx,
                );
            }));
        let theme_tab = |dark, label: &'static str| {
            Tab::new()
                .flex_1()
                .aria_label(label)
                .child(h_flex().gap_2().child(theme_sample(dark)).child(label))
        };
        let appearance = controls::segmented("settings-appearance")
            .with_size(px(40.))
            .selected_index(match self.config.appearance {
                Appearance::System => 0,
                Appearance::Light => 1,
                Appearance::Dark => 2,
            })
            .child(Tab::new().flex_1().label(messages().system()))
            .child(theme_tab(false, messages().light()))
            .child(theme_tab(true, messages().dark()))
            .on_click(cx.listener(|this, index: &usize, window, cx| {
                this.set_appearance(
                    [Appearance::System, Appearance::Light, Appearance::Dark][*index],
                    window,
                    cx,
                );
            }));
        let motion = controls::segmented("settings-motion")
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
            }));
        let row = |icon, label: &'static str, control: TabBar| {
            h_flex()
                .min_h(px(46.))
                .justify_between()
                .gap_3()
                .child(
                    h_flex()
                        .gap_2()
                        .child(
                            Icon::new(icon)
                                .size(px(16.))
                                .text_color(cx.theme().muted_foreground),
                        )
                        .child(div().font_medium().child(label)),
                )
                .child(control.w(relative(0.66)))
        };
        v_flex()
            .w_full()
            .gap(px(12.))
            .child(
                v_flex()
                    .px_4()
                    .py_1()
                    .rounded_lg()
                    .border_1()
                    .border_color(cx.theme().border)
                    .bg(cx.theme().sidebar)
                    .child(row(IconName::Palette, messages().appearance(), appearance))
                    .child(
                        row(IconName::Globe, messages().language(), language)
                            .border_t_1()
                            .border_color(cx.theme().border),
                    )
                    .child(
                        row(IconName::Play, messages().motion(), motion)
                            .border_t_1()
                            .border_color(cx.theme().border),
                    ),
            )
            .child(self.git_settings(cx))
            .into_any_element()
    }

    fn git_settings(&self, cx: &mut Context<Self>) -> AnyElement {
        let git_path = self
            .git
            .as_ref()
            .map(|git| git.executable.display().to_string());
        let (status, color) = if self.detecting {
            (
                messages().detecting().to_owned(),
                cx.theme().muted_foreground,
            )
        } else if let Some(git) = &self.git {
            (
                git.version.trim_start_matches("git version ").to_owned(),
                cx.theme().success,
            )
        } else {
            (messages().git_missing().to_owned(), cx.theme().warning)
        };
        v_flex()
            .px_4()
            .py_3()
            .gap_2()
            .rounded_lg()
            .border_1()
            .border_color(cx.theme().border)
            .bg(cx.theme().sidebar)
            .child(
                h_flex()
                    .justify_between()
                    .child(
                        h_flex()
                            .gap_2()
                            .child(Icon::new(IconName::SquareTerminal).size(px(16.)))
                            .child(div().font_semibold().child("Git")),
                    )
                    .child(
                        h_flex()
                            .gap(px(6.))
                            .child(div().size(px(6.)).rounded_full().bg(color))
                            .child(div().text_size(px(12.)).child(status)),
                    ),
            )
            .when_some(git_path, |view, path| {
                view.child(
                    div()
                        .id("git-executable-path")
                        .w_full()
                        .text_ellipsis()
                        .whitespace_nowrap()
                        .overflow_hidden()
                        .text_size(px(12.))
                        .text_color(cx.theme().muted_foreground)
                        .child(path.clone())
                        .tooltip(move |window, cx| {
                            gpui_kit::component::tooltip::Tooltip::new(path.clone())
                                .build(window, cx)
                        }),
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

// Small window previews keep their respective palettes in either application theme.
fn theme_sample(dark: bool) -> impl IntoElement {
    let (surface, sidebar, line) = if dark {
        (0x191a1b, 0x383a3c, 0x929599)
    } else {
        (0xffffff, 0xe1e3e6, 0x747a81)
    };
    h_flex()
        .w(px(31.))
        .h(px(20.))
        .rounded(px(3.))
        .overflow_hidden()
        .border_1()
        .border_color(rgb(line))
        .bg(rgb(surface))
        .child(div().w(px(7.)).h_full().bg(rgb(sidebar)))
        .child(
            v_flex()
                .pl(px(3.))
                .gap(px(3.))
                .child(div().w(px(12.)).h(px(2.)).bg(rgb(line)))
                .child(div().w(px(7.)).h(px(2.)).bg(rgb(line))),
        )
}

fn about_page(panel: &GitPanel, cx: &mut Context<GitPanel>) -> AnyElement {
    let links = [
        (
            "about-repository",
            gpui_kit::assets::IconName::Github,
            messages().repository_link(),
            "Hunlongyu/githly",
            env!("CARGO_PKG_REPOSITORY"),
        ),
        (
            "about-feedback",
            gpui_kit::assets::IconName::MessageSquare,
            messages().feedback(),
            "GitHub Issues",
            concat!(env!("CARGO_PKG_REPOSITORY"), "/issues"),
        ),
    ];
    v_flex()
        .gap(px(12.))
        .child(
            v_flex()
                .rounded_lg()
                .overflow_hidden()
                .border_1()
                .border_color(cx.theme().border)
                .bg(cx.theme().sidebar)
                .child(
                    h_flex()
                        .h(px(110.))
                        .rounded_tl_lg()
                        .rounded_tr_lg()
                        .px_5()
                        .justify_between()
                        .bg(linear_gradient(
                            105.,
                            linear_color_stop(cx.theme().success.opacity(0.04), 0.),
                            linear_color_stop(cx.theme().success.opacity(0.20), 1.),
                        ))
                        .child(
                            v_flex()
                                .gap(px(6.))
                                .child(
                                    div()
                                        .text_size(px(30.))
                                        .line_height(px(36.))
                                        .font_semibold()
                                        .child("Githly"),
                                )
                                .child(
                                    h_flex()
                                        .gap_2()
                                        .child(
                                            div()
                                                .text_color(cx.theme().muted_foreground)
                                                .child(messages().developer()),
                                        )
                                        .child(env!("CARGO_PKG_AUTHORS")),
                                ),
                        )
                        .child(crate::app_assets::logo(px(84.))),
                )
                .child(panel.about_update_controls(cx)),
        )
        .child(
            h_flex().gap(px(10.)).children(links.into_iter().map(
                |(id, icon, heading, destination, url)| {
                    controls::button(id)
                        .flex_1()
                        .min_w_0()
                        .h(px(66.))
                        .rounded_lg()
                        .px_3()
                        .accessibility_label(heading)
                        .tooltip(url)
                        .child(
                            h_flex()
                                .w_full()
                                .gap_2()
                                .child(Icon::new(icon).size(px(18.)))
                                .child(
                                    v_flex()
                                        .flex_1()
                                        .min_w_0()
                                        .gap(px(2.))
                                        .child(
                                            div().text_size(px(13.)).font_semibold().child(heading),
                                        )
                                        .child(
                                            div()
                                                .text_size(px(12.))
                                                .text_color(cx.theme().muted_foreground)
                                                .child(destination),
                                        ),
                                )
                                .child(
                                    Icon::new(IconName::ExternalLink)
                                        .size(px(12.))
                                        .text_color(cx.theme().muted_foreground),
                                ),
                        )
                        .on_click(move |_, _, cx| cx.open_url(url))
                },
            )),
        )
        .into_any_element()
}

#[cfg(all(test, feature = "ui-tests"))]
mod tests {
    use super::{Appearance, Config, ConfigStore, GitPanel, Language};
    use gpui_kit::component::{ActiveTheme, ThemeMode};
    use gpui_kit::test::TestWindowExt;
    use gpui_kit::{AppContext, TestAppContext, WindowAppearance, WindowOptions};

    #[gpui_kit::test]
    fn appearance_follows_live_changes_only_when_system_is_selected(cx: &mut TestAppContext) {
        cx.update(gpui_kit::init);
        let directory = tempfile::tempdir().unwrap();
        let store = ConfigStore {
            path: directory.path().join("config.json"),
        };
        let (handle, panel) = cx.update(|cx| {
            gpui_kit::open_window(WindowOptions::default(), cx, |window, cx| {
                cx.new(|cx| {
                    GitPanel::new_idle(store.clone(), Config::default(), None, None, window, cx)
                })
            })
            .unwrap()
        });
        cx.update_window(handle, |_, window, cx| {
            panel.update(cx, |p, cx| p.open_settings(window, cx));
            window.render_frame(cx);
            assert_eq!(
                window
                    .within("settings-appearance")
                    .find(0_usize)
                    .selected(),
                Some(true)
            );
            for (index, preference, system, expected) in [
                (
                    2_usize,
                    Appearance::Dark,
                    WindowAppearance::Light,
                    ThemeMode::Dark,
                ),
                (
                    1_usize,
                    Appearance::Light,
                    WindowAppearance::Dark,
                    ThemeMode::Light,
                ),
            ] {
                window.within("settings-appearance").click(index, cx);
                panel.update(cx, |p, cx| p.sync_appearance(system, cx));
                assert_eq!(panel.read(cx).config.appearance, preference);
                assert_eq!(cx.theme().mode, expected);
            }
        })
        .unwrap();
        cx.run_until_parked();
        assert_eq!(store.load().unwrap().appearance, Appearance::Light);
        cx.update_window(handle, |_, window, cx| {
            window.within("settings-appearance").click(0_usize, cx);
            assert_eq!(
                cx.theme().mode,
                Appearance::System.resolve(window.appearance())
            );
            for (system, expected) in [
                (WindowAppearance::Dark, ThemeMode::Dark),
                (WindowAppearance::Light, ThemeMode::Light),
            ] {
                panel.update(cx, |p, cx| p.sync_appearance(system, cx));
                assert_eq!(cx.theme().mode, expected);
            }
            window.render_frame(cx);
            assert_eq!(
                window
                    .within("settings-appearance")
                    .find(0_usize)
                    .selected(),
                Some(true)
            );
        })
        .unwrap();
        cx.run_until_parked();
        assert_eq!(store.load().unwrap().appearance, Appearance::System);
    }

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
            assert_eq!(
                window.within("settings-pages").find(1_usize).label(),
                Some("关于")
            );
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
            assert_eq!(
                window.within("settings-pages").find(1_usize).label(),
                Some("About")
            );
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
            window.within("settings-pages").click(1_usize, cx);
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
            window.within("settings-pages").click(0_usize, cx);
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
