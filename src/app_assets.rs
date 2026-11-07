use gpui_kit::{AssetSource, SharedString, assets};
use std::borrow::Cow;

assets::icon_assets!(ExtraIcons, [CloudDownload, Archive]);

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
    fn application_source_loads_update_stash_and_default_icons() {
        let paths = AppAssets.list("icons").unwrap();
        for icon in [
            IconName::CloudDownload,
            IconName::Archive,
            IconName::Settings,
        ] {
            let path = icon.path();
            assert!(paths.contains(&path));
            let svg = AppAssets.load(&path).unwrap().unwrap();
            assert!(std::str::from_utf8(&svg).unwrap().contains("<svg"));
        }
    }
}
