use super::*;
use crate::i18n::messages;
use crate::update;
use gpui_kit::component::{progress::Progress, text::TextView};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

#[derive(Default)]
pub(super) struct UpdateState {
    task: Option<Task<()>>,
    progress: Option<(u64, u64)>,
    busy: bool,
    message: String,
    available: Option<update::Available>,
    installer: Option<PathBuf>,
}

impl UpdateState {
    pub(super) fn update_available(&self) -> bool {
        self.available.is_some()
    }

    fn finish_install(&mut self, result: anyhow::Result<update::InstallOutcome>, cx: &mut App) {
        self.busy = false;
        self.message = match result {
            Ok(update::InstallOutcome::Restart(path)) => {
                cx.set_restart_path(path);
                cx.restart();
                messages().restarting().into()
            }
            #[cfg(windows)]
            Ok(update::InstallOutcome::InstallerStarted) => messages().installer_opened().into(),
            Err(error) => messages().install_failed(error),
        };
    }
}

struct UpdateActions(WeakEntity<GitPanel>);

impl Render for UpdateActions {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.0
            .update(cx, |panel, cx| panel.update_actions(cx))
            .unwrap_or_else(|_| div().into_any_element())
    }
}

impl GitPanel {
    pub(super) fn check_update(&mut self, cx: &mut Context<Self>) {
        if self.updates.busy {
            return;
        }
        self.updates.busy = true;
        self.updates.available = None;
        self.updates.installer = None;
        let mut preferences = self.config.updates.clone();
        let task = cx.background_spawn(async move {
            let result = update::check(&mut preferences);
            (result, preferences.api_retry_after)
        });
        self.updates.task = Some(cx.spawn(async move |this, cx| {
            let (result, retry_after) = task.await;
            let _ = this.update(cx, |this, cx| {
                this.updates.busy = false;
                if this.config.updates.api_retry_after != retry_after {
                    this.config.updates.api_retry_after = retry_after;
                    this.save(cx);
                }
                match result {
                    Ok(available) => this.updates.available = available,
                    Err(error) => tracing::warn!("Update check failed: {error:#}"),
                }
                cx.notify();
            });
        }));
        cx.notify();
    }

    fn download_update(&mut self, cx: &mut Context<Self>) {
        if self.updates.busy {
            return;
        }
        let Some(available) = self.updates.available.clone() else {
            return;
        };
        self.updates.busy = true;
        self.updates.message = messages().downloading().into();
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
                this.updates.busy = false;
                match result {
                    Ok(path) => {
                        this.updates.installer = Some(path);
                        this.updates.message = messages().download_verified().into();
                    }
                    Err(error) => this.updates.message = messages().download_failed(error),
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    fn install_update(&mut self, cx: &mut Context<Self>) {
        if self.updates.busy {
            return;
        }
        let (Some(path), Some(available)) = (
            self.updates.installer.clone(),
            self.updates.available.clone(),
        ) else {
            return;
        };
        self.updates.busy = true;
        self.updates.message = messages().opening_installer().into();
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
        let Some(available) = &self.updates.available else {
            return;
        };
        let title = format!(
            "{} · v{}",
            messages().release_notes(),
            available.manifest.version
        );
        let notes = available
            .manifest
            .notes
            .split("\n## 下载")
            .next()
            .unwrap_or_default()
            .trim();
        let body: SharedString = if notes.is_empty() {
            messages().notes_unavailable().into()
        } else {
            notes.to_owned().into()
        };
        let parent = cx.entity();
        let actions = cx.new(|cx| {
            cx.observe(&parent, |_, _, cx| cx.notify()).detach();
            UpdateActions(parent.downgrade())
        });
        window.open_dialog(cx, move |dialog, _, _| {
            dialog
                .title(title.clone())
                .w(px(620.))
                .centered()
                .child(
                    div()
                        .id("release-notes-body")
                        .test_support()
                        .max_h(px(360.))
                        .overflow_y_scroll()
                        .child(TextView::markdown("release-markdown", body.clone())),
                )
                .footer(actions.clone())
        });
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
            .when(!self.updates.message.is_empty(), |view| {
                view.child(div().text_size(px(12.)).child(self.updates.message.clone()))
            })
            .when(self.updates.update_available(), |view| {
                view.child(
                    h_flex()
                        .justify_end()
                        .child(if self.updates.installer.is_some() {
                            controls::button("install-update")
                                .primary()
                                .label(messages().install_update())
                                .disabled(self.updates.busy)
                                .on_click(cx.listener(|this, _, _, cx| this.install_update(cx)))
                        } else {
                            controls::button("download-update")
                                .primary()
                                .label(messages().download_update())
                                .disabled(self.updates.busy)
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
    fn update_icon_opens_notes_and_download_progress_in_the_same_dialog(cx: &mut TestAppContext) {
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
            window.close_dialog(cx);
            panel.update(cx, |panel, cx| {
                panel.updates.available = Some(Available {
                    distribution: Distribution::Native,
                    manifest: Manifest {
                        version: "1.2.3".into(),
                        notes: "- Faster refresh".into(),
                        // The worker rejects this fixture before any network request.
                        notes_url: String::new(),
                        installer: Installer {
                            url: String::new(),
                            size: 100,
                            sha256: String::new(),
                        },
                    },
                });
                cx.notify();
            });
            window.render_frame(cx);
            window.click("available-update", cx);
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
