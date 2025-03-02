// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: LGPL-3.0-or-later

use super::cache::TranslationCacheLanguage;
use crate::{use_ctw, LanguageDto, LanguageId, RcPtrEq};
use std::fmt::Write;
use std::rc::Rc;
use yew::hook;

/// Only works in function component.
#[hook]
pub fn use_translator() -> Translator {
    use_ctw().translator
}

/// The output of the `translate!` macro if no API is passed.
pub type TranslateFn = RcPtrEq<dyn Fn(&Translator) -> String>;

#[derive(Clone, Debug, PartialEq)]
pub struct Translator {
    pub language_id: LanguageId,
    pub(crate) languages: Rc<[LanguageDto]>,
    pub(crate) translations: TranslationCacheLanguage,
}

impl Translator {
    pub fn translated_text(&self, id: &str) -> String {
        self.translate_phrase(Some(id), "", &[])
    }

    pub fn translate_phrase(
        &self,
        phrase_id: Option<&str>,
        phrase: &str,
        vars: &[(&'static str, String)],
    ) -> String {
        let translated_phrase = self
            .translations
            .0
            .translations
            .get(phrase_id.unwrap_or(phrase))
            .map(|x| x.as_str());
        let mut ret = translated_phrase.unwrap_or(phrase).to_owned();
        let mut buffer = String::new();
        for (from, to) in vars {
            buffer.clear();
            write!(&mut buffer, "{{{from}}}").unwrap();
            ret = ret.replace(&buffer, to);
        }
        ret
    }
}
