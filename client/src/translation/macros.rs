// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: LGPL-3.0-or-later

/// Declare static translations.
#[macro_export]
macro_rules! s {
    ($name: ident) => {
        fn $name(self) -> &'static str;
    };
    ($name: ident, $value: expr) => {
        fn $name(self) -> &'static str {
            $value
        }
    };
}

#[macro_export]
macro_rules! sd {
    ($name: ident, $doc: literal) => {
        #[doc = $doc]
        fn $name(self) -> &'static str;
    };
}

/// Re-use static translations.
#[macro_export]
macro_rules! sl {
    ($name: ident, $link: ident) => {
        fn $name(self) -> &'static str {
            self.$link()
        }
    };
}
