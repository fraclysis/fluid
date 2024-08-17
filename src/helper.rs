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

                eprintln!("Operation failed on {path}. {e}");

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

pub trait MutRef<T> {
    fn mut_ref(&self) -> &mut T;
}

impl<T> MutRef<T> for UnsafeCell<T> {
    fn mut_ref(&self) -> &mut T {
        unsafe { &mut *self.get() }
    }
}

pub trait Warn {
    fn warn(self);
}

impl<T, E: std::error::Error> Warn for Result<T, E> {
    fn warn(self) {
        if let Err(e) = self {
            eprintln!("{e}")
        }
    }
}

pub trait IntoIoResult<T> {
    fn io_result(self) -> Result<T, io::Error>;
}

impl<T> IntoIoResult<T> for Option<T> {
    fn io_result(self) -> Result<T, io::Error> {
        match self {
            Some(s) => Ok(s),
            None => Err(std::io::ErrorKind::NotFound.into()),
        }
    }
}

#[derive(Default, Clone)]
pub struct Ru<T>(pub Rc<UnsafeCell<T>>);

impl<T> Ru<T> {
    pub fn new(t: T) -> Self {
        Self(Rc::new(UnsafeCell::new(t)))
    }

    pub fn downgrade(&self) -> Wu<T> {
        Wu(Rc::downgrade(&self.0))
    }
}

#[derive(Default, Clone)]
pub struct Wu<T>(pub Weak<UnsafeCell<T>>);

impl<T> Wu<T> {
    pub fn ru(&self) -> Ru<T> {
        Ru(self.0.upgrade().unwrap())
    }
}

impl<T> Deref for Ru<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        unsafe { &*self.0.get() }
    }
}

impl<T> DerefMut for Ru<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { &mut *self.0.get() }
    }
}

use std::{
    cell::{RefMut, UnsafeCell},
    io,
    ops::{Deref, DerefMut},
    path::Path,
    process,
    rc::{Rc, Weak},
};
