use super::{Installer, MAX_INSTALLER, verify_reader};
use crate::i18n::{Locale, LocalizedText};
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

/// Older versions left these directories behind when the running image could
/// not be moved to the system temp directory. Never remove their contents:
/// a failed rollback may have left the only copy of the old executable there.
pub(super) fn clean_empty_staging_dirs(directory: &Path) {
    let Ok(entries) = fs::read_dir(directory) else {
        return;
    };
    for entry in entries.flatten() {
        if entry.file_name().to_str().is_some_and(|name| {
            name.strip_prefix(".githly-update-").is_some_and(|suffix| {
                suffix.len() == 6 && suffix.bytes().all(|byte| byte.is_ascii_alphanumeric())
            })
        }) && entry.file_type().is_ok_and(|kind| kind.is_dir())
        {
            // remove_dir only succeeds for an empty directory.
            let _ = fs::remove_dir(entry.path());
        }
    }
}

pub(super) fn apply(archive: &Path, package: &Installer) -> Result<PathBuf> {
    let exe = std::env::current_exe()?.canonicalize()?;
    let directory = exe
        .parent()
        .with_context(|| LocalizedText::new(Locale::invalid_installer_url))?;
    let mut file = File::open(archive)?;
    verify_reader(&mut file, package)?;
    file.rewind()?;
    let mut zip = zip::ZipArchive::new(file)?;
    let entry = zip
        .by_name("Githly.exe")
        .with_context(|| LocalizedText::new(Locale::invalid_portable_zip))?;
    let size = entry.size();
    ensure!(
        entry.is_file() && (1..=MAX_INSTALLER).contains(&size),
        LocalizedText::new(Locale::invalid_portable_zip)
    );

    // Use sibling files: Windows may keep the old image locked until exit.
    let mut output = tempfile::Builder::new()
        .prefix(".githly-update-")
        .tempfile_in(directory)?;
    ensure!(
        io::copy(&mut entry.take(size + 1), &mut output)? == size,
        LocalizedText::new(Locale::invalid_portable_zip)
    );
    output.as_file().sync_all()?;
    let next = output.into_temp_path();

    let mut old = tempfile::Builder::new()
        .prefix(".githly-previous-")
        .suffix(".exe")
        .tempfile_in(directory)?
        .into_temp_path();
    fs::rename(&exe, &old)?;
    // From here the backup must survive until rollback or deferred deletion.
    old.disable_cleanup(true);
    if let Err(error) = next.persist(&exe) {
        fs::rename(&old, &exe).with_context(|| old.display().to_string())?;
        return Err(error.into());
    }
    // Windows can rename a running image, but only releases it after exit.
    if let Err(error) = self_replace::self_delete_at(&old) {
        tracing::warn!("Portable update cleanup: {error}");
    }
    Ok(exe)
}
