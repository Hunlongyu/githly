use super::GitPanel;
use crate::config::{Config, ConfigStore};
use crate::git::history_tests::{command, fixture};
use gpui_kit::test::TestWindowExt;
use gpui_kit::{AppContext, TestAppContext, WindowOptions, point, px};
use std::sync::Arc;

#[gpui_kit::test]
fn scrolling_near_bottom_loads_more_and_timer_refreshes_local_changes(cx: &mut TestAppContext) {
    cx.executor().allow_parking();
    cx.update(gpui_kit::init);
    let (directory, git) = fixture(625);
    let repo = Arc::new(futures_lite::future::block_on(git.open(directory.path())).unwrap());
    let (window, panel) = cx.update(|cx| {
        gpui_kit::open_window(WindowOptions::default(), cx, |window, cx| {
            cx.new(|cx| {
                let mut panel = GitPanel::new(
                    ConfigStore {
                        path: directory.path().join("test-config.json"),
                    },
                    Config::default(),
                    Some("test".into()),
                    None,
                    window,
                    cx,
                );
                panel.detect_task = None;
                panel.detecting = false;
                panel.git = Some(git);
                panel.repository = Some(repo);
                panel.filter_history(cx);
                panel
            })
        })
        .unwrap()
    });
    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        panel.update(cx, |panel, _| {
            panel
                .history_scroll
                .scroll_to_item(199, gpui_kit::ScrollStrategy::Bottom)
        });
        window.render_frame(cx);
        window.render_frame(cx);
    })
    .unwrap();
    wait_for_git(cx, &panel, |panel| {
        panel.repository.as_ref().unwrap().commits.len() >= 400
    });
    panel.update(cx, |panel, _| assert!(panel.history_error.is_none()));
    std::fs::write(directory.path().join("automatic.txt"), "new file").unwrap();
    panel.update(cx, |panel, cx| panel.start_auto_refresh(cx));
    cx.run_until_parked();
    cx.executor()
        .advance_clock(std::time::Duration::from_secs(60));
    wait_for_git(cx, &panel, |panel| {
        panel
            .repository
            .as_ref()
            .unwrap()
            .changes
            .iter()
            .any(|file| file.path == "automatic.txt")
    });
    panel.update(cx, |panel, cx| panel.release_project(cx));
}

fn wait_for_git(
    cx: &mut TestAppContext,
    panel: &gpui_kit::Entity<GitPanel>,
    predicate: impl Fn(&GitPanel) -> bool,
) {
    // Real Git processes use OS time; the test executor's virtual deadline can
    // otherwise expire before the process has had a chance to run.
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    loop {
        cx.run_until_parked();
        if panel.update(cx, |panel, _| predicate(panel)) {
            break;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "Git UI update timed out"
        );
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
}

#[gpui_kit::test]
fn refresh_keeps_commit_diff_and_scroll_anchor_and_close_releases_session(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let (directory, git) = fixture(80);
    let previous = futures_lite::future::block_on(git.open(directory.path())).unwrap();
    command(
        &git,
        directory.path(),
        &[
            "-c",
            "core.hooksPath=NUL",
            "commit",
            "--allow-empty",
            "-m",
            "new head",
        ],
    );
    let next = futures_lite::future::block_on(git.refresh(&previous, true))
        .unwrap()
        .unwrap();
    let (window, panel) = cx.update(|cx| {
        gpui_kit::open_window(WindowOptions::default(), cx, |window, cx| {
            cx.new(|cx| {
                let mut panel = GitPanel::new(
                    ConfigStore {
                        path: directory.path().join("test-config.json"),
                    },
                    Config::default(),
                    Some("test".into()),
                    None,
                    window,
                    cx,
                );
                panel.detect_task = None;
                panel.detecting = false;
                panel.git = Some(git.clone());
                panel.selected_commit = Some(previous.commits[12].clone());
                panel.detail_open = true;
                panel.repository = Some(Arc::new(previous));
                panel.filter_history(cx);
                panel
            })
        })
        .unwrap()
    });
    panel.update(cx, |panel, cx| {
        let selected_id = panel.selected_commit.as_ref().unwrap().id.clone();
        let diff = panel.diff.clone();
        panel
            .history_scroll
            .0
            .borrow()
            .base_handle
            .set_offset(point(px(0.), px(-327.)));
        let retired = panel.apply_refresh(next, cx);
        assert_eq!(panel.selected_commit.as_ref().unwrap().id, selected_id);
        assert_eq!(panel.diff, diff);
        assert_eq!(
            panel.history_scroll.0.borrow().base_handle.offset().y,
            px(-359.)
        );
        drop(retired);
        panel.start_auto_refresh(cx);
        assert!(panel.refresh_task.is_some());
        panel.history_task = Some(cx.spawn(async |_, _| std::future::pending().await));
        let weak = Arc::downgrade(panel.repository.as_ref().unwrap());
        let generation = panel.project_generation;
        panel.release_project(cx);
        assert!(panel.project_generation > generation);
        assert!(
            panel.refresh_task.is_none()
                && panel.history_task.is_none()
                && panel.load_task.is_none()
        );
        assert!(panel.repository.is_none() && panel.history_indices.capacity() == 0);
        cx.background_spawn(async move { drop(weak) }).detach();
    });
    cx.run_until_parked();
    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
    })
    .unwrap();
}

#[gpui_kit::test]
fn repeated_scroll_requests_are_coalesced_and_cancelled_on_close(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let (directory, git) = fixture(625);
    let repo = Arc::new(futures_lite::future::block_on(git.open(directory.path())).unwrap());
    let weak = Arc::downgrade(&repo);
    let (_, panel) = cx.update(|cx| {
        gpui_kit::open_window(WindowOptions::default(), cx, |window, cx| {
            cx.new(|cx| {
                let mut panel = GitPanel::new(
                    ConfigStore {
                        path: directory.path().join("test-config.json"),
                    },
                    Config::default(),
                    Some("test".into()),
                    None,
                    window,
                    cx,
                );
                panel.detect_task = None;
                panel.git = Some(git);
                panel.repository = Some(repo);
                panel.filter_history(cx);
                panel
            })
        })
        .unwrap()
    });
    panel.update(cx, |panel, cx| {
        for _ in 0..20 {
            panel.load_more_history(cx);
        }
        assert!(panel.history_loading);
        panel.release_project(cx);
    });
    cx.run_until_parked();
    assert!(
        weak.upgrade().is_none(),
        "closed session must not be retained by a pending reader"
    );
    panel.update(cx, |panel, _| assert!(panel.repository.is_none()));
}
