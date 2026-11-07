use super::{Installer, verify_reader};
use crate::i18n::messages;
use anyhow::{Context, Result, ensure};
use std::{
    fs::{self, File},
    io::{self, Read, Seek},
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
};

pub(super) fn current_path() -> Result<Option<PathBuf>> {
    let Some(dir) = std::env::var_os("APPDIR") else {
        return Ok(None);
    };
    // A terminal packaged as an AppImage can pass its environment to native Githly.
    let bundled = Path::new(&dir).join("usr/bin/githly");
    if bundled.canonicalize().ok().as_ref() != Some(&std::env::current_exe()?) {
        return Ok(None);
    }
    let image = std::env::var_os("APPIMAGE").context(messages().invalid_installer_url())?;
    Ok(Some(Path::new(&image).canonicalize()?))
}

pub(super) fn replace(image: &Path, download: &Path, installer: &Installer) -> Result<()> {
    let directory = image.parent().context(messages().invalid_installer_url())?;
    let mode = fs::metadata(image)?.permissions().mode() & 0o777;
    let mut staged = tempfile::NamedTempFile::new_in(directory)?;
    let file = staged.as_file_mut();
    let bytes = io::copy(&mut File::open(download)?.take(installer.size + 1), file)?;
    ensure!(bytes == installer.size, messages().installer_incomplete());
    file.rewind()?;
    verify_reader(file, installer)?;
    file.set_permissions(fs::Permissions::from_mode(mode))?;
    file.sync_all()?;
    staged.persist(image)?;
    Ok(())
}
