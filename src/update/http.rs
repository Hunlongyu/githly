use super::{Fetch, Response};
use crate::i18n::messages;
use anyhow::{Context, Result, ensure};
use std::{
    ffi::c_void,
    io::Write,
    ptr::{null, null_mut},
    time::{Duration, Instant},
};
use windows_sys::Win32::Networking::WinHttp::*;

pub struct Http;

struct Handle(*mut c_void);
impl Handle {
    fn new(value: *mut c_void) -> Result<Self> {
        ensure!(
            !value.is_null(),
            messages().connection_failed(std::io::Error::last_os_error())
        );
        Ok(Self(value))
    }
}
impl Drop for Handle {
    fn drop(&mut self) {
        // SAFETY: each non-null handle has exactly one owner, on this worker.
        unsafe {
            WinHttpCloseHandle(self.0);
        }
    }
}
fn wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(Some(0)).collect()
}
fn succeeded(value: i32) -> Result<()> {
    ensure!(
        value != 0,
        messages().request_failed(std::io::Error::last_os_error())
    );
    Ok(())
}
fn header(handle: &Handle, name: &str) -> Option<u64> {
    let name = wide(name);
    let mut buffer = [0u16; 128];
    let mut size = std::mem::size_of_val(&buffer) as u32;
    // SAFETY: valid request, terminated header name and correctly sized buffer.
    let result = unsafe {
        WinHttpQueryHeaders(
            handle.0,
            WINHTTP_QUERY_CUSTOM,
            name.as_ptr(),
            buffer.as_mut_ptr().cast(),
            &mut size,
            null_mut(),
        )
    };
    if result == 0 {
        return None;
    }
    String::from_utf16_lossy(&buffer[..(size as usize / 2).min(buffer.len())])
        .trim_matches('\0')
        .trim()
        .parse()
        .ok()
}

impl Fetch for Http {
    fn get(&mut self, address: &str, output: &mut dyn Write, limit: u64) -> Result<Response> {
        let url = url::Url::parse(address)?;
        ensure!(
            url.scheme() == "https" && url.username().is_empty() && url.password().is_none(),
            messages().https_required()
        );
        let host = wide(url.host_str().context(messages().missing_host())?);
        let path = wide(&format!(
            "{}{}",
            url.path(),
            url.query().map(|q| format!("?{q}")).unwrap_or_default()
        ));
        let agent = wide(concat!("Githly/", env!("CARGO_PKG_VERSION")));
        let verb = wide("GET");
        let headers = wide(if address == super::API {
            "Accept: application/vnd.github+json\r\n"
        } else {
            "Accept: application/octet-stream\r\n"
        });
        let start = Instant::now();
        // SAFETY: all strings remain alive and NUL terminated through synchronous
        // calls. Handles close in request/connection/session order on every exit.
        unsafe {
            let session = Handle::new(WinHttpOpen(
                agent.as_ptr(),
                WINHTTP_ACCESS_TYPE_AUTOMATIC_PROXY,
                null(),
                null(),
                0,
            ))?;
            succeeded(WinHttpSetTimeouts(
                session.0, 10_000, 15_000, 15_000, 20_000,
            ))?;
            let connection = Handle::new(WinHttpConnect(
                session.0,
                host.as_ptr(),
                url.port_or_known_default().unwrap_or(443),
                0,
            ))?;
            let request = Handle::new(WinHttpOpenRequest(
                connection.0,
                verb.as_ptr(),
                path.as_ptr(),
                null(),
                null(),
                null(),
                WINHTTP_FLAG_SECURE,
            ))?;
            let policy = WINHTTP_OPTION_REDIRECT_POLICY_DISALLOW_HTTPS_TO_HTTP;
            succeeded(WinHttpSetOption(
                request.0,
                WINHTTP_OPTION_REDIRECT_POLICY,
                (&policy as *const u32).cast(),
                4,
            ))?;
            succeeded(WinHttpSendRequest(
                request.0,
                headers.as_ptr(),
                headers.len() as u32 - 1,
                null(),
                0,
                0,
                0,
            ))?;
            succeeded(WinHttpReceiveResponse(request.0, null_mut()))?;
            let mut status = 0u32;
            let mut size = 4;
            succeeded(WinHttpQueryHeaders(
                request.0,
                WINHTTP_QUERY_STATUS_CODE | WINHTTP_QUERY_FLAG_NUMBER,
                null(),
                (&mut status as *mut u32).cast(),
                &mut size,
                null_mut(),
            ))?;
            let response = Response {
                status,
                retry_after: header(&request, "Retry-After"),
                rate_reset: header(&request, "X-RateLimit-Reset"),
            };
            if status != 200 {
                return Ok(response);
            }
            if let Some(length) = header(&request, "Content-Length") {
                ensure!(length <= limit, messages().update_size_limit());
            }
            let mut buffer = [0u8; 64 * 1024];
            let mut total = 0u64;
            let budget = Duration::from_secs(if limit <= 1024 * 1024 { 45 } else { 600 });
            loop {
                ensure!(start.elapsed() < budget, messages().request_timeout());
                let mut read = 0;
                succeeded(WinHttpReadData(
                    request.0,
                    buffer.as_mut_ptr().cast(),
                    buffer.len() as u32,
                    &mut read,
                ))?;
                if read == 0 {
                    break;
                }
                total += u64::from(read);
                ensure!(total <= limit, messages().update_size_limit());
                output.write_all(&buffer[..read as usize])?;
            }
            Ok(response)
        }
    }
}
