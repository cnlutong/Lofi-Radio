use std::{
    fs::{File, OpenOptions},
    io::{self, Read, Write},
    os::windows::{
        fs::OpenOptionsExt,
        io::{AsRawHandle, FromRawHandle, OwnedHandle},
    },
    path::Path,
    thread,
    time::{Duration, Instant},
};
use windows_sys::Win32::{
    Foundation::{ERROR_NO_DATA, INVALID_HANDLE_VALUE},
    Storage::FileSystem::{SECURITY_IDENTIFICATION, SECURITY_SQOS_PRESENT},
    System::{
        Pipes::{GetNamedPipeServerProcessId, PIPE_NOWAIT, SetNamedPipeHandleState},
        Power::{
            PowerClearRequest, PowerCreateRequest, PowerRequestSystemRequired, PowerSetRequest,
        },
        Threading::{POWER_REQUEST_CONTEXT_SIMPLE_STRING, REASON_CONTEXT, REASON_CONTEXT_0},
    },
};

pub struct Pipe(File);

impl Pipe {
    pub fn connect(path: &Path, player_pid: u32) -> io::Result<Self> {
        // mpv 0.41 restricts the server to the current user. Do not allow impersonation.
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .custom_flags(SECURITY_SQOS_PRESENT | SECURITY_IDENTIFICATION)
            .open(path)?;
        let mut server_pid = 0;
        // SAFETY: the file owns a live pipe handle; output points to initialized storage.
        if unsafe { GetNamedPipeServerProcessId(file.as_raw_handle(), &mut server_pid) } == 0 {
            return Err(io::Error::last_os_error());
        }
        if server_pid != player_pid {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "Unexpected IPC server",
            ));
        }
        // Bound reads AND writes so a stalled player cannot lock the UI indefinitely.
        if unsafe {
            SetNamedPipeHandleState(
                file.as_raw_handle(),
                &PIPE_NOWAIT,
                std::ptr::null(),
                std::ptr::null(),
            )
        } == 0
        {
            return Err(io::Error::last_os_error());
        }
        Ok(Self(file))
    }
}

fn retry(mut operation: impl FnMut() -> io::Result<usize>) -> io::Result<usize> {
    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        match operation() {
            Ok(0) => {}
            Err(e) if e.raw_os_error() == Some(ERROR_NO_DATA as i32) => {}
            result => return result,
        }
        if Instant::now() >= deadline {
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "Player IPC timed out",
            ));
        }
        thread::sleep(Duration::from_millis(5));
    }
}

impl Read for Pipe {
    fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
        if bytes.is_empty() {
            return Ok(0);
        }
        retry(|| self.0.read(bytes))
    }
}

impl Write for Pipe {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if bytes.is_empty() {
            return Ok(0);
        }
        retry(|| self.0.write(bytes))
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

pub fn prevent_sleep() -> io::Result<OwnedHandle> {
    let mut reason: Vec<u16> = "Lofi Radio playback\0".encode_utf16().collect();
    let context = REASON_CONTEXT {
        Version: 0,
        Flags: POWER_REQUEST_CONTEXT_SIMPLE_STRING,
        Reason: REASON_CONTEXT_0 {
            SimpleReasonString: reason.as_mut_ptr(),
        },
    };
    // SAFETY: context and UTF-16 reason remain valid for the duration of the call.
    let raw = unsafe { PowerCreateRequest(&context) };
    if raw == INVALID_HANDLE_VALUE || raw.is_null() {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: PowerCreateRequest returned a newly owned handle.
    let handle = unsafe { OwnedHandle::from_raw_handle(raw) };
    if unsafe { PowerSetRequest(handle.as_raw_handle(), PowerRequestSystemRequired) } == 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(handle)
}

pub fn allow_sleep(handle: OwnedHandle) {
    // SAFETY: the owned handle is live; only idle system sleep was requested.
    unsafe {
        PowerClearRequest(handle.as_raw_handle(), PowerRequestSystemRequired);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nonblocking_ipc_retries_and_times_out() {
        let mut attempts = 0;
        assert_eq!(
            retry(|| {
                attempts += 1;
                Ok(if attempts == 1 { 0 } else { 3 })
            })
            .unwrap(),
            3
        );
        let error = retry(|| Err(io::Error::from_raw_os_error(ERROR_NO_DATA as i32))).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::TimedOut);
        assert!(retry(|| Err(io::Error::new(io::ErrorKind::BrokenPipe, "closed"))).is_err());
        let handle = prevent_sleep().unwrap();
        allow_sleep(handle);
    }
}
