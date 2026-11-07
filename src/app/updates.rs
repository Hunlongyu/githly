use super::*;
use crate::i18n::messages;
use crate::update;
use gpui_kit::component::{input::Input, tab::Tab};
use std::time::Duration;

#[derive(Default)]
pub(super) struct UpdateState {
    task: Option<Task<()>>,
    timer: Option<Task<()>>,
    busy: bool,
    message: String,
    pub available: Option<update::Available>,
    installer: Option<PathBuf>,
    pub mirror_input: Option<Entity<InputState>>,
}

impl GitPanel {
    pub(super) fn start_update_timer(&mut self, cx: &mut Context<Self>) {
        self.updates.timer = Some(cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(Duration::from_secs(30))
                .await;
            loop {
                if this
                    .update(cx, |this, cx| {
                        let preferences = &this.config.updates;
                        if preferences.enabled
                            && update::now().saturating_sub(preferences.last_check)
                                >= preferences.interval_hours.clamp(6, 168) * 3600
                        {
                            this.check_update(cx);
                        }
                    })
                    .is_err()
                {
                    break;
                }
                // Settings changes take effect on the next hourly wake; no UI ticks.
                cx.background_executor()
                    .timer(Duration::from_secs(3600))
                    .await;
            }
        }));
    }

    pub(super) fn check_update(&mut self, cx: &mut Context<Self>) {
        if self.updates.busy {
            return;
        }
        if update::now().saturating_sub(self.config.updates.last_check) < 60 {
            self.updates.message = messages().check_cooldown().into();
            cx.notify();
            return;
        }
        self.config.updates.last_check = update::now();
        self.save(cx);
        self.updates.busy = true;
        self.updates.message = messages().checking_updates().into();
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
                this.config.updates.api_retry_after = retry_after;
                this.save(cx);
                match result {
                    Ok(check) => {
                        this.updates.message = check.available.as_ref().map_or_else(
                            || {
                                format!(
                                    "{} · {}",
                                    if matches!(check.source, update::Source::Mirror(_)) {
                                        messages().mirror_no_update()
                                    } else {
                                        messages().up_to_date()
                                    },
                                    check.source
                                )
                            },
                            |available| {
                                messages().new_version(&available.manifest.version, &check.source)
                            },
                        );
                        this.updates.available = check.available;
                    }
                    Err(error) => this.updates.message = messages().check_failed(error),
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
        let task = cx.background_spawn(async move { update::download(&available) });
        self.updates.task = Some(cx.spawn(async move |this, cx| {
            let result = task.await;
            let _ = this.update(cx, |this, cx| {
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
        }));
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
        let task = cx
            .background_spawn(async move { update::install(&path, &available.manifest.installer) });
        self.updates.task = Some(cx.spawn(async move |this, cx| {
            let result = task.await;
            let _ = this.update(cx, |this, cx| {
                this.updates.busy = false;
                this.updates.message = match result {
                    Ok(()) => messages().installer_opened().into(),
                    Err(error) => messages().install_failed(error),
                };
                cx.notify();
            });
        }));
        cx.notify();
    }

    pub(super) fn update_settings(&self, cx: &mut Context<Self>) -> AnyElement {
        let busy = self.updates.busy;
        let hours = self.config.updates.interval_hours;
        v_flex()
            .gap(px(8.))
            .child(
                h_flex()
                    .gap(px(8.))
                    .justify_between()
                    .child(
                        div()
                            .font_semibold()
                            .child(messages().software_updates(env!("CARGO_PKG_VERSION"))),
                    )
                    .child(
                        controls::button("check-update")
                            .label(messages().check_updates())
                            .disabled(busy)
                            .on_click(cx.listener(|this, _, _, cx| this.check_update(cx))),
                    ),
            )
            .child(
                controls::segmented("update-interval")
                    .selected_index(if !self.config.updates.enabled {
                        0
                    } else if hours >= 168 {
                        3
                    } else if hours > 24 {
                        2
                    } else {
                        1
                    })
                    .child(Tab::new().label(messages().manual()))
                    .child(Tab::new().label(messages().daily()))
                    .child(Tab::new().label(messages().three_days()))
                    .child(Tab::new().label(messages().weekly()))
                    .on_click(cx.listener(|this, index: &usize, _, cx| {
                        this.config.updates.enabled = *index != 0;
                        this.config.updates.interval_hours = [24, 24, 72, 168][*index];
                        this.save(cx);
                        cx.notify();
                    })),
            )
            .when_some(self.updates.mirror_input.as_ref(), |view, input| {
                view.child(
                    h_flex()
                        .gap(px(8.))
                        .child(Input::new(input).small().flex_1())
                        .child(
                            controls::button("save-mirror")
                                .label(messages().save_mirror())
                                .disabled(busy)
                                .on_click(cx.listener(|this, _, _, cx| {
                                    let value = this
                                        .updates
                                        .mirror_input
                                        .as_ref()
                                        .unwrap()
                                        .read(cx)
                                        .value();
                                    match update::mirror_prefix(&value) {
                                        Ok(value) => {
                                            this.config.updates.mirror = value;
                                            this.save(cx);
                                            this.updates.message = messages().mirror_saved().into();
                                            this.updates.available = None;
                                            this.updates.installer = None;
                                        }
                                        Err(error) => this.updates.message = error.to_string(),
                                    }
                                    cx.notify();
                                })),
                        ),
                )
            })
            .child(
                div()
                    .text_size(px(11.))
                    .text_color(cx.theme().muted_foreground)
                    .child(messages().mirror_help()),
            )
            .when(!self.updates.message.is_empty(), |view| {
                view.child(div().text_size(px(12.)).child(self.updates.message.clone()))
            })
            .when_some(self.updates.available.as_ref(), |view, available| {
                let notes = available.manifest.notes_url.clone();
                view.child(
                    h_flex()
                        .gap(px(8.))
                        .child(
                            controls::button("release-notes")
                                .label(messages().release_notes())
                                .on_click(move |_, _, cx| cx.open_url(&notes)),
                        )
                        .child(if self.updates.installer.is_some() {
                            controls::button("install-update")
                                .primary()
                                .label(messages().install_update())
                                .disabled(busy)
                                .on_click(cx.listener(|this, _, _, cx| this.install_update(cx)))
                        } else {
                            controls::button("download-update")
                                .primary()
                                .label(messages().download_update())
                                .disabled(busy)
                                .on_click(cx.listener(|this, _, _, cx| this.download_update(cx)))
                        }),
                )
            })
            .into_any_element()
    }
}
