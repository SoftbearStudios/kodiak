// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: LGPL-3.0-or-later

use std::fmt::Debug;
use std::marker::Unsize;
use std::ops::{CoerceUnsized, Deref};
use std::rc::Rc;

pub struct RcPtrEq<T: ?Sized>(Rc<T>);

impl<T: ?Sized + Unsize<U>, U: ?Sized> CoerceUnsized<RcPtrEq<U>> for RcPtrEq<T> {}

impl<T> RcPtrEq<T> {
    pub fn new(t: T) -> Self {
        Self(Rc::new(t))
    }
}

impl<T: ?Sized> Deref for RcPtrEq<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.0.deref()
    }
}

impl<T: ?Sized> Clone for RcPtrEq<T> {
    fn clone(&self) -> Self {
        Self(Rc::clone(&self.0))
    }
}

impl<T: ?Sized + Debug> Debug for RcPtrEq<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        Debug::fmt(&self.0, f)
    }
}

impl<T: ?Sized> PartialEq for RcPtrEq<T> {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}

impl<T: ?Sized> Eq for RcPtrEq<T> {}
