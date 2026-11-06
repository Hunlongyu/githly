use super::{GitPanel, diff_data, image_diff, view};
use crate::{
    config::{Config, ConfigStore},
    git::ChangedFile,
};
use gpui_kit::test::TestWindowExt;
use gpui_kit::{
    AppContext, Bounds, Context, IntoElement, ParentElement, Render, RenderImage, ScrollStrategy,
    Styled, TestAppContext, UniformListScrollHandle, Window, WindowBounds, WindowOptions, div, px,
    size,
};
use std::{cell::RefCell, ops::Range, rc::Rc};

struct FileListHost {
    scroll: UniformListScrollHandle,
    requested: Rc<RefCell<Vec<Range<usize>>>>,
}

impl Render for FileListHost {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let requested = self.requested.clone();
        view::detail_file_list(360, &self.scroll, move |range, _, _| {
            requested.borrow_mut().push(range.clone());
            range
                .map(|index| div().h(px(28.)).child(index.to_string()).into_any_element())
                .collect()
        })
        .w_full()
    }
}

#[gpui_kit::test]
fn large_diff_file_list_only_constructs_visible_rows(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let requested = Rc::new(RefCell::new(Vec::new()));
    let (window, host) = cx.update(|cx| {
        gpui_kit::open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds {
                    origin: Default::default(),
                    size: size(px(205.), px(280.)),
                })),
                ..Default::default()
            },
            cx,
            |_, cx| {
                cx.new(|_| FileListHost {
                    scroll: UniformListScrollHandle::new(),
                    requested: requested.clone(),
                })
            },
        )
        .unwrap()
    });
    cx.run_until_parked();
    cx.update_window(window, |_, window, cx| {
        for _ in 0..3 {
            window.render_frame(cx);
        }
        let ranges = requested.borrow();
        assert!(!ranges.is_empty());
        assert!(
            ranges.iter().all(|range| range.len() <= 12),
            "rendered beyond viewport: {ranges:?}"
        );
        assert!(ranges.iter().all(|range| range.end <= 12));
        drop(ranges);
        requested.borrow_mut().clear();
        host.update(cx, |host, cx| {
            host.scroll.scroll_to_item(359, ScrollStrategy::Bottom);
            cx.notify();
        });
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(window, |_, window, cx| {
        for _ in 0..3 {
            window.render_frame(cx);
        }
        assert_eq!(host.read(cx).scroll.is_scrolled_to_end(), Some(true));
        let ranges = requested.borrow();
        assert!(
            ranges.iter().any(|range| range.contains(&359)),
            "last file unreachable: {ranges:?}"
        );
        assert!(ranges.iter().all(|range| range.len() <= 12));
    })
    .unwrap();
}

#[gpui_kit::test]
fn closing_detail_drops_document_state_and_pending_requests(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let directory = tempfile::tempdir().unwrap();
    let (window, panel) = cx.update(|cx| {
        gpui_kit::open_window(WindowOptions::default(), cx, |window, cx| cx.new(|cx| {
            let mut panel = GitPanel::new(
                ConfigStore { path: directory.path().join("config.json") },
                Config::default(), Some("Test: do not save configuration".into()), None, window, cx,
            );
            panel.detect_task = None;
            panel.detecting = false;
            panel.detail_open = true;
            panel.selected_file = Some(0);
            panel.files = (0..360).map(|index| ChangedFile {
                path: format!("src/file-{index}.cpp"), original_path: None, status: "M".into(),
            }).collect();
            panel.diff.update(cx, |diff, cx| diff.set_files(diff_data::parse(
                "diff --git a/QSpin_en.ts b/QSpin_en.ts\n--- a/QSpin_en.ts\n+++ b/QSpin_en.ts\n@@ -1 +1 @@\n-<translation>Old</translation>\n+<translation>New</translation>\n"
            ).unwrap(), cx));
            panel.detail_task = Some(cx.spawn(async |_, _| std::future::pending().await));
            panel.diff_task = Some(cx.spawn(async |_, _| std::future::pending().await));
            panel
        })).unwrap()
    });
    let old_state = cx.update(|cx| panel.read(cx).diff.downgrade());
    let image = std::sync::Arc::new(RenderImage::new(vec![image::Frame::new(
        image::RgbaImage::new(2, 2),
    )]));
    let old_image = std::sync::Arc::downgrade(&image);
    panel.update(cx, |panel, cx| {
        panel.image_diff = Some(image_diff::ImageDiff {
            original: image_diff::ImageSide::Missing,
            modified: image_diff::test_preview(image),
        });
        assert_eq!(panel.diff.read(cx).files().len(), 1);
        let generation = panel.generation;
        panel.close_detail(cx);
        assert!(!panel.detail_open);
        assert_eq!(panel.files.capacity(), 0);
        assert!(panel.diff.read(cx).files().is_empty());
        assert!(panel.detail_task.is_none() && panel.diff_task.is_none());
        assert!(panel.selected_file.is_none());
        assert!(panel.image_diff.is_none());
        assert!(
            panel.generation > generation,
            "stale asynchronous results must be rejected"
        );
    });
    cx.run_until_parked();
    cx.update_window(window, |_, window, cx| {
        for _ in 0..3 {
            window.render_frame(cx);
        }
        assert!(
            old_state.upgrade().is_none(),
            "closed Diff state is still retained"
        );
        assert!(
            old_image.upgrade().is_none(),
            "closed image bytes must be released"
        );
    })
    .unwrap();
}

#[test]
fn simple_diff_preserves_changes_without_loading_a_language_grammar() {
    for path in ["QSpin_en.ts", "MainWindow.cpp", "删除文件.rs"] {
        let patch = format!(
            "diff --git a/{path} b/{path}\n--- a/{path}\n+++ b/{path}\n@@ -1 +1 @@\n-old\n+new\n"
        );
        let files = diff_data::parse(&patch).unwrap();
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].path().as_str(), path);
        assert_eq!(files[0].language().as_str(), "text");
        assert_eq!((files[0].additions(), files[0].deletions()), (1, 1));
    }
}
