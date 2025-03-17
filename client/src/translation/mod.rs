// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: LGPL-3.0-or-later

mod cache;
mod macros;
mod phrases;
mod translator;

pub(crate) use self::cache::{TranslationCache, Translations};
pub use self::translator::{use_translator, TranslateFn, Translator};

// Re-export.
pub use crate::translate;
