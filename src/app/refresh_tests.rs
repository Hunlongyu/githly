use super::GitPanel;
use crate::config::{Config, ConfigStore};
use crate::git::history_tests::{command, fixture};
use gpui_kit::test::TestWindowExt;
use gpui_kit::{AppContext, TestAppContext, WindowOptions, point, px};
use std::sync::Arc;

#[gpui_kit::test]
fn startup_and_history_wait_for_user_selection(cx: &mut TestAppContext) {
    cx.executor().allow_parking();
    cx.update(gpui_kit::init);
    let (directory, _) = fixture(2);
    let storage = tempfile::tempdir().unwrap();
    let root = directory.path().to_path_buf();
    let mut config = Config::default();
    config.remember_project(&root);
    let (_window, panel) = cx.update(|cx| {
        gpui_kit::open_window(WindowOptions::default(), cx, |window, cx| {
            cx.new(|cx| {
                GitPanel::new(
                    ConfigStore {
                        path: storage.path().join("config.json"),
                    },
                    config,
                    None,
                    None,
                    window,
                    cx,
                )
            })
        })
        .unwrap()
    });

    wait_for_git(cx, &panel, |panel| panel.git.is_some());
    panel.update(cx, |panel, cx| {
        assert!(panel.repository.is_none() && !panel.loading);
        panel.load_project(root, cx);
    });
    wait_for_git(cx, &panel, |panel| panel.repository.is_some());
    panel.update(cx, |panel, cx| {
        assert_eq!(panel.repository.as_ref().unwrap().commits.len(), 2);
        assert!(panel.selected_commit.is_none() && !panel.detail_open);
        panel.select_commit(0, cx);
        assert!(panel.selected_commit.is_some() && panel.detail_open);
        panel.browse_branch(crate::git::HistoryScope::All, cx);
    });
    wait_for_git(cx, &panel, |panel| {
        panel.repository.as_ref().unwrap().history_scope == crate::git::HistoryScope::All
    });
    panel.update(cx, |panel, cx| {
        assert!(panel.selected_commit.is_none() && !panel.detail_open);
        panel.release_project(cx);
    });
}

#[gpui_kit::test]
fn worktree_dropdown_opens_a_linked_directory(cx: &mut TestAppContext) {
    cx.executor().allow_parking();
    cx.update(gpui_kit::init);
    let (repo, git) = fixture(1);
    let storage = tempfile::tempdir().unwrap();
    let linked = storage.path().join("linked");
    command(
        &git,
        repo.path(),
        &["worktree", "add", "-b", "linked", linked.to_str().unwrap()],
    );
    let (window, panel) = cx.update(|cx| {
        gpui_kit::open_window(WindowOptions::default(), cx, |window, cx| {
            cx.new(|cx| {
                GitPanel::new(
                    ConfigStore {
                        path: storage.path().join("config.json"),
                    },
                    Config::default(),
                    None,
                    Some(repo.path().to_owned()),
                    window,
                    cx,
                )
            })
        })
        .unwrap()
    });
    wait_for_git(cx, &panel, |panel| panel.repository.is_some());
    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        window.click("worktrees", cx);
        assert!(
            window.find("popup-menu").bounds().top() >= window.find("worktrees").bounds().bottom()
        );
    })
    .unwrap();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    loop {
        cx.run_until_parked();
        let ready = cx
            .update_window(window, |_, window, cx| {
                window.render_frame(cx);
                window.within("popup-menu").try_find(1_usize).is_some()
            })
            .unwrap();
        if ready {
            break;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "worktree menu did not load"
        );
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    cx.update_window(window, |_, window, cx| {
        window.within("popup-menu").click(1_usize, cx);
        assert!(window.try_find("popup-menu").is_none());
    })
    .unwrap();
    wait_for_git(cx, &panel, |panel| {
        panel
            .repository
            .as_ref()
            .is_some_and(|repo| repo.branch == "linked")
    });
    panel.update(cx, |panel, cx| panel.release_project(cx));
}

#[gpui_kit::test]
fn change_groups_scroll_together_with_visible_headers(cx: &mut TestAppContext) {
    use gpui_kit::{Bounds, ScrollStrategy, WindowBounds, size};

    cx.update(gpui_kit::init);
    let (directory, git) = fixture(1);
    let mut repo = futures_lite::future::block_on(git.open(directory.path(), 46)).unwrap();
    repo.changes = (0..400)
        .map(|i| crate::git::ChangedFile {
            status: if i < 200 { " M" } else { "??" }.into(),
            path: format!("file-{i}.txt"),
            original_path: None,
        })
        .collect();
    let (window, panel) = cx.update(|cx| {
        gpui_kit::open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds {
                    origin: Default::default(),
                    size: size(px(900.), px(600.)),
                })),
                ..Default::default()
            },
            cx,
            |window, cx| {
                cx.new(|cx| {
                    let mut panel = GitPanel::new(
                        ConfigStore {
                            path: directory.path().join("config.json"),
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
                    panel.repository = Some(Arc::new(repo));
                    panel.filter_history(cx);
                    panel
                })
            },
        )
        .unwrap()
    });
    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        let scroll = panel.read(cx).changes_scroll.clone();
        let viewport = scroll.0.borrow().base_handle.bounds();
        let header =
            |window: &gpui_kit::Window, group: usize| window.find(("file-group", group)).bounds();
        let upper = header(window, 0);
        assert_eq!(upper.bottom(), viewport.top());
        assert_eq!(header(window, 1).bottom(), viewport.bottom());
        window.scroll(
            ("file-group", 1_usize),
            gpui_kit::ScrollDelta::Pixels(point(px(0.), px(-140.))),
            cx,
        );
        assert!(scroll.0.borrow().base_handle.offset().y < px(0.));

        scroll.scroll_to_item_strict(200, ScrollStrategy::Center);
        window.render_frame(cx);
        let middle = header(window, 1);
        assert!(middle.top() > upper.bottom() && middle.bottom() < viewport.bottom());
        scroll.scroll_to_bottom();
        window.render_frame(cx);
        assert_eq!(header(window, 0), upper);
        assert_eq!(header(window, 1).top(), upper.bottom());

        panel.update(cx, |panel, cx| {
            Arc::make_mut(panel.repository.as_mut().unwrap())
                .changes
                .retain(|file| file.status == "??" || file.path == "file-0.txt");
            cx.notify();
        });
        window.render_frame(cx);
        scroll.scroll_to_item_strict(0, ScrollStrategy::Top);
        window.render_frame(cx);
        assert_eq!(header(window, 1).top(), upper.bottom() + px(28.));
        window.click(("file-group", 0_usize), cx);
        assert_eq!(header(window, 1).top(), upper.bottom());
        window.click(("file-group", 1_usize), cx);
        assert_eq!(panel.read(cx).collapsed_change_groups, [true, true]);
    })
    .unwrap();
}

#[gpui_kit::test]
fn scrolling_near_bottom_loads_more_and_timer_refreshes_local_changes(cx: &mut TestAppContext) {
    cx.executor().allow_parking();
    cx.update(gpui_kit::init);
    let (directory, _) = fixture(625);
    let options = WindowOptions {
        window_bounds: Some(gpui_kit::WindowBounds::Windowed(gpui_kit::Bounds {
            origin: Default::default(),
            size: gpui_kit::size(px(1280.), px(820.)),
        })),
        ..Default::default()
    };
    let (window, panel) = cx.update(|cx| {
        gpui_kit::open_window(options, cx, |window, cx| {
            cx.new(|cx| {
                GitPanel::new(
                    ConfigStore {
                        path: directory.path().join("test-config.json"),
                    },
                    Config::default(),
                    Some("test".into()),
                    Some(directory.path().to_owned()),
                    window,
                    cx,
                )
            })
        })
        .unwrap()
    });
    wait_for_git(cx, &panel, |panel| panel.repository.is_some());
    cx.update_window(window, |_, window, cx| window.render_frame(cx))
        .unwrap();
    cx.run_until_parked();
    cx.update_window(window, |_, window, cx| {
        panel.update(cx, |panel, _| {
            assert_eq!(panel.repository.as_ref().unwrap().commits.len(), 46);
            assert!(!panel.history_loading && !panel.history_near_end);
            panel
                .history_scroll
                .scroll_to_item(45, gpui_kit::ScrollStrategy::Bottom)
        });
        window.render_frame(cx);
        window.render_frame(cx);
    })
    .unwrap();
    wait_for_git(cx, &panel, |panel| {
        panel.repository.as_ref().unwrap().commits.len() == 146
    });
    panel.update(cx, |panel, _| assert!(panel.history_error.is_none()));
    std::fs::write(directory.path().join("automatic.txt"), "new file").unwrap();
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
    let previous = futures_lite::future::block_on(git.open(directory.path(), 46)).unwrap();
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
    let next = futures_lite::future::block_on(git.refresh(&previous, true, 46))
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
    let repo = Arc::new(futures_lite::future::block_on(git.open(directory.path(), 46)).unwrap());
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
