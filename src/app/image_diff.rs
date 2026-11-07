use crate::git::{ChangedFile, Git};
use crate::i18n::{Locale, LocalizedText, messages};
use anyhow::{Context as _, Result};
use bstr::ByteSlice;
use gpui_kit::{
    AnyElement, App, Bounds, InteractiveElement, IntoElement, ObjectFit, ParentElement,
    RenderImage, Styled, TestSupportExt, Window, canvas,
    component::{ActiveTheme, h_flex, v_flex},
    div, fill, point, px, size,
};
use image::{GenericImageView, ImageReader, RgbaImage};
use std::{io::Cursor, path::Path, sync::Arc};

const PREVIEW_EDGE: u32 = 1600;
const DECODE_LIMIT: u64 = 64 * 1024 * 1024;

#[cfg(all(test, feature = "ui-tests"))]
pub(super) fn test_preview(pixels: Arc<RenderImage>) -> ImageSide {
    ImageSide::Ready(ImagePreview {
        pixels,
        width: 2,
        height: 2,
        byte_len: 16,
        first_frame: false,
    })
}

pub(super) struct ImageDiff {
    pub original: ImageSide,
    pub modified: ImageSide,
}

pub(super) enum ImageSide {
    Ready(ImagePreview),
    Missing,
    Error(LocalizedText),
}

pub(super) struct ImagePreview {
    pub pixels: Arc<RenderImage>,
    width: u32,
    height: u32,
    byte_len: usize,
    first_frame: bool,
}

pub(super) fn is_image(path: impl AsRef<[u8]>) -> bool {
    path.as_ref()
        .to_path()
        .ok()
        .and_then(Path::extension)
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| {
            matches!(
                ext.to_ascii_lowercase().as_str(),
                "png" | "jpg" | "jpeg" | "gif" | "webp" | "bmp" | "ico" | "tif" | "tiff" | "svg"
            )
        })
}

pub(super) async fn load(
    git: &Git,
    root: &Path,
    original_revision: Option<&str>,
    modified_revision: Option<&str>,
    file: &ChangedFile,
) -> ImageDiff {
    let original_path = file.original_path.as_deref().unwrap_or(&file.path);
    // Decode sequentially, dropping encoded bytes before loading the other side.
    let original = if let Some(revision) = original_revision {
        ImageSide::from_bytes(
            git.image_file(root, Some(revision), original_path).await,
            original_path,
        )
    } else {
        ImageSide::Missing
    };
    let modified = ImageSide::from_bytes(
        git.image_file(root, modified_revision, &file.path).await,
        &file.path,
    );
    ImageDiff { original, modified }
}

impl ImageSide {
    pub(super) fn from_bytes(bytes: Result<Option<Vec<u8>>>, path: impl AsRef<[u8]>) -> Self {
        match bytes.and_then(|bytes| bytes.map(|bytes| decode(&bytes, path)).transpose()) {
            Ok(Some(preview)) => Self::Ready(preview),
            Ok(None) => Self::Missing,
            Err(error) => Self::Error(LocalizedText::with_error(error, |l, e| {
                l.image_preview_failed(e)
            })),
        }
    }
}

fn decode(bytes: &[u8], path: impl AsRef<[u8]>) -> Result<ImagePreview> {
    let svg = path
        .as_ref()
        .to_path()?
        .extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("svg"));
    let (mut pixels, width, height, first_frame) = if svg {
        let (pixels, width, height) = decode_svg(bytes)?;
        (pixels, width, height, false)
    } else {
        let mut reader = ImageReader::new(Cursor::new(bytes)).with_guessed_format()?;
        let first_frame = matches!(
            reader.format(),
            Some(image::ImageFormat::Gif | image::ImageFormat::WebP)
        );
        let mut limits = image::Limits::default();
        limits.max_alloc = Some(DECODE_LIMIT);
        limits.max_image_width = Some(16384);
        limits.max_image_height = Some(16384);
        reader.limits(limits);
        let decoded = reader
            .decode()
            .with_context(|| LocalizedText::new(Locale::invalid_image))?;
        let (width, height) = decoded.dimensions();
        let preview = if width > PREVIEW_EDGE || height > PREVIEW_EDGE {
            decoded.thumbnail(PREVIEW_EDGE, PREVIEW_EDGE)
        } else {
            decoded
        };
        (preview.into_rgba8(), width, height, first_frame)
    };
    // GPUI's decoded image source uses BGRA; avoid the global encoded-image cache.
    for pixel in pixels.as_chunks_mut::<4>().0 {
        pixel.swap(0, 2);
    }
    Ok(ImagePreview {
        pixels: Arc::new(RenderImage::new(vec![image::Frame::new(pixels)])),
        width,
        height,
        byte_len: bytes.len(),
        first_frame,
    })
}

fn decode_svg(bytes: &[u8]) -> Result<(RgbaImage, u32, u32)> {
    use resvg::{tiny_skia, usvg};
    let mut options = usvg::Options::default();
    // A Git blob must not resolve external files from the current working directory.
    options.image_href_resolver.resolve_string = Box::new(|_, _| None);
    if std::str::from_utf8(bytes).is_ok_and(|svg| svg.contains("<text")) {
        static FONTS: std::sync::OnceLock<Arc<usvg::fontdb::Database>> = std::sync::OnceLock::new();
        options.fontdb = FONTS
            .get_or_init(|| {
                let mut fonts = usvg::fontdb::Database::new();
                fonts.load_system_fonts();
                Arc::new(fonts)
            })
            .clone();
    }
    let tree = usvg::Tree::from_data(bytes, &options)
        .with_context(|| LocalizedText::new(Locale::parse_svg))?;
    let original = tree.size().to_int_size();
    let scale = (PREVIEW_EDGE as f32 / tree.size().width().max(tree.size().height())).min(1.);
    let width = (tree.size().width() * scale).round().max(1.) as u32;
    let height = (tree.size().height() * scale).round().max(1.) as u32;
    let mut pixmap = tiny_skia::Pixmap::new(width, height)
        .with_context(|| LocalizedText::new(Locale::svg_size))?;
    resvg::render(
        &tree,
        tiny_skia::Transform::from_scale(scale, scale),
        &mut pixmap.as_mut(),
    );
    // tiny-skia stores premultiplied RGBA; GPUI expects straight alpha.
    let rgba = pixmap
        .pixels()
        .iter()
        .flat_map(|pixel| {
            let pixel = pixel.demultiply();
            [pixel.red(), pixel.green(), pixel.blue(), pixel.alpha()]
        })
        .collect();
    Ok((
        RgbaImage::from_raw(width, height, rgba)
            .with_context(|| LocalizedText::new(Locale::svg_pixels))?,
        original.width(),
        original.height(),
    ))
}

impl ImageDiff {
    pub(super) fn release_textures(&self, window: &mut Window) {
        for side in [&self.original, &self.modified] {
            if let ImageSide::Ready(preview) = side {
                let _ = window.drop_image(preview.pixels.clone());
            }
        }
    }

    pub(super) fn view(&self, cx: &App) -> impl IntoElement {
        h_flex()
            .id("image-diff")
            .flex_1()
            .min_h_0()
            .w_full()
            .overflow_hidden()
            .child(
                self.original
                    .view("image-original", messages().before(), cx),
            )
            .child(
                div()
                    .w(px(1.))
                    .h_full()
                    .flex_shrink_0()
                    .bg(cx.theme().border),
            )
            .child(self.modified.view("image-modified", messages().after(), cx))
    }
}

impl ImageSide {
    fn view(&self, id: &'static str, title: &'static str, cx: &App) -> impl IntoElement {
        let mut header = h_flex()
            .h(px(28.))
            .flex_shrink_0()
            .px(px(12.))
            .gap(px(8.))
            .border_b_1()
            .border_color(cx.theme().border)
            .text_size(px(11.))
            .text_color(cx.theme().muted_foreground)
            .child(div().flex_shrink_0().child(title));
        let content = match self {
            Self::Ready(preview) => {
                let bytes = if preview.byte_len >= 1024 * 1024 {
                    format!("{:.1} MiB", preview.byte_len as f64 / (1024. * 1024.))
                } else {
                    format!("{:.1} KiB", preview.byte_len as f64 / 1024.)
                };
                header = header.child(div().flex_1().min_w_0().truncate().child(format!(
                    "{} × {} · {}{}",
                    preview.width,
                    preview.height,
                    bytes,
                    if preview.first_frame {
                        messages().first_frame()
                    } else {
                        ""
                    },
                )));
                div()
                    .id("image-preview")
                    .test_support()
                    .size_full()
                    .overflow_hidden()
                    .child(image_canvas(preview.pixels.clone(), cx))
                    .into_any_element()
            }
            Self::Missing => empty_side(messages().missing_image().into(), cx),
            Self::Error(message) => empty_side(message.to_string(), cx),
        };
        v_flex()
            .id(id)
            .test_support()
            .flex_1()
            .min_w_0()
            .h_full()
            .overflow_hidden()
            .child(header)
            .child(div().flex_1().min_h_0().w_full().child(content))
    }
}

fn empty_side(message: String, cx: &App) -> AnyElement {
    v_flex()
        .size_full()
        .justify_center()
        .items_center()
        .p(px(16.))
        .text_size(px(12.))
        .text_color(cx.theme().muted_foreground)
        .child(message)
        .into_any_element()
}

fn image_canvas(pixels: Arc<RenderImage>, cx: &App) -> impl IntoElement {
    let base = cx.theme().background;
    let alternate = cx.theme().muted;
    canvas(
        |_, _, _| (),
        move |bounds, _, window, _| {
            window.paint_quad(fill(bounds, base));
            window.paint_quad(fill(
                bounds,
                gpui_kit::checkerboard(alternate, 12. * window.scale_factor()),
            ));
            // Use the actual viewport, without an intrinsic image layout size.
            let area = Bounds::new(
                bounds.origin + point(px(12.), px(12.)),
                size(
                    (bounds.size.width - px(24.)).max(px(0.)),
                    (bounds.size.height - px(24.)).max(px(0.)),
                ),
            );
            let fitted = ObjectFit::Contain.get_bounds(area, pixels.size(0));
            if let Err(error) =
                window.paint_image(area, fitted, Default::default(), pixels.clone(), 0, false)
            {
                tracing::warn!("{}", messages().draw_image_failed(error));
            }
        },
    )
    .size_full()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn png_preview_preserves_alpha_and_converts_to_bgra() {
        let input = RgbaImage::from_pixel(2, 3, image::Rgba([200, 40, 10, 128]));
        let mut bytes = Cursor::new(Vec::new());
        input.write_to(&mut bytes, image::ImageFormat::Png).unwrap();
        let preview = decode(bytes.get_ref(), "文件.PNG").unwrap();
        assert_eq!((preview.width, preview.height), (2, 3));
        assert_eq!(
            &preview.pixels.as_bytes(0).unwrap()[..4],
            &[10, 40, 200, 128]
        );
        assert!(is_image("文件.PNG"));
    }

    #[test]
    fn svg_preview_scales_and_keeps_transparency() {
        let preview = decode(br##"<svg xmlns="http://www.w3.org/2000/svg" width="3200" height="1600"><rect width="3200" height="1600" fill="#ff0000" opacity="0.5"/></svg>"##, "icon.svg").unwrap();
        assert_eq!((preview.width, preview.height), (3200, 1600));
        assert_eq!(preview.pixels.as_bytes(0).unwrap().len(), 1600 * 800 * 4);
        assert_eq!(&preview.pixels.as_bytes(0).unwrap()[..3], &[0, 0, 255]);
        assert!((preview.pixels.as_bytes(0).unwrap()[3] as i16 - 128).abs() <= 1);
    }

    #[test]
    fn raster_preview_is_bounded_and_excessive_dimensions_are_rejected() {
        for (width, accepted) in [(3200, true), (16385, false)] {
            let input = RgbaImage::from_pixel(width, 2, image::Rgba([0, 255, 0, 255]));
            let mut bytes = Cursor::new(Vec::new());
            input.write_to(&mut bytes, image::ImageFormat::Png).unwrap();
            let result = decode(bytes.get_ref(), "wide.png");
            assert_eq!(result.is_ok(), accepted);
            if let Ok(preview) = result {
                assert_eq!(preview.pixels.as_bytes(0).unwrap().len(), 1600 * 4);
                assert_eq!((preview.width, preview.height), (3200, 2));
            }
        }
    }

    #[test]
    fn missing_and_corrupt_images_remain_distinct() {
        assert!(matches!(
            ImageSide::from_bytes(Ok(None), "removed.png"),
            ImageSide::Missing
        ));
        assert!(matches!(
            ImageSide::from_bytes(Ok(Some(b"broken".to_vec())), "bad.png"),
            ImageSide::Error(_)
        ));
    }
}

#[cfg(all(test, feature = "ui-tests"))]
mod layout_tests {
    use super::*;
    use gpui_kit::{
        AppContext, Context, Render, TestAppContext, Window, WindowBounds, WindowOptions,
        test::TestWindowExt,
    };

    struct Host {
        images: ImageDiff,
        width: f32,
    }

    impl Render for Host {
        fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            v_flex()
                .w(px(self.width))
                .h(px(300.))
                .child(self.images.view(cx))
        }
    }

    #[gpui_kit::test]
    fn images_stay_visible_in_equal_panes_and_release_textures(cx: &mut TestAppContext) {
        cx.update(gpui_kit::init);
        let (window, host) = cx.update(|cx| {
            gpui_kit::open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(Bounds::new(
                        Default::default(),
                        size(px(800.), px(400.)),
                    ))),
                    ..Default::default()
                },
                cx,
                |_, cx| {
                    cx.new(|_| Host {
                        images: ImageDiff {
                            original: test_preview(Arc::new(RenderImage::new(vec![
                                image::Frame::new(RgbaImage::new(2, 2)),
                            ]))),
                            modified: test_preview(Arc::new(RenderImage::new(vec![
                                image::Frame::new(RgbaImage::new(2, 2)),
                            ]))),
                        },
                        width: 640.,
                    })
                },
            )
            .unwrap()
        });
        cx.update_window(window, |_, window, cx| {
            for width in [640., 480., 780.] {
                host.update(cx, |host, cx| {
                    host.width = width;
                    cx.notify();
                });
                window.render_frame(cx);
                let original = window.find("image-original").bounds();
                let modified = window.find("image-modified").bounds();
                assert!((original.size.width - modified.size.width).abs() <= px(1.));
                let previews = window.find_all("image-preview");
                assert_eq!(previews.len(), 2);
                for (preview, pane) in previews.iter().zip([original, modified]) {
                    let bounds = preview.bounds();
                    assert!(bounds.size.width > px(100.) && bounds.size.height > px(100.));
                    assert_eq!(bounds.top(), pane.top() + px(28.));
                    assert_eq!(bounds.bottom(), pane.bottom());
                    assert_eq!(bounds.left(), pane.left());
                    assert_eq!(bounds.right(), pane.right());
                }
            }
            let images = &host.read(cx).images;
            let ImageSide::Ready(original) = &images.original else {
                panic!("missing image")
            };
            assert!(window.has_image_atlas_entry(&original.pixels));
            images.release_textures(window);
            assert!(!window.has_image_atlas_entry(&original.pixels));
        })
        .unwrap();
    }
}
