use gpui_kit::{
    AssetSource, Image, ImageFormat, IntoElement, Pixels, SharedString, Styled, assets, img,
};
use std::{
    borrow::Cow,
    sync::{Arc, LazyLock},
};

pub(crate) fn logo(size: Pixels) -> impl IntoElement {
    static LOGO: LazyLock<Arc<Image>> = LazyLock::new(|| {
        Arc::new(Image::from_bytes(
            ImageFormat::Png,
            include_bytes!("../assets/branding/githly-256.png").to_vec(),
        ))
    });
    img(LOGO.clone()).size(size).flex_shrink_0()
}

assets::icon_assets!(ExtraIcons, [CloudDownload, Archive, ListX, MessageSquare]);

pub(crate) struct AppAssets;

impl AssetSource for AppAssets {
    fn load(&self, path: &str) -> anyhow::Result<Option<Cow<'static, [u8]>>> {
        match ExtraIcons.load(path)? {
            Some(bytes) => Ok(Some(bytes)),
            None => assets::Assets.load(path),
        }
    }

    fn list(&self, path: &str) -> anyhow::Result<Vec<SharedString>> {
        let mut paths = assets::Assets.list(path)?;
        paths.extend(ExtraIcons.list(path)?);
        paths.sort();
        paths.dedup();
        Ok(paths)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use assets::IconName;

    #[test]
    fn application_source_loads_extra_and_default_icons() {
        let paths = AppAssets.list("icons").unwrap();
        for icon in [
            IconName::CloudDownload,
            IconName::Archive,
            IconName::ListX,
            IconName::MessageSquare,
            IconName::Settings,
        ] {
            let path = icon.path();
            assert!(paths.contains(&path));
            let svg = AppAssets.load(&path).unwrap().unwrap();
            assert!(std::str::from_utf8(&svg).unwrap().contains("<svg"));
        }
    }
}
