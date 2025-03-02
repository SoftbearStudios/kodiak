// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: LGPL-3.0-or-later

use crate::{js_fetch, js_response_text, LanguageDto, LanguageId, TranslationResponse};
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use wasm_bindgen::JsValue;
use wasm_bindgen_futures::future_to_promise;

pub struct TranslationCache {
    pub(crate) languages: Rc<[LanguageDto]>,
    /// Presence of item in this map indicates GET has been sent.
    translations: RefCell<HashMap<LanguageId, Entry>>,
}

impl Default for TranslationCache {
    fn default() -> Self {
        Self {
            languages: Vec::new().into(),
            translations: Default::default(),
        }
    }
}

type Entry = Rc<RefCell<Rc<Translations>>>;

#[derive(Default, Debug)]
pub(crate) struct Translations {
    pub(crate) translations: HashMap<String, String>,
}

#[derive(Clone, Debug, Default)]
pub struct TranslationCacheLanguage(pub(crate) Rc<Translations>);

impl PartialEq for TranslationCacheLanguage {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}

impl TranslationCache {
    pub(crate) fn prepare_insert(&self, language_id: LanguageId) -> Option<Entry> {
        if let std::collections::hash_map::Entry::Vacant(vacant) =
            self.translations.borrow_mut().entry(language_id)
        {
            let ret = Entry::default();
            vacant.insert(ret.clone());
            Some(ret)
        } else {
            None
        }
    }

    pub fn get(&self, language_id: LanguageId) -> TranslationCacheLanguage {
        let mut request = Option::<Entry>::None;
        let ret = {
            TranslationCacheLanguage(Rc::clone(
                &self
                    .translations
                    .borrow_mut()
                    .entry(language_id)
                    .or_insert_with(|| {
                        let ret = Entry::default();
                        request = Some(ret.clone());
                        ret
                    })
                    .borrow(),
            ))
        };

        if let Some(request) = request {
            let _ = future_to_promise(async move {
                let Ok(response) = js_fetch(&format!(
                    "/translation.json?language_id={}",
                    language_id.as_str()
                ))
                .await
                else {
                    return Err(JsValue::UNDEFINED);
                };
                let Ok(json) = js_response_text(response).await else {
                    return Err(JsValue::UNDEFINED);
                };
                let Ok(response) = serde_json::from_str::<TranslationResponse>(&json) else {
                    return Err(JsValue::UNDEFINED);
                };
                *request.borrow_mut() = Rc::new(Translations {
                    translations: *response.translations,
                });
                Ok(JsValue::UNDEFINED)
            });
        }

        ret
    }
}
