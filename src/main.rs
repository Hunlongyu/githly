#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

use crate::{app_assets::AppAssets, i18n::messages};

mod app;
mod app_assets;
mod config;
mod git;
mod graph;
mod i18n;
mod motion;
#[cfg(windows)]
mod single_instance;
mod theme;
#[cfg(unix)]
mod unix_instance;
mod update;

use config::{Appearance, ConfigStore};
use gpui_kit::component::{ThemeMode, TitleBar};
use gpui_kit::*;

fn main() -> anyhow::Result<()> {
    let log_dir = directories::BaseDirs::new()
        .ok_or_else(|| anyhow::anyhow!(messages().appdata_missing()))?
        .data_local_dir()
        .join("githly")
        .join("logs");
    std::fs::create_dir_all(&log_dir)?;
    let (writer, _log_guard) =
        tracing_appender::non_blocking(tracing_appender::rolling::daily(log_dir, "githly.log"));
    tracing_subscriber::fmt()
        .with_ansi(false)
        .with_writer(writer)
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "warn".into()),
        )
        .init();
    std::panic::set_hook(Box::new(|panic| {
        tracing::error!("{panic}");
    }));
    let store = ConfigStore::for_app()?;
    let (config, error) = match store.load() {
        Ok(config) => (config, None),
        Err(error) => (
            Default::default(),
            Some(i18n::LocalizedText::diagnostic(error)),
        ),
    };
    i18n::set_language(config.language);
    let initial_project = std::env::args_os().nth(1).map(std::path::PathBuf::from);
    #[cfg(unix)]
    let Some(instance) = unix_instance::claim_or_forward(initial_project.as_deref())
        .inspect_err(|error| tracing::error!("{error:#}"))?
    else {
        return Ok(());
    };
    #[cfg(windows)]
    let Some(_instance_guard) = single_instance::claim_or_forward(initial_project.as_deref())
        .inspect_err(|error| tracing::error!("{error:#}"))?
    else {
        return Ok(());
    };
    application().with_assets(AppAssets).run(move |cx| {
        init(cx);
        motion::init(config.motion, cx);
        cx.set_app_identity("com.hunlongyu.githly", "Githly");
        cx.on_window_closed(|cx, _| {
            if cx.windows().is_empty() {
                cx.quit();
            }
        })
        .detach();
        cx.bind_keys([
            KeyBinding::new(
                if cfg!(target_os = "macos") {
                    "cmd-o"
                } else {
                    "ctrl-o"
                },
                app::OpenProject,
                None,
            ),
            KeyBinding::new(
                if cfg!(target_os = "macos") {
                    "cmd-,"
                } else {
                    "ctrl-,"
                },
                app::OpenSettings,
                None,
            ),
            KeyBinding::new(
                if cfg!(target_os = "macos") {
                    "cmd-f"
                } else {
                    "ctrl-f"
                },
                app::FocusHistory,
                None,
            ),
            KeyBinding::new(
                if cfg!(target_os = "macos") {
                    "cmd-b"
                } else {
                    "ctrl-b"
                },
                app::ToggleBranches,
                None,
            ),
        ]);
        let mode = match config.appearance {
            Appearance::Light => ThemeMode::Light,
            Appearance::Dark => ThemeMode::Dark,
            Appearance::System => cx.window_appearance().into(),
        };
        theme::apply(mode, None, cx);
        let bounds = Bounds::centered(None, size(px(1280.), px(820.)), cx);
        open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                app_id: Some("githly".into()),
                window_min_size: Some(size(px(900.), px(600.))),
                titlebar: Some(TitlebarOptions {
                    title: Some("Githly".into()),
                    ..TitleBar::title_bar_options()
                }),
                ..TitleBar::window_options()
            },
            cx,
            move |window, cx| {
                let panel = cx.new(|cx| {
                    app::GitPanel::new(store, config, error, initial_project, window, cx)
                });
                #[cfg(windows)]
                single_instance::listen(window, panel.downgrade(), cx)
                    .unwrap_or_else(|error| panic!("{}: {error:#}", messages().register_launch()));
                #[cfg(unix)]
                instance.listen(window, panel.downgrade(), cx);
                panel
            },
        )
        .unwrap_or_else(|error| panic!("{}: {error:#}", messages().create_window()));
        cx.activate(true);
    });
    Ok(())
}
