use crate::app::GitPanel;
use anyhow::{Context as _, Result, bail, ensure};
use async_channel::Sender;
use gpui_kit::{App, WeakEntity, Window};
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use std::{
    ffi::OsString,
    io,
    os::windows::{
        ffi::{OsStrExt, OsStringExt},
        io::{FromRawHandle, OwnedHandle},
    },
    path::{Path, PathBuf},
    ptr,
    time::{Duration, Instant},
};
use windows_sys::{
    Win32::{
        Foundation::{ERROR_ALREADY_EXISTS, GetLastError, HWND, LPARAM, LRESULT, WPARAM},
        System::{DataExchange::COPYDATASTRUCT, Threading::CreateMutexW},
        UI::{
            Shell::{DefSubclassProc, RemoveWindowSubclass, SetWindowSubclass},
            WindowsAndMessaging::*,
        },
    },
    core::{PCWSTR, w},
};

const MUTEX: PCWSTR = w!("Local\\Hunlongyu.Githly.Instance");
const WINDOW_MARKER: PCWSTR = w!("Hunlongyu.Githly.LaunchWindow");
const LAUNCH: usize = 0x47495448;
const MAX_PATH_UNITS: usize = 32767;

pub fn claim_or_forward(project: Option<&Path>) -> Result<Option<OwnedHandle>> {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        let raw = unsafe { CreateMutexW(ptr::null(), 0, MUTEX) };
        if raw.is_null() {
            return Err(io::Error::last_os_error()).context("无法创建 Githly 单实例锁");
        }
        let exists = unsafe { GetLastError() } == ERROR_ALREADY_EXISTS;
        let guard = unsafe { OwnedHandle::from_raw_handle(raw) };
        if !exists {
            return Ok(Some(guard));
        }
        // Do not keep an exiting instance's mutex alive while looking for its window.
        drop(guard);
        let mut hwnd: HWND = ptr::null_mut();
        unsafe { EnumWindows(Some(find_window), (&mut hwnd as *mut HWND) as LPARAM) };
        if !hwnd.is_null() {
            forward(hwnd, project)?;
            return Ok(None);
        }
        ensure!(
            Instant::now() < deadline,
            "已有 Githly 进程，但主窗口尚未就绪"
        );
        std::thread::sleep(Duration::from_millis(50));
    }
}

unsafe extern "system" fn find_window(hwnd: HWND, result: LPARAM) -> i32 {
    if unsafe { GetPropW(hwnd, WINDOW_MARKER) } == hwnd {
        unsafe { *(result as *mut HWND) = hwnd };
        return 0;
    }
    1
}

fn forward(hwnd: HWND, project: Option<&Path>) -> Result<()> {
    let path = project.map(std::path::absolute).transpose()?;
    let units: Vec<u16> = path
        .as_deref()
        .unwrap_or(Path::new(""))
        .as_os_str()
        .encode_wide()
        .collect();
    ensure!(units.len() <= MAX_PATH_UNITS, "仓库路径过长");
    let packet = COPYDATASTRUCT {
        dwData: LAUNCH,
        cbData: (units.len() * 2) as u32,
        lpData: units.as_ptr().cast_mut().cast(),
    };
    let mut accepted = 0;
    unsafe {
        let mut pid = 0;
        GetWindowThreadProcessId(hwnd, &mut pid);
        AllowSetForegroundWindow(pid);
        if SendMessageTimeoutW(
            hwnd,
            WM_COPYDATA,
            0,
            (&packet as *const _) as LPARAM,
            SMTO_ABORTIFHUNG | SMTO_ERRORONEXIT,
            5000,
            &mut accepted,
        ) == 0
        {
            return Err(io::Error::last_os_error()).context("已有 Githly 窗口未响应启动请求");
        }
    }
    ensure!(accepted == 1, "已有 Githly 窗口未接受启动请求");
    Ok(())
}

pub fn listen(window: &Window, panel: WeakEntity<GitPanel>, cx: &mut App) -> Result<()> {
    let native = HasWindowHandle::window_handle(window)
        .ok()
        .context("无法取得主窗口句柄")?;
    let RawWindowHandle::Win32(handle) = native.as_raw() else {
        bail!("缺少 Win32 窗口句柄");
    };
    let hwnd = handle.hwnd.get() as HWND;
    let (tx, rx) = async_channel::bounded::<Option<PathBuf>>(32);
    let sender = Box::into_raw(Box::new(tx));
    // The window owns this reference until WM_NCDESTROY. Publish the marker only
    // after its handler can acknowledge requests.
    unsafe {
        if SetWindowSubclass(hwnd, Some(receive), LAUNCH, sender as usize) == 0 {
            drop(Box::from_raw(sender));
            bail!("无法接收重复启动请求");
        }
        if SetPropW(hwnd, WINDOW_MARKER, hwnd) == 0 {
            let error = io::Error::last_os_error();
            if RemoveWindowSubclass(hwnd, Some(receive), LAUNCH) != 0 {
                drop(Box::from_raw(sender));
            }
            return Err(error).context("无法标记主窗口");
        }
    }
    let window = window.window_handle();
    cx.spawn(async move |cx| {
        while let Ok(project) = rx.recv().await {
            if window
                .update(cx, |_, window, cx| {
                    window.activate_window();
                    if let Some(path) = project {
                        let _ = panel.update(cx, |panel, cx| panel.load_project(path, cx));
                    }
                })
                .is_err()
            {
                break;
            }
        }
    })
    .detach();
    Ok(())
}

unsafe extern "system" fn receive(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    id: usize,
    data: usize,
) -> LRESULT {
    let sender = data as *mut Sender<Option<PathBuf>>;
    if msg == WM_NCDESTROY {
        unsafe {
            RemovePropW(hwnd, WINDOW_MARKER);
            RemoveWindowSubclass(hwnd, Some(receive), id);
            drop(Box::from_raw(sender));
        }
    } else if msg == WM_COPYDATA && lparam != 0 {
        let packet = unsafe { &*(lparam as *const COPYDATASTRUCT) };
        if packet.dwData == LAUNCH {
            let size = packet.cbData as usize;
            let path = if size == 0 {
                None
            } else {
                if !size.is_multiple_of(2) || size > MAX_PATH_UNITS * 2 || packet.lpData.is_null() {
                    return 0;
                }
                // WM_COPYDATA owns this buffer only for the duration of the call.
                let bytes = unsafe { std::slice::from_raw_parts(packet.lpData.cast::<u8>(), size) };
                let (pairs, _) = bytes.as_chunks::<2>();
                let units: Vec<_> = pairs.iter().copied().map(u16::from_le_bytes).collect();
                let path = PathBuf::from(OsString::from_wide(&units));
                if !path.is_absolute() || units.contains(&0) {
                    return 0;
                }
                Some(path)
            };
            return unsafe { &*sender }.try_send(path).is_ok() as LRESULT;
        }
    }
    unsafe { DefSubclassProc(hwnd, msg, wparam, lparam) }
}
