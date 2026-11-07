use anyhow::{Context, Result, ensure};
use std::{
    ffi::CString,
    fs, io,
    os::unix::ffi::OsStrExt,
    path::{Path, PathBuf},
    process::Command,
};

fn run(command: &mut Command) -> Result<String> {
    let output = command.output()?;
    ensure!(
        output.status.success(),
        "{:?} ({}): {}",
        command.get_program(),
        output.status,
        String::from_utf8_lossy(&output.stderr).trim()
    );
    Ok(String::from_utf8(output.stdout)?.trim().to_owned())
}

fn plist_value(bundle: &Path, key: &str) -> Result<String> {
    run(Command::new("/usr/bin/plutil")
        .args(["-extract", key, "raw", "-o", "-"])
        .arg(bundle.join("Contents/Info.plist")))
}

fn stage_installation() -> Result<(PathBuf, tempfile::TempDir)> {
    let exe = std::env::current_exe()?;
    if let Some(bundle) = exe.ancestors().nth(3)
        && bundle.extension().is_some_and(|ext| ext == "app")
        && exe.ends_with("Contents/MacOS/githly")
        && !bundle
            .components()
            .any(|c| c.as_os_str() == "AppTranslocation")
    {
        let parent = bundle.parent().context("Invalid app location")?;
        match tempfile::tempdir_in(parent) {
            Ok(stage) => return Ok((bundle.to_owned(), stage)),
            Err(error) if error.kind() == io::ErrorKind::ReadOnlyFilesystem => {}
            Err(error) => return Err(error).with_context(|| parent.display().to_string()),
        }
    }
    let system = PathBuf::from("/Applications/Githly.app");
    let target = if system.try_exists()? {
        system
    } else {
        let apps = directories::BaseDirs::new()
            .context("Home directory unavailable")?
            .home_dir()
            .join("Applications");
        fs::create_dir_all(&apps)?;
        apps.join("Githly.app")
    };
    let stage = tempfile::tempdir_in(target.parent().context("Invalid app location")?)
        .with_context(|| target.display().to_string())?;
    Ok((target, stage))
}

fn install_bundle(source: &Path, target: &Path, stage: &Path, version: &str) -> Result<()> {
    let next = stage.join("Githly.app");
    run(Command::new("/usr/bin/ditto").arg(source).arg(&next))?;
    run(Command::new("/usr/bin/codesign")
        .args(["--verify", "--strict"])
        .arg(&next))?;
    let id = "com.hunlongyu.githly";
    ensure!(
        plist_value(&next, "CFBundleIdentifier")? == id
            && plist_value(&next, "CFBundleShortVersionString")? == version,
        "Unexpected app bundle"
    );
    let flag = if target.try_exists()? {
        ensure!(
            plist_value(target, "CFBundleIdentifier")? == id,
            "Another app occupies {}",
            target.display()
        );
        libc::RENAME_SWAP
    } else {
        libc::RENAME_EXCL
    };
    // Atomically exchange whole bundles on the destination volume. The
    // running bundle stays in place on failure; staging owns the old one on success.
    let from = CString::new(next.as_os_str().as_bytes())?;
    let to = CString::new(target.as_os_str().as_bytes())?;
    if unsafe { libc::renamex_np(from.as_ptr(), to.as_ptr(), flag) } != 0 {
        return Err(io::Error::last_os_error()).with_context(|| target.display().to_string());
    }
    Ok(())
}

pub(super) fn install(dmg: &Path, version: &str) -> Result<PathBuf> {
    let (target, stage) = stage_installation()?;
    let mount = tempfile::tempdir()?;
    run(Command::new("/usr/bin/hdiutil")
        .args(["attach", "-readonly", "-nobrowse", "-mountpoint"])
        .arg(mount.path())
        .arg(dmg))?;
    let result = install_bundle(
        &mount.path().join("Githly.app"),
        &target,
        stage.path(),
        version,
    );
    if let Err(error) = run(Command::new("/usr/bin/hdiutil")
        .arg("detach")
        .arg(mount.path()))
    {
        tracing::warn!("Could not detach {}: {error:#}", mount.keep().display());
    }
    result?;
    Ok(target)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    #[test]
    fn bundle_swap_requires_the_expected_signed_version() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("Githly.app");
        let contents = source.join("Contents");
        fs::create_dir_all(contents.join("MacOS")).unwrap();
        let binary = contents.join("MacOS/githly");
        fs::copy("/usr/bin/true", &binary).unwrap();
        fs::set_permissions(&binary, fs::Permissions::from_mode(0o755)).unwrap();
        fs::write(
            contents.join("Info.plist"),
            include_str!("../../assets/macos/Info.plist").replace("@VERSION@", "2.0.0"),
        )
        .unwrap();
        run(Command::new("/usr/bin/codesign")
            .args(["--force", "--sign", "-"])
            .arg(&source))
        .unwrap();

        let target = dir.path().join("Installed Githly.app");
        let old = target.join("Contents/Info.plist");
        fs::create_dir_all(old.parent().unwrap()).unwrap();
        fs::write(
            &old,
            include_str!("../../assets/macos/Info.plist").replace("@VERSION@", "1.0.0"),
        )
        .unwrap();
        let stage = tempfile::tempdir_in(dir.path()).unwrap();
        assert!(install_bundle(&source, &target, stage.path(), "3.0.0").is_err());
        assert_eq!(plist_value(&target, "CFBundleVersion").unwrap(), "1.0.0");
        let stage = tempfile::tempdir_in(dir.path()).unwrap();
        install_bundle(&source, &target, stage.path(), "2.0.0").unwrap();
        assert_eq!(plist_value(&target, "CFBundleVersion").unwrap(), "2.0.0");
        assert_eq!(
            plist_value(&stage.path().join("Githly.app"), "CFBundleVersion").unwrap(),
            "1.0.0"
        );
    }
}
