use super::*;
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
            self.updates.message = "刚刚已检查，请至少等待一分钟再试。".into();
            cx.notify();
            return;
        }
        self.config.updates.last_check = update::now();
        self.save(cx);
        self.updates.busy = true;
        self.updates.message = "正在检查更新…".into();
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
                                    if check.source.starts_with("镜像：") {
                                        "镜像暂未提供较新版本"
                                    } else {
                                        "已是最新版本"
                                    },
                                    check.source
                                )
                            },
                            |available| {
                                format!("发现 {} · {}", available.manifest.version, check.source)
                            },
                        );
                        this.updates.available = check.available;
                    }
                    Err(error) => this.updates.message = format!("检查失败：{error:#}"),
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
        self.updates.message = "正在后台下载安装包…".into();
        let task = cx.background_spawn(async move { update::download(&available) });
        self.updates.task = Some(cx.spawn(async move |this, cx| {
            let result = task.await;
            let _ = this.update(cx, |this, cx| {
                this.updates.busy = false;
                match result {
                    Ok(path) => {
                        this.updates.installer = Some(path);
                        this.updates.message = "安装包已下载并通过校验，可启动安装。".into();
                    }
                    Err(error) => this.updates.message = format!("下载失败：{error:#}"),
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
        self.updates.message = "正在重新校验并打开安装程序…".into();
        let task = cx
            .background_spawn(async move { update::install(&path, &available.manifest.installer) });
        self.updates.task = Some(cx.spawn(async move |this, cx| {
            let result = task.await;
            let _ = this.update(cx, |this, cx| {
                this.updates.busy = false;
                this.updates.message = match result {
                    Ok(()) => "安装程序已打开；完成后可重新启动 Githly。".into(),
                    Err(error) => format!("安装未启动：{error:#}；当前版本可继续使用"),
                };
                cx.notify();
            });
        }));
        cx.notify();
    }

    pub(super) fn update_settings(&self, cx: &mut Context<Self>) -> AnyElement {
        let busy = self.updates.busy;
        let hours = self.config.updates.interval_hours;
        v_flex().gap(px(8.)).child(
            h_flex().gap(px(8.)).justify_between()
                .child(div().font_semibold().child(format!("软件更新 · {}", env!("CARGO_PKG_VERSION"))))
                .child(controls::button("check-update").label("检查更新").disabled(busy)
                    .on_click(cx.listener(|this, _, _, cx| this.check_update(cx))))
        ).child(
            controls::segmented("update-interval")
                .selected_index(if !self.config.updates.enabled { 0 } else if hours >= 168 { 3 } else if hours > 24 { 2 } else { 1 })
                .child(Tab::new().label("手动"))
                .child(Tab::new().label("每天"))
                .child(Tab::new().label("每三天"))
                .child(Tab::new().label("每周"))
                .on_click(cx.listener(|this, index: &usize, _, cx| {
                    this.config.updates.enabled = *index != 0;
                    this.config.updates.interval_hours = [24, 24, 72, 168][*index];
                    this.save(cx);
                    cx.notify();
                }))
        ).when_some(self.updates.mirror_input.as_ref(), |view, input| {
            view.child(h_flex().gap(px(8.)).child(Input::new(input).small().flex_1())
                .child(controls::button("save-mirror").label("保存镜像").disabled(busy)
                    .on_click(cx.listener(|this, _, _, cx| {
                        let value = this.updates.mirror_input.as_ref().unwrap().read(cx).value();
                        match update::mirror_prefix(&value) {
                            Ok(value) => {
                                this.config.updates.mirror = value;
                                this.save(cx);
                                this.updates.message = "更新源设置已保存。".into();
                                this.updates.available = None;
                                this.updates.installer = None;
                            }
                            Err(error) => this.updates.message = error.to_string(),
                        }
                        cx.notify();
                    }))))
        }).child(div().text_size(px(11.)).text_color(cx.theme().muted_foreground)
            .child("镜像填 HTTPS 前缀，需支持在前缀后拼接完整 GitHub Release 地址；留空使用 GitHub。"))
        .when(!self.updates.message.is_empty(), |view| view.child(div().text_size(px(12.)).child(self.updates.message.clone())))
        .when_some(self.updates.available.as_ref(), |view, available| {
            let notes = available.manifest.notes_url.clone();
            view.child(h_flex().gap(px(8.))
                .child(controls::button("release-notes").label("更新说明").on_click(move |_, _, cx| cx.open_url(&notes)))
                .child(if self.updates.installer.is_some() {
                    controls::button("install-update").primary().label("安装更新…").disabled(busy)
                        .on_click(cx.listener(|this, _, _, cx| this.install_update(cx)))
                } else {
                    controls::button("download-update").primary().label("下载更新").disabled(busy)
                        .on_click(cx.listener(|this, _, _, cx| this.download_update(cx)))
                }))
        }).into_any_element()
    }
}
