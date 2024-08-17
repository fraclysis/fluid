pub trait IoError<T> {
    fn exit<P: AsRef<Path>>(self, path: P) -> T;
}

impl<T> IoError<T> for Result<T, io::Error> {
    fn exit<P: AsRef<Path>>(self, path: P) -> T {
        match self {
            Ok(value) => value,
            Err(e) => {
                let os_str = &path.as_ref().as_os_str();
                let path = os_str.to_str().unwrap();

                eprintln!("{path}. {e}");

                process::exit(e.raw_os_error().unwrap_or(1))
            }
        }
    }
}

pub trait PointerDeref<R> {
    fn drf<'a>(self) -> &'a mut R;
}

impl<R> PointerDeref<R> for *mut R {
    fn drf<'a>(self) -> &'a mut R {
        unsafe { &mut *self }
    }
}

use std::{io, path::Path, process};
