// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: LGPL-3.0-or-later

use crate::bitcode::{self, Decode, Encode, Error};

thread_local! {
    static BUFFER: std::cell::RefCell<bitcode::Buffer> = Default::default();
}

pub fn decode_buffer<'a, T: Decode<'a> + ?Sized>(bytes: &'a [u8]) -> Result<T, Error> {
    BUFFER.with(|b| b.borrow_mut().decode(bytes))
}

pub fn encode_buffer<T: Encode + ?Sized>(t: &T) -> Vec<u8> {
    BUFFER.with(|b| b.borrow_mut().encode(t).to_owned())
}
