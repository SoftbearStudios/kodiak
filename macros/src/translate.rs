// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: LGPL-3.0-or-later

use litrs::StringLit;
use proc_macro::TokenStream;
use proc_macro2::{Ident, Literal, Span, TokenStream as TokenStream2, TokenTree};
use quote::quote;
use serde::Serialize;
use std::collections::HashSet;
use std::env::var;
use std::fs::File;
use std::io::Write;

fn append_log(translation_id: &Option<Literal>, english_text: Option<&String>) {
    const PHRASES_TARGET_DIR: &str = "CARGO_RUSTC_CURRENT_DIR";
    const PHRASES_TXT: &str = "phrases.txt";

    let tid_string = translation_id.as_ref().map(string_literal);
    let target_dir = var(PHRASES_TARGET_DIR).expect(&format!(
        "${PHRASES_TARGET_DIR}: cannot read environment variable"
    ));
    let path = format!("{target_dir}/{PHRASES_TXT}");
    let mut file = File::options()
        .append(true)
        .create(true)
        .open(&path)
        .expect(&format!("{path}: unable to open file for writing"));

    if tid_string.is_none() && english_text.is_none() {
        panic!("{path}: expected ID or English text");
    }

    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct TranslationItem {
        #[serde(skip_serializing_if = "Option::is_none")]
        id: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        english_text: Option<String>,
    }
    let translation_item = TranslationItem {
        id: tid_string,
        english_text: english_text.map(|t| dequote(&t)),
    };
    let output =
        serde_json::to_string(&translation_item).expect("{path}: unable to serialize JSON");
    let cr = "\n";
    let output_cr = format!("{output}{cr}");
    file.write_all(output_cr.as_bytes())
        .expect(&format!("{path}: unable to append"));
    file.sync_all().expect("{path}: unable to sync");
}

fn dequote(s: &String) -> String {
    let quote = "\"";
    if s.starts_with(quote) && s.ends_with(quote) {
        let n = s.len() - 1;
        s[1..n].to_owned()
    } else {
        s.to_owned()
    }
}

pub fn string_literal(lit: &Literal) -> String {
    match StringLit::try_from(lit) {
        // Error if the token is not a string literal
        Err(e) => panic!("{e}"),
        Ok(lit) => lit.value().to_owned(),
    }
}

// Translate text if a translation is available.
//
// Note: the translate! macro is only run during re-compilation; that is, it is
// not run if source code hasn't changed and hence there is nothing to recompile.
//
// For example:
//   translate!(t, "Please, {name}, may I have some more!") -> t.translate_phrase("Please ...", 0, args)
//   translate!(t, "Please, {name}, may I have some more!"); -> t.translate_phrase("pls1", "Please ...", args)
//
//   where args is HashMap<String, String>
//
// If you don't pass t, it returns a function that takes a t.
//
pub fn translate(token_stream: TokenStream) -> TokenStream {
    let tokens: Vec<_> = TokenStream2::from(token_stream).into_iter().collect();
    let n = tokens.len();

    let mut api: Option<Ident> = None;
    let mut tid: Option<Literal> = None;
    let mut phrase: Option<Literal> = None;

    for (j, t) in tokens.into_iter().enumerate() {
        match t {
            TokenTree::Ident(ident) if j == 0 => {
                api = Some(ident);
            }
            TokenTree::Literal(literal)
                if (api.is_none() && j == 0 && n == 3) || (api.is_some() && j == 2 && n == 5) =>
            {
                tid = Some(literal);
            }
            TokenTree::Literal(literal)
                if (api.is_none() && (j == 0 && n == 1) || (j == 2 && n == 3))
                    || (api.is_some() && ((j == 2 && n == 3) || (j == 4 && n == 5))) =>
            {
                phrase = Some(literal);
            }
            _ => {}
        }
    }

    let phrase_string = phrase.as_ref().map(string_literal);

    if let Some(phrase_string) = phrase_string.as_ref() {
        append_log(&tid, Some(phrase_string));
    }

    let name_hash = phrase_string
        .as_ref()
        .map(|phrase_string| {
            let mut name_hash: HashSet<String> = HashSet::new();
            let mut name = Vec::new();
            let mut parsing_name = false;
            for ch in phrase_string.chars() {
                match ch {
                    '{' if !parsing_name => parsing_name = true,
                    '{' if parsing_name => parsing_name = false,
                    '}' if parsing_name => {
                        if !name.is_empty() {
                            let v = name.iter().collect();
                            name_hash.insert(v);
                            name.clear();
                        }
                        parsing_name = false;
                    }
                    _ => {
                        if parsing_name {
                            name.push(ch);
                        }
                    }
                }
            }
            name_hash
        })
        .unwrap_or_default();

    let vars = name_hash.into_iter().map(|name| {
        let vid = Ident::new(&name, Span::call_site());
        quote! {
            (#name, #vid.to_string())
        }
    });
    let phrase = phrase.expect("translate! requires phrase");
    let tid = tid.map(|n| quote! {Some(#n)}).unwrap_or(quote! {None});

    let call = quote! {
        translate_phrase(#tid, #phrase, &[#(#vars),*])
    };

    if let Some(api) = api {
        quote! {
            #api.#call
        }
        .into()
    } else {
        quote! {
            translation_prerequisites::RcPtrEq::new(move |api| api.#call)
        }
        .into()
    }
}

// Reference a (large amount of) translated (e.g. markdown) text by ID.
//
// Note: the translated_text! macro is only run during re-compilation; that is, it is
// not run if source code hasn't changed and hence there is nothing to recompile.
//
// For example:
//   translated_text!(t, "help") -> t.translated_text("help")
//
pub fn translated_text(token_stream: TokenStream) -> TokenStream {
    let tokens: Vec<_> = TokenStream2::from(token_stream).into_iter().collect();

    let mut api: Option<Ident> = None;
    let mut tid: Option<Literal> = None;

    for (j, t) in tokens.into_iter().enumerate() {
        match t {
            TokenTree::Ident(ident) if j == 0 => {
                api = Some(ident);
            }
            TokenTree::Literal(literal) if j == 2 => {
                tid = Some(literal);
            }
            _ => {}
        }
    }

    append_log(&tid, None);

    let api = api.expect("translated_text! requires api");

    quote! {
        #api.translated_text(#tid)
    }
    .into()
}
