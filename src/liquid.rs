use std::{
    cell::UnsafeCell,
    collections::HashMap,
    fmt::{self, Debug},
    io::{Error, ErrorKind},
    ops::{Deref, DerefMut},
    rc::{Rc, Weak},
};

use crate::{
    parser::{LiquidState, ParseError},
    MutRef,
};

pub type Object = HashMap<String, Liquid>;
pub type Array = Vec<Liquid>;

#[derive(Clone)]
pub enum LiquidInner {
    String(String),
    Int(i64),
    Bool(bool),
    Object(MutRc<Object>),
    WeakObject(Weak<UnsafeCell<Object>>),
    Array(MutRc<Array>),
    WeakArray(Weak<UnsafeCell<Array>>),
    Nil,
}

impl Debug for LiquidInner {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LiquidInner::String(arg0) => arg0.fmt(f),
            LiquidInner::Int(arg0) => arg0.fmt(f),
            LiquidInner::Bool(arg0) => arg0.fmt(f),
            LiquidInner::Object(arg0) => {
                if arg0.get_mut().contains_key("contents") {
                    let hash_map = arg0.get_mut();
                    let value = hash_map.insert("contents".to_string(), "{{ contents }}".into());

                    let res = hash_map.fmt(f);

                    if let Some(value) = value {
                        hash_map.insert("contents".to_string(), value);
                    } else {
                        hash_map.remove("contents");
                    }

                    res
                } else {
                    arg0.get_mut().fmt(f)
                }
            }
            LiquidInner::WeakObject(arg0) => arg0.fmt(f),
            LiquidInner::Array(arg0) => arg0.get_mut().fmt(f),
            LiquidInner::WeakArray(arg0) => arg0.fmt(f),
            LiquidInner::Nil => write!(f, "Nil"),
        }
    }
}

impl Into<Liquid> for LiquidInner {
    fn into(self) -> Liquid {
        Liquid { inner: UnsafeCell::new(self) }
    }
}

pub struct Liquid {
    pub inner: UnsafeCell<LiquidInner>,
}

impl Debug for Liquid {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.inner.mut_ref().fmt(f)
    }
}

impl Clone for Liquid {
    fn clone(&self) -> Self {
        Self { inner: UnsafeCell::new(self.inner.mut_ref().clone()) }
    }
}

impl Liquid {
    pub fn default_nil() -> Self {
        LiquidInner::Nil.into()
    }

    pub fn default_string() -> Self {
        LiquidInner::String(Default::default()).into()
    }

    pub fn default_int() -> Self {
        LiquidInner::Int(Default::default()).into()
    }

    pub fn default_bool() -> Self {
        LiquidInner::Bool(Default::default()).into()
    }

    pub fn default_object() -> Self {
        LiquidInner::Object(Default::default()).into()
    }

    pub fn default_array() -> Self {
        LiquidInner::Array(Default::default()).into()
    }

    pub fn is(&self) -> bool {
        match &self.inner.mut_ref() {
            LiquidInner::String(s) => !s.is_empty(),
            LiquidInner::Int(_) => true,
            LiquidInner::Bool(b) => *b,
            LiquidInner::Object(o) => !o.get_mut().is_empty(),
            LiquidInner::WeakObject(o) => {
                if let Some(o) = o.upgrade() {
                    !o.mut_ref().is_empty()
                } else {
                    false
                }
            }
            LiquidInner::Array(o) => !o.get_mut().is_empty(),
            LiquidInner::WeakArray(o) => {
                if let Some(o) = o.upgrade() {
                    !o.mut_ref().is_empty()
                } else {
                    false
                }
            }
            LiquidInner::Nil => false,
        }
    }

    pub fn is_string(&self) -> bool {
        match self.inner.mut_ref() {
            LiquidInner::String(_) => true,
            _ => false,
        }
    }

    pub fn is_int(&self) -> bool {
        match self.inner.mut_ref() {
            LiquidInner::Int(_) => true,
            _ => false,
        }
    }

    pub fn is_bool(&self) -> bool {
        match self.inner.mut_ref() {
            LiquidInner::Bool(_) => true,
            _ => false,
        }
    }

    pub fn is_object(&self) -> bool {
        match self.inner.mut_ref() {
            LiquidInner::Object(_) => true,
            LiquidInner::WeakObject(_) => true,
            _ => false,
        }
    }

    pub fn is_array(&self) -> bool {
        match self.inner.mut_ref() {
            LiquidInner::Array(_) => true,
            LiquidInner::WeakArray(_) => true,
            _ => false,
        }
    }

    pub fn is_nil(&self) -> bool {
        match self.inner.mut_ref() {
            LiquidInner::Nil => true,
            _ => false,
        }
    }

    pub fn as_string(&self) -> Option<&mut String> {
        match self.inner.mut_ref() {
            LiquidInner::String(s) => Some(s),
            _ => None,
        }
    }

    pub fn as_int(&self) -> Option<i64> {
        match self.inner.mut_ref() {
            LiquidInner::Int(i) => Some(*i),
            _ => None,
        }
    }

    pub fn as_bool(&self) -> Option<bool> {
        match self.inner.mut_ref() {
            LiquidInner::Bool(b) => Some(*b),
            _ => None,
        }
    }

    pub fn as_object(&self) -> Option<MutRc<Object>> {
        match self.inner.mut_ref() {
            LiquidInner::Object(o) => Some(o.clone()),
            LiquidInner::WeakObject(o) => {
                if let Some(o) = o.upgrade() {
                    Some(MutRc(o, true))
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    pub fn as_array(&self) -> Option<MutRc<Array>> {
        match self.inner.mut_ref() {
            LiquidInner::Array(o) => Some(o.clone()),
            LiquidInner::WeakArray(o) => {
                if let Some(o) = o.upgrade() {
                    Some(MutRc(o, true))
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    pub fn len(&self) -> Option<usize> {
        match self.inner.mut_ref() {
            LiquidInner::Bool(_) => None,
            LiquidInner::Int(_) => None,
            LiquidInner::Nil => None,
            LiquidInner::String(s) => Some(s.len()),
            LiquidInner::Object(_) | LiquidInner::WeakObject(_) => {
                Some(self.as_object()?.get_mut().len())
            }
            LiquidInner::Array(_) | LiquidInner::WeakArray(_) => {
                Some(self.as_array()?.get_mut().len())
            }
        }
    }

    pub fn with_string(&self, mut f: impl FnMut(&mut String)) {
        if let Some(v) = self.as_string() {
            f(v)
        }
    }

    pub fn with_int(&self, mut f: impl FnMut(i64)) {
        if let Some(v) = self.as_int() {
            f(v)
        }
    }

    pub fn with_bool(&self, mut f: impl FnMut(bool)) {
        if let Some(v) = self.as_bool() {
            f(v)
        }
    }

    pub fn with_object(&self, mut f: impl FnMut(&mut Object)) {
        if let Some(mut v) = self.as_object() {
            f(&mut v)
        }
    }

    pub fn with_array(&self, mut f: impl FnMut(&mut Array)) {
        if let Some(mut v) = self.as_array() {
            f(&mut v)
        }
    }
}

impl From<()> for Liquid {
    fn from(_: ()) -> Self {
        Self { inner: UnsafeCell::new(LiquidInner::Nil) }
    }
}

impl From<bool> for Liquid {
    fn from(value: bool) -> Self {
        Self { inner: UnsafeCell::new(LiquidInner::Bool(value)) }
    }
}

impl From<i64> for Liquid {
    fn from(value: i64) -> Self {
        Self { inner: UnsafeCell::new(LiquidInner::Int(value)) }
    }
}

impl From<String> for Liquid {
    fn from(value: String) -> Self {
        Self { inner: UnsafeCell::new(LiquidInner::String(value)) }
    }
}

impl From<&str> for Liquid {
    fn from(value: &str) -> Self {
        Self { inner: UnsafeCell::new(LiquidInner::String(value.to_string())) }
    }
}

impl From<MutRc<Object>> for Liquid {
    fn from(value: MutRc<Object>) -> Self {
        if value.1 {
            Self { inner: UnsafeCell::new(LiquidInner::WeakObject(Rc::downgrade(&value.0))) }
        } else {
            Self { inner: UnsafeCell::new(LiquidInner::Object(value)) }
        }
    }
}

impl From<MutRc<Array>> for Liquid {
    fn from(value: MutRc<Array>) -> Self {
        if value.1 {
            Self { inner: UnsafeCell::new(LiquidInner::WeakArray(Rc::downgrade(&value.0))) }
        } else {
            Self { inner: UnsafeCell::new(LiquidInner::Array(value)) }
        }
    }
}

#[derive(Debug, Default, Clone)]
pub struct MutRc<T>(
    pub Rc<UnsafeCell<T>>,
    /// Is weak reference
    pub bool,
);

impl<T> MutRc<T> {
    pub fn new(x: T) -> Self {
        Self(Rc::new(UnsafeCell::new(x)), false)
    }

    pub fn get_mut(&self) -> &mut T {
        self.0.mut_ref()
    }
}

impl<T> Deref for MutRc<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.get_mut()
    }
}

impl<T> DerefMut for MutRc<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.get_mut()
    }
}

pub trait OptionToResult<'a, O, E, I>: Sized {
    fn result(self, state: &'a I) -> Result<O, E>;
}

impl<'local, T> OptionToResult<'local, T, ParseError, LiquidState<'local>> for Option<T> {
    fn result(self, _: &LiquidState) -> Result<T, ParseError> {
        match self {
            Some(o) => Ok(o),
            None => Err(ParseError::new(format!("Option::None"))),
        }
    }
}

impl<'local, T> OptionToResult<'local, T, Error, ()> for Option<T> {
    fn result(self, _: &()) -> Result<T, Error> {
        match self {
            Some(o) => Ok(o),
            None => Err(ErrorKind::NotFound.into()),
        }
    }
}
