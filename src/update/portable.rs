use super::{Installer, MAX_INSTALLER, verify_reader};
use crate::i18n::messages;
use anyhow::{Context, Result, ensure};
use std::{
    fs::{self, File},
    io::{self, Read, Seek},
    path::{Path, PathBuf},
};
use winreg::{
    RegKey,
    enums::{HKEY_CURRENT_USER, KEY_READ, KEY_WOW64_64KEY},
};

pub(super) fn is_installed(exe: &Path) -> Result<bool> {
    // Inno Setup records this for existing installations, including custom /DIR paths.
    const KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Uninstall\{EA4B331C-38DE-4F1B-97DE-3BB671F82434}_is1";
    let key = match RegKey::predef(HKEY_CURRENT_USER)
        .open_subkey_with_flags(KEY, KEY_READ | KEY_WOW64_64KEY)
    {
        Ok(key) => key,
        Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(false),
        Err(err) => return Err(err.into()),
    };
    let location: String = key.get_value("InstallLocation")?;
    let installed = PathBuf::from(location).join("Githly.exe");
    match installed.canonicalize() {
        Ok(path) => Ok(path == exe.canonicalize()?),
        Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(err) => Err(err.into()),
    }
}

pub(super) fn apply(archive: &Path, package: &Installer) -> Result<()> {
    let exe = std::env::current_exe()?.canonicalize()?;
    let directory = exe.parent().context(messages().invalid_installer_url())?;
    let mut file = File::open(archive)?;
    verify_reader(&mut file, package)?;
    file.rewind()?;
    let mut zip = zip::ZipArchive::new(file)?;
    let entry = zip
        .by_name("Githly.exe")
        .context(messages().invalid_portable_zip())?;
    let size = entry.size();
    ensure!(
        entry.is_file() && (1..=MAX_INSTALLER).contains(&size),
        messages().invalid_portable_zip()
    );

    // Stage on the destination volume before touching the running binary.
    let stage = tempfile::Builder::new()
        .prefix(".githly-update-")
        .tempdir_in(directory)?;
    let next = stage.path().join("new.exe");
    let mut output = File::create(&next)?;
    ensure!(
        io::copy(&mut entry.take(size + 1), &mut output)? == size,
        messages().invalid_portable_zip()
    );
    output.sync_all()?;
    drop(output);

    let old = stage.path().join("previous.exe");
    fs::rename(&exe, &old)?;
    if let Err(error) = fs::rename(&next, &exe) {
        if let Err(rollback) = fs::rename(&old, &exe) {
            let recovery = stage.keep();
            return Err(error).context(format!(
                "{}: {rollback}",
                recovery.join("previous.exe").display()
            ));
        }
        return Err(error.into());
    }
    // Windows can rename a running image, but only releases it after exit.
    if let Err(error) = self_replace::self_delete_at(&old) {
        tracing::warn!("Portable update cleanup: {error}");
    }
    Ok(())
}
