use crate::{app::GitPanel, i18n::messages};
use anyhow::{Context, Result, ensure};
use async_io::{Async, Timer};
use futures_lite::{
    future,
    io::{AsyncReadExt, AsyncWriteExt},
};
use gpui_kit::{App, WeakEntity, Window};
use std::{
    fs::{self, File, TryLockError},
    io::{self, Read, Write},
    net::Shutdown,
    os::unix::{
        ffi::{OsStrExt, OsStringExt},
        fs::{DirBuilderExt, MetadataExt},
        net::{UnixListener, UnixStream},
    },
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

const LIMIT: usize = 32 * 1024;
const TIMEOUT: Duration = Duration::from_secs(5);

pub struct Instance {
    listener: Async<UnixListener>,
    socket: PathBuf,
    _lock: File,
}

impl Drop for Instance {
    fn drop(&mut self) {
        // Still holding the lock: the next owner cannot bind this path yet.
        let _ = fs::remove_file(&self.socket);
    }
}

pub fn claim_or_forward(project: Option<&Path>) -> Result<Option<Instance>> {
    // SAFETY: geteuid has no arguments or memory preconditions.
    let uid = unsafe { libc::geteuid() };
    let dir = directories::BaseDirs::new()
        .and_then(|dirs| dirs.runtime_dir().map(Path::to_owned))
        .unwrap_or_else(std::env::temp_dir)
        .join(format!("githly-{uid}"));
    match fs::DirBuilder::new().mode(0o700).create(&dir) {
        Err(error) if error.kind() != io::ErrorKind::AlreadyExists => return Err(error.into()),
        _ => {}
    }
    let meta = fs::symlink_metadata(&dir)?;
    ensure!(
        meta.is_dir() && meta.uid() == uid && meta.mode() & 0o077 == 0,
        "Invalid private IPC directory: {}",
        dir.display()
    );
    let lock = File::options()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(dir.join("instance.lock"))?;
    let socket = dir.join("window.sock");
    let deadline = Instant::now() + TIMEOUT;
    loop {
        match lock.try_lock() {
            Ok(()) => {
                if let Err(error) = fs::remove_file(&socket)
                    && error.kind() != io::ErrorKind::NotFound
                {
                    return Err(error.into());
                }
                let listener = Async::new(UnixListener::bind(&socket)?)?;
                return Ok(Some(Instance {
                    listener,
                    socket,
                    _lock: lock,
                }));
            }
            Err(TryLockError::Error(error)) => return Err(error.into()),
            Err(TryLockError::WouldBlock) => {}
        }
        match UnixStream::connect(&socket) {
            Ok(mut stream) => {
                stream.set_read_timeout(Some(TIMEOUT))?;
                stream.set_write_timeout(Some(TIMEOUT))?;
                if let Some(path) = project {
                    let path = std::path::absolute(path)?;
                    let bytes = path.as_os_str().as_bytes();
                    ensure!(bytes.len() <= LIMIT, messages().path_too_long());
                    stream.write_all(bytes)?;
                }
                stream.shutdown(Shutdown::Write)?;
                let mut ack = [0];
                stream
                    .read_exact(&mut ack)
                    .context(messages().instance_unresponsive())?;
                ensure!(ack == [1], messages().launch_rejected());
                return Ok(None);
            }
            Err(error)
                if matches!(
                    error.kind(),
                    io::ErrorKind::NotFound | io::ErrorKind::ConnectionRefused
                ) && Instant::now() < deadline =>
            {
                std::thread::sleep(Duration::from_millis(40))
            }
            Err(error) => return Err(error).context(messages().instance_not_ready()),
        }
    }
}

impl Instance {
    pub fn listen(self, window: &Window, panel: WeakEntity<GitPanel>, cx: &mut App) {
        let window = window.window_handle();
        cx.spawn(async move |cx| {
            while let Ok((mut stream, _)) = self.listener.accept().await {
                let mut bytes = Vec::new();
                let mut read = (&mut stream).take(LIMIT as u64 + 1);
                let Ok(count) = future::race(async { read.read_to_end(&mut bytes).await }, async {
                    Timer::after(TIMEOUT).await;
                    Err(io::ErrorKind::TimedOut.into())
                })
                .await
                else {
                    continue;
                };
                if count > LIMIT || bytes.contains(&0) {
                    continue;
                }
                let path = PathBuf::from(std::ffi::OsString::from_vec(bytes));
                if count != 0 && !path.is_absolute() {
                    continue;
                }
                let accepted = window
                    .update(cx, |_, window, cx| {
                        window.activate_window();
                        if count > 0 {
                            return panel
                                .update(cx, |panel, cx| panel.load_project(path, cx))
                                .is_ok();
                        }
                        true
                    })
                    .unwrap_or(false);
                let _ = stream.write_all(&[u8::from(accepted)]).await;
            }
            // Keep the lock and socket owned by this task for its entire lifetime.
            drop(self);
        })
        .detach();
    }
}
