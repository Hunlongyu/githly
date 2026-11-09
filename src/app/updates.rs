use super::*;
use crate::i18n::messages;
use crate::update;
use gpui_kit::component::{progress::Progress, text::TextView};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

#[derive(Default, PartialEq, Eq)]
enum UpdateActivity {
    #[default]
    Idle,
    Checking,
    Downloading,
    Installing,
}

#[derive(Default)]
pub(super) struct UpdateState {
    task: Option<Task<()>>,
    progress: Option<(u64, u64)>,
    activity: UpdateActivity,
    check_message: Option<LocalizedText>,
    message: Option<LocalizedText>,
    available: Option<update::Available>,
    installer: Option<PathBuf>,
}

impl UpdateState {
    pub(super) fn available_update(&self) -> Option<&update::Available> {
        self.available
            .as_ref()
            .filter(|_| self.activity != UpdateActivity::Checking)
    }

    fn busy(&self) -> bool {
        self.activity != UpdateActivity::Idle
    }

    fn finish_check(&mut self, result: anyhow::Result<Option<update::Available>>) {
        self.activity = UpdateActivity::Idle;
        let available = match result {
            Ok(available) => available,
            Err(error) => {
                tracing::warn!("Update check failed: {error:#}");
                self.check_message =
                    Some(LocalizedText::with_error(error, |l, e| l.check_failed(e)));
                return;
            }
        };
        // Keep the verified download only if the release and its package still match.
        if !matches!((&self.available, &available), (Some(old), Some(new))
            if old.manifest.version == new.manifest.version
                && old.distribution == new.distribution
                && old.manifest.installer.sha256 == new.manifest.installer.sha256)
        {
            self.installer = None;
            self.message = None;
        }
        self.check_message = available
            .is_none()
            .then(|| LocalizedText::new(Locale::up_to_date));
        self.available = available;
    }

    fn finish_install(&mut self, result: anyhow::Result<update::InstallOutcome>, cx: &mut App) {
        self.activity = UpdateActivity::Idle;
        self.message = Some(match result {
            Ok(update::InstallOutcome::Restart(path)) => {
                cx.set_restart_path(path);
                cx.restart();
                LocalizedText::new(Locale::restarting)
            }
            #[cfg(windows)]
            Ok(update::InstallOutcome::InstallerStarted) => {
                LocalizedText::new(Locale::installer_opened)
            }
            Err(error) => LocalizedText::with_error(error, |l, e| l.install_failed(e)),
        });
    }
}

struct UpdateActions(WeakEntity<GitPanel>);

// Published release notes put the English translation after its own heading.
// A release with a single language is shown verbatim in both locales.
fn release_note_sections(notes: &str) -> (SharedString, SharedString) {
    let notes = notes.replace("\r\n", "\n");
    let notes = notes.split("\n## 下载").next().unwrap_or_default().trim();
    if let Some((zh, en)) = notes.split_once("\n**English**\n")
        && !zh.trim().is_empty()
        && !en.trim().is_empty()
    {
        return (zh.trim().to_owned().into(), en.trim().to_owned().into());
    }
    let body: SharedString = notes.to_owned().into();
    (body.clone(), body)
}

impl Render for UpdateActions {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.0
            .update(cx, |panel, cx| panel.update_actions(cx))
            .unwrap_or_else(|_| div().into_any_element())
    }
}

impl GitPanel {
    pub(super) fn check_update(&mut self, cx: &mut Context<Self>) {
        if self.updates.busy() {
            return;
        }
        self.updates.activity = UpdateActivity::Checking;
        self.updates.check_message = None;
        let mut preferences = self.config.updates.clone();
        let task = cx.background_spawn(async move {
            let result = update::check(&mut preferences);
            (result, preferences.api_retry_after)
        });
        self.updates.task = Some(cx.spawn(async move |this, cx| {
            let (result, retry_after) = task.await;
            let _ = this.update(cx, |this, cx| {
                if this.config.updates.api_retry_after != retry_after {
                    this.config.updates.api_retry_after = retry_after;
                    this.save(cx);
                }
                this.updates.finish_check(result);
                cx.notify();
            });
        }));
        cx.notify();
    }

    fn download_update(&mut self, cx: &mut Context<Self>) {
        if self.updates.busy() {
            return;
        }
        let Some(available) = self.updates.available.clone() else {
            return;
        };
        self.updates.activity = UpdateActivity::Downloading;
        self.updates.message = Some(LocalizedText::new(Locale::downloading));
        let total = available.manifest.installer.size;
        let received = Arc::new(AtomicU64::new(0));
        let counter = received.clone();
        let executor = cx.background_executor().clone();
        self.updates.progress = Some((0, total));
        let task = cx.background_spawn(async move { update::download(&available, &counter) });
        let progress_task = cx.spawn(async move |this, cx| {
            loop {
                executor.timer(Duration::from_millis(125)).await;
                let progress = (received.load(Ordering::Relaxed), total);
                if this
                    .update(cx, |this, cx| {
                        if this.updates.progress != Some(progress) {
                            this.updates.progress = Some(progress);
                            cx.notify();
                        }
                    })
                    .is_err()
                {
                    break;
                }
            }
        });
        cx.spawn(async move |this, cx| {
            let result = task.await;
            drop(progress_task);
            if let Err(error) = &result {
                tracing::warn!("Update download failed: {error:#}");
            }
            let _ = this.update(cx, |this, cx| {
                this.updates.progress = None;
                this.updates.activity = UpdateActivity::Idle;
                match result {
                    Ok(path) => {
                        this.updates.installer = Some(path);
                        this.updates.message = Some(LocalizedText::new(Locale::download_verified));
                    }
                    Err(error) => {
                        this.updates.message = Some(LocalizedText::with_error(error, |l, e| {
                            l.download_failed(e)
                        }))
                    }
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    fn install_update(&mut self, cx: &mut Context<Self>) {
        if self.updates.busy() {
            return;
        }
        let (Some(path), Some(available)) = (
            self.updates.installer.clone(),
            self.updates.available.clone(),
        ) else {
            return;
        };
        self.updates.activity = UpdateActivity::Installing;
        self.updates.message = Some(LocalizedText::new(Locale::opening_installer));
        let task = cx.background_spawn(async move { update::install(&path, &available) });
        self.updates.task = Some(cx.spawn(async move |this, cx| {
            let result = task.await;
            let _ = this.update(cx, |this, cx| {
                this.updates.finish_install(result, cx);
                cx.notify();
            });
        }));
        cx.notify();
    }

    pub(super) fn open_update_dialog(&self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(available) = self.updates.available_update() else {
            return;
        };
        let version = available.manifest.version.clone();
        let (zh, en) = release_note_sections(&available.manifest.notes);
        let parent = cx.entity();
        let actions = cx.new(|cx| {
            cx.observe(&parent, |_, _, cx| cx.notify()).detach();
            UpdateActions(parent.downgrade())
        });
        window.open_dialog(cx, move |dialog, _, _| {
            let notes = match messages() {
                Locale::Chinese => &zh,
                Locale::English => &en,
            };
            dialog
                .title(format!("{} · v{version}", messages().release_notes()))
                .w(px(620.))
                .centered()
                .child(
                    div()
                        .id("release-notes-body")
                        .test_support()
                        .max_h(px(360.))
                        .overflow_y_scroll()
                        .child(TextView::markdown(
                            "release-markdown",
                            if notes.is_empty() {
                                messages().notes_unavailable().into()
                            } else {
                                notes.clone()
                            },
                        )),
                )
                .footer(actions.clone())
        });
    }

    pub(super) fn about_update_controls(&self, cx: &mut Context<Self>) -> AnyElement {
        let check = controls::button("check-update")
            .label(if self.updates.activity == UpdateActivity::Checking {
                messages().checking_updates()
            } else {
                messages().check_updates()
            })
            .disabled(self.updates.busy())
            .on_click(cx.listener(|this, _, _, cx| this.check_update(cx)));
        v_flex()
            .gap_1()
            .child(h_flex().gap_2().child(check).when_some(
                self.updates.available_update(),
                |row, available| {
                    row.child(
                        controls::button("about-release-notes")
                            .link()
                            .label(messages().view_update(&available.manifest.version))
                            .on_click(cx.listener(|this, _, window, cx| {
                                window.close_dialog(cx);
                                this.open_update_dialog(window, cx);
                            })),
                    )
                },
            ))
            .when_some(self.updates.check_message.as_ref(), |view, message| {
                view.child(
                    div()
                        .text_xs()
                        .text_color(cx.theme().muted_foreground)
                        .child(message.to_string()),
                )
            })
            .into_any_element()
    }

    fn update_actions(&self, cx: &mut Context<Self>) -> AnyElement {
        v_flex()
            .w_full()
            .gap_2()
            .when_some(self.updates.progress, |view, (bytes, total)| {
                let percent = bytes * 100 / total.max(1);
                view.child(
                    Progress::new("update-progress")
                        .value(percent as f32)
                        .accessibility_label(messages().download_update()),
                )
                .child(div().text_xs().child(format!(
                    "{percent}% · {:.1} / {:.1} MiB",
                    bytes as f64 / 1048576.,
                    total as f64 / 1048576.
                )))
            })
            .when_some(self.updates.message.as_ref(), |view, message| {
                view.child(div().text_size(px(12.)).child(message.to_string()))
            })
            .when(self.updates.available_update().is_some(), |view| {
                view.child(
                    h_flex()
                        .justify_end()
                        .child(if self.updates.installer.is_some() {
                            controls::button("install-update")
                                .primary()
                                .label(messages().install_update())
                                .disabled(self.updates.busy())
                                .on_click(cx.listener(|this, _, _, cx| this.install_update(cx)))
                        } else {
                            controls::button("download-update")
                                .primary()
                                .label(messages().download_update())
                                .disabled(self.updates.busy())
                                .on_click(cx.listener(|this, _, _, cx| this.download_update(cx)))
                        }),
                )
            })
            .into_any_element()
    }
}

#[cfg(all(test, feature = "ui-tests"))]
mod tests {
    use super::GitPanel;
    use crate::config::{Config, ConfigStore};
    use crate::update::{Available, Distribution, Installer, Manifest};
    use gpui_kit::component::WindowExt;
    use gpui_kit::{AppContext, TestAppContext, WindowOptions, test::TestWindowExt};

    #[test]
    fn release_notes_select_the_translation_and_preserve_single_language_notes() {
        let (zh, en) = super::release_note_sections(
            "- 修复搜索\r\n\r\n**English**\r\n\r\n- Fix search\r\n\r\n## 下载\r\n链接",
        );
        assert_eq!(zh.as_ref(), "- 修复搜索");
        assert_eq!(en.as_ref(), "- Fix search");
        let (zh, en) = super::release_note_sections("- Fix search");
        assert_eq!(zh.as_ref(), "- Fix search");
        assert_eq!(en, zh);
    }

    fn release() -> Available {
        Available {
            distribution: Distribution::Native,
            manifest: Manifest {
                version: "1.2.3".into(),
                notes: "- Faster refresh".into(),
                // The download worker rejects this fixture before any network request.
                notes_url: String::new(),
                installer: Installer {
                    url: String::new(),
                    size: 100,
                    sha256: String::new(),
                },
            },
        }
    }

    #[test]
    fn rechecking_keeps_a_verified_download_only_for_the_same_release_asset() {
        let mut updates = super::UpdateState::default();
        let mut release = release();
        updates.finish_check(Ok(Some(release.clone())));
        let downloaded = std::path::PathBuf::from("verified.zip");
        updates.installer = Some(downloaded.clone());
        updates.finish_check(Err(anyhow::anyhow!("offline")));
        assert!(
            updates
                .check_message
                .as_ref()
                .unwrap()
                .to_string()
                .contains("offline")
        );
        assert!(updates.available_update().is_some());
        assert_eq!(updates.installer.as_ref(), Some(&downloaded));

        updates.finish_check(Ok(Some(release.clone())));
        assert!(updates.check_message.is_none());
        assert_eq!(updates.installer.as_ref(), Some(&downloaded));
        release.manifest.installer.sha256 = "another digest".into();
        updates.finish_check(Ok(Some(release)));
        assert!(updates.installer.is_none());
        updates.finish_check(Ok(None));
        assert!(updates.available_update().is_none());
        assert_eq!(
            updates.check_message.unwrap().to_string(),
            super::messages().up_to_date()
        );
    }

    #[gpui_kit::test]
    async fn only_a_completed_install_restarts_the_selected_copy(cx: &mut TestAppContext) {
        let mut updates = super::UpdateState::default();
        let mut restart = cx.expect_restart();
        cx.update(|cx| updates.finish_install(Err(anyhow::anyhow!("cancelled")), cx));
        assert!(restart.try_recv().unwrap().is_none());
        #[cfg(windows)]
        {
            cx.update(|cx| {
                updates.finish_install(Ok(super::update::InstallOutcome::InstallerStarted), cx)
            });
            assert!(restart.try_recv().unwrap().is_none());
        }
        let path = std::path::PathBuf::from("portable copy/Githly");
        cx.update(|cx| {
            updates.finish_install(Ok(super::update::InstallOutcome::Restart(path.clone())), cx)
        });
        assert_eq!(restart.await.unwrap().0, Some(path));
    }

    #[gpui_kit::test]
    fn about_checks_for_updates_and_opens_the_existing_download_dialog(cx: &mut TestAppContext) {
        cx.update(gpui_kit::init);
        let directory = tempfile::tempdir().unwrap();
        let (window, panel) = cx.update(|cx| {
            gpui_kit::open_window(WindowOptions::default(), cx, |window, cx| {
                cx.new(|cx| {
                    GitPanel::new(
                        ConfigStore {
                            path: directory.path().join("config.json"),
                        },
                        // This dialog works without Git. Keep real subprocess I/O off
                        // GPUI's deterministic test scheduler.
                        Config {
                            git_path: Some(directory.path().join("missing-git")),
                            ..Config::default()
                        },
                        None,
                        None,
                        window,
                        cx,
                    )
                })
            })
            .unwrap()
        });
        cx.update_window(window, |_, window, cx| {
            window.render_frame(cx);
            assert!(window.try_find("available-update").is_none());
            window.click("settings", cx);
            assert!(window.try_find("settings-scroll").is_some());
            assert!(window.try_find("update-mode").is_none());
            assert!(window.try_find("check-update").is_none());
            window.click("settings-about", cx);
            window.click("check-update", cx);
            assert_eq!(
                window.find("check-update").label(),
                Some(super::messages().checking_updates())
            );
            panel.update(cx, |panel, cx| {
                // Supply a deterministic result before the test executor runs the network task.
                drop(panel.updates.task.take());
                panel.updates.finish_check(Ok(Some(release())));
                cx.notify();
            });
            window.render_frame(cx);
            window.click("about-release-notes", cx);
            assert!(window.try_find("release-notes-body").is_some());
            assert!(window.try_find("settings-scroll").is_none());
            assert!(window.try_find("update-progress").is_none());
            window.click("download-update", cx);
            assert!(window.try_find("release-notes-body").is_some());
            assert!(window.try_find("update-progress").is_some());
            window.close_dialog(cx);
            window.render_frame(cx);
            window.click("available-update", cx);
            assert!(window.try_find("update-progress").is_some());
        })
        .unwrap();
    }
}
