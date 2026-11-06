use gpui_kit::component::diff::DiffFile;

pub(super) enum Content {
    Text(Vec<DiffFile>),
    Image(super::image_diff::ImageDiff),
}

pub(super) fn parse(patch: &str) -> anyhow::Result<Vec<DiffFile>> {
    // Diff::syntax_highlight(false) only changes painting. Override the
    // language as well so presentation preparation skips grammar parsing,
    // including Qt translation .ts files otherwise detected as TypeScript.
    // Line backgrounds, change signs and inline word emphasis are independent.
    Ok(DiffFile::parse(patch)?
        .into_iter()
        .map(|file| file.with_language("text"))
        .collect())
}
