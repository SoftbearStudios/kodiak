// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: LGPL-3.0-or-later

use std::collections::HashMap;
use std::sync::Arc;

use super::ServerActor;
use crate::{
    ArenaService, LanguageDto, LanguageId, TranslationRequest, TranslationResponse, TranslationsDto,
};
use actix::Handler;

pub struct TranslationActlet {
    privacy: String,
    terms: String,
    dtos: Box<[TranslationsDto]>,
    cache: HashMap<LanguageId, Arc<HashMap<String, String>>>,
    pub(crate) languages: Arc<[LanguageDto]>,
}

impl Default for TranslationActlet {
    fn default() -> Self {
        Self {
            privacy: "Loading...".into(),
            terms: "Loading...".into(),
            dtos: Default::default(),
            cache: Default::default(),
            languages: vec![].into(),
        }
    }
}

impl TranslationActlet {
    pub fn update_privacy(&mut self, privacy: String) {
        self.privacy = privacy;
        self.cache.clear();
    }

    pub fn update_terms(&mut self, terms: String) {
        self.terms = terms;
        self.cache.clear();
    }

    pub fn update(&mut self, languages: Box<[LanguageDto]>, dtos: Box<[TranslationsDto]>) {
        self.languages = languages.into();
        self.dtos = dtos;
        self.cache.clear();
    }
}

impl<G: ArenaService> Handler<TranslationRequest> for ServerActor<G> {
    type Result = TranslationResponse;

    fn handle(&mut self, msg: TranslationRequest, _ctx: &mut Self::Context) -> TranslationResponse {
        let translations = self
            .translations
            .cache
            .entry(msg.language_id)
            .or_insert_with(|| {
                Arc::new(
                    self.translations
                        .dtos
                        .iter()
                        .filter(|dto| dto.bulktext || msg.language_id != LanguageId::new("en"))
                        .filter_map(|dto| {
                            dto.translated_text
                                .get(&msg.language_id)
                                // Send bulktext because client doesn't have English.
                                .or_else(|| {
                                    dto.translated_text
                                        .get(&Default::default())
                                        .filter(|_| dto.bulktext)
                                })
                                .zip(
                                    dto.translation_id
                                        .as_ref()
                                        //.filter(|_| dto.bulktext)
                                        .cloned()
                                        .or_else(|| {
                                            dto.translated_text.get(&LanguageId::new("en")).cloned()
                                        }),
                                )
                                .map(|(translation, id)| (id, translation.clone()))
                        })
                        .chain([
                            ("terms_md".to_owned(), self.translations.terms.clone()),
                            ("privacy_md".to_owned(), self.translations.privacy.clone()),
                        ])
                        .collect(),
                )
            });
        TranslationResponse {
            translations: Arc::clone(&*translations),
        }
    }
}
