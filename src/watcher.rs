use std::{
    cell::UnsafeCell,
    io::{Error, ErrorKind},
    mem::forget,
    ptr::{null, null_mut},
    time::{Duration, Instant},
};

use windows_sys::Win32::{
    Foundation::{
        CloseHandle, BOOL, FALSE, HANDLE, INVALID_HANDLE_VALUE, TRUE, WAIT_ABANDONED_0,
        WAIT_FAILED, WAIT_OBJECT_0, WAIT_TIMEOUT,
    },
    Storage::FileSystem::{
        FindCloseChangeNotification, FindFirstChangeNotificationW, FindNextChangeNotification,
        FILE_NOTIFY_CHANGE_DIR_NAME, FILE_NOTIFY_CHANGE_FILE_NAME, FILE_NOTIFY_CHANGE_LAST_WRITE,
    },
    System::{
        Console::{SetConsoleCtrlHandler, CTRL_C_EVENT},
        Environment::GetCurrentDirectoryW,
        Threading::{CreateSemaphoreW, ReleaseSemaphore, WaitForMultipleObjects, INFINITE},
    },
};

use crate::MutRef;

static mut EXIT_SEMAPHORE: HANDLE = 0;

const WATCHER_PATHS_COUNT: usize = 4;

pub const LAYOUTS_FOLDER_STATUS_ID: u32 = 1;
const WATCHER_PATHS: [&str; WATCHER_PATHS_COUNT] =
    ["\\layouts\0", "\\site\0", "\\includes\0", "\\assets\0"];

pub struct Watcher {
    semaphore: HANDLE,
    handles: [HANDLE; WATCHER_PATHS_COUNT],
}

impl Watcher {
    /// Alerts `Watcher::watch` through `EXIT_SEMAPHORE`
    unsafe extern "system" fn console_handler(ctrl_type: u32) -> BOOL {
        if ctrl_type == CTRL_C_EVENT {
            if EXIT_SEMAPHORE != 0 {
                if ReleaseSemaphore(EXIT_SEMAPHORE, 1, null_mut()) == 0 {
                    dbg!(Error::last_os_error());
                    FALSE
                } else {
                    TRUE
                }
            } else {
                FALSE
            }
        } else {
            FALSE
        }
    }

    pub fn new() -> Result<Self, Error> {
        unsafe { Self::_new() }
    }

    unsafe fn _new() -> Result<Self, Error> {
        let semaphore = CreateSemaphoreW(null(), 0, 1, null());
        if semaphore == 0 {
            return Err(Error::last_os_error());
        }

        let defer_semaphore = Defer(|| {
            if CloseHandle(semaphore) == 0 {
                dbg!(Error::last_os_error());
            }
        });

        if SetConsoleCtrlHandler(Some(Self::console_handler), TRUE) == 0 {
            return Err(Error::last_os_error());
        }

        const MAX_BUFFER_SIZE: usize = 512;

        let mut buffer: Vec<u16> = Vec::with_capacity(MAX_BUFFER_SIZE);

        let mut buffer_primer_size =
            GetCurrentDirectoryW(MAX_BUFFER_SIZE as u32, buffer.as_mut_ptr()) as usize;
        if buffer_primer_size == 0 {
            return Err(Error::last_os_error());
        }

        if !(MAX_BUFFER_SIZE > buffer_primer_size) {
            buffer.reserve(buffer_primer_size);
            buffer_primer_size =
                GetCurrentDirectoryW(MAX_BUFFER_SIZE as u32, buffer.as_mut_ptr()) as usize;
            if buffer_primer_size == 0 {
                return Err(Error::last_os_error());
            }
        }

        buffer.set_len(buffer_primer_size);

        let handles = UnsafeCell::new([INVALID_HANDLE_VALUE; WATCHER_PATHS_COUNT]);

        let defer_handles = Defer(|| {
            for h in handles.mut_ref() {
                if *h != INVALID_HANDLE_VALUE {
                    if FindCloseChangeNotification(*h) == 0 {
                        dbg!(Error::last_os_error());
                    }
                }
            }
        });

        for (i, small_path) in WATCHER_PATHS.iter().enumerate() {
            buffer.extend(small_path.encode_utf16());

            let change = FindFirstChangeNotificationW(
                buffer.as_ptr(),
                TRUE,
                FILE_NOTIFY_CHANGE_DIR_NAME
                    | FILE_NOTIFY_CHANGE_FILE_NAME
                    | FILE_NOTIFY_CHANGE_LAST_WRITE,
            );
            if (change == INVALID_HANDLE_VALUE) | (change == 0) {
                return Err(Error::last_os_error());
            }

            handles.mut_ref()[i] = change;

            buffer.set_len(buffer_primer_size);
        }

        forget(defer_handles);
        forget(defer_semaphore);

        EXIT_SEMAPHORE = semaphore;

        Ok(Self { semaphore, handles: handles.into_inner() })
    }

    pub fn watch(&self, proc: impl FnMut(u32)) -> Result<(), Error> {
        unsafe { self._watch(proc) }
    }

    unsafe fn _watch(&self, mut proc: impl FnMut(u32)) -> Result<(), Error> {
        let mut update_list = [0; WATCHER_PATHS_COUNT + 1];
        update_list[0] = self.semaphore;
        for i in 0..WATCHER_PATHS_COUNT {
            update_list[i + 1] = self.handles[i];
        }

        let mut last_time = Instant::now();
        let update_latency = Duration::from_millis(300);

        loop {
            let status = WaitForMultipleObjects(
                update_list.len() as u32,
                update_list.as_ptr(),
                FALSE,
                INFINITE,
            );

            if status >= WAIT_OBJECT_0 + 1 && status < WAIT_OBJECT_0 + update_list.len() as u32 {
                let now = Instant::now();

                if now.duration_since(last_time) > update_latency {
                    proc(status);
                }

                if FindNextChangeNotification(update_list[status as usize]) == 0 {
                    return Err(Error::last_os_error());
                }

                last_time = now;

                continue;
            }

            if status == WAIT_OBJECT_0 {
                return Ok(());
            }

            if status >= WAIT_ABANDONED_0 && status < WAIT_ABANDONED_0 + update_list.len() as u32 {
                return Err(Error::new(
                    ErrorKind::Other,
                    format!(
                        "Wait abandoned. With status {} and handle index {}.",
                        status,
                        status - WAIT_ABANDONED_0
                    ),
                ));
            }

            if status == WAIT_TIMEOUT {
                return Err(Error::new(ErrorKind::Other, "Wait time out"));
            }

            if status == WAIT_FAILED {
                return Err(Error::last_os_error());
            }

            return Err(dbg!(Error::new(ErrorKind::Other, "Unknown error")));
        }
    }
}

impl Drop for Watcher {
    fn drop(&mut self) {
        unsafe { EXIT_SEMAPHORE = 0 };

        if unsafe { CloseHandle(self.semaphore) } == 0 {
            dbg!(Error::last_os_error());
        }

        for handle in self.handles {
            if unsafe { FindCloseChangeNotification(handle) } == 0 {
                dbg!(Error::last_os_error());
            }
        }
    }
}

pub struct Defer<F: FnMut()>(pub F);

impl<F: FnMut()> Drop for Defer<F> {
    fn drop(&mut self) {
        (self.0)()
    }
}
