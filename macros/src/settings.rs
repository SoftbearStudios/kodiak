// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: LGPL-3.0-or-later

use crate::str_lit_to_expr;
use convert_case::{Case, Casing};
use proc_macro::TokenStream;
use proc_macro2::{Ident, Span};
use quote::{format_ident, quote};
use syn::{
    parse_macro_input, Data, DataStruct, DeriveInput, Expr, ExprLit, Field, Fields, FieldsNamed,
    Lit, Meta, MetaList, NestedMeta,
};

pub(crate) fn derive_settings(input: TokenStream) -> TokenStream {
    let DeriveInput { ident, data, .. } = parse_macro_input!(input);
    if let Data::Struct(DataStruct { fields, .. }) = data {
        if let Fields::Named(FieldsNamed { named, .. }) = fields {
            let mut loaders = Vec::with_capacity(named.len());
            let mut getters = Vec::with_capacity(named.len());
            let mut setters = Vec::with_capacity(named.len());
            let mut validators = Vec::with_capacity(named.len());
            let mut displayers = Vec::with_capacity(named.len());
            let mut preferences = Vec::with_capacity(named.len());
            let mut statistics = Vec::with_capacity(named.len());
            let mut synchronizers = Vec::with_capacity(named.len());

            for Field {
                ident, ty, attrs, ..
            } in named
            {
                let ident = ident.expect("uh oh");
                let mut ident_string = ident.to_string().to_case(Case::Camel);
                let getter_name = format_ident!("get_{}", ident);
                let setter_name = format_ident!("set_{}", ident);
                let validator_name = format_ident!("validate_{}", ident);
                let mut range = None;
                let mut post = false;

                let mut storage = quote! { essential_storage };
                let mut optional = false;
                let mut validations = Vec::new();

                for attribute in attrs.into_iter().filter(|a| a.path.is_ident("setting")) {
                    let meta = attribute.parse_meta().expect("couldn't parse as meta");
                    if let Meta::List(MetaList { nested, .. }) = meta {
                        for meta in nested {
                            match meta {
                                NestedMeta::Meta(Meta::NameValue(meta)) => {
                                    if meta.path.is_ident("range") {
                                        let valid_range = str_lit_to_expr(meta.lit);
                                        let Expr::Range(ref expr_range) = valid_range else {
                                            panic!("invalid range");
                                        };
                                        let Expr::Lit(ExprLit {
                                            lit: Lit::Float(from),
                                            ..
                                        }) = expr_range.from.as_deref().unwrap()
                                        else {
                                            panic!("invalid range start");
                                        };
                                        let Expr::Lit(ExprLit {
                                            lit: Lit::Float(to),
                                            ..
                                        }) = expr_range.to.as_deref().unwrap()
                                        else {
                                            panic!("invalid range end");
                                        };
                                        range = Some(
                                            from.base10_parse::<f32>().unwrap()
                                                ..=to.base10_parse::<f32>().unwrap(),
                                        );
                                        validations.push(quote! {
                                            let valid = #valid_range;
                                            let value = value.clamp(valid.start, valid.end);
                                        });
                                    } else if meta.path.is_ident("rename") {
                                        ident_string = if let Lit::Str(s) = meta.lit {
                                            s.value()
                                        } else {
                                            panic!("must rename to string");
                                        };
                                    } else if meta.path.is_ident("checkbox") {
                                        let label = if let Lit::Str(s) = meta.lit {
                                            s.value()
                                        } else {
                                            panic!("must label as string");
                                        };
                                        let (category, label) =
                                            label.split_once('/').unwrap_or(("General", &label));
                                        let category = Ident::new(category, Span::call_site());
                                        displayers.push(quote! {
                                            checkbox(SettingCategory::#category, settings_prerequisites::translate!(t, #label), self.#ident, Self::#setter_name);
                                        });
                                    } else if meta.path.is_ident("dropdown") {
                                        let label = if let Lit::Str(s) = meta.lit {
                                            s.value()
                                        } else {
                                            panic!("must label as string");
                                        };
                                        let (category, label) =
                                            label.split_once('/').unwrap_or(("General", &label));

                                        let category = Ident::new(category, Span::call_site());

                                        displayers.push(quote! {
                                            dropdown(SettingCategory::#category, settings_prerequisites::translate!(t, #label), self.#ident.into(), |n: usize| {
                                                <#ty as strum::IntoEnumIterator>::iter().nth(n).map(|variant| {
                                                    let val : &'static str = variant.into();
                                                    (
                                                        val,
                                                        <#ty as strum::EnumMessage>::get_message(&variant).unwrap()
                                                    )
                                                })
                                            }, |settings, string, browser_storages| {
                                                if let Ok(value) = <#ty as std::str::FromStr>::from_str(string) {
                                                    Self::#setter_name(settings, value, browser_storages);
                                                }
                                            });
                                        });
                                    } else if meta.path.is_ident("slider") {
                                        let label = if let Lit::Str(s) = meta.lit {
                                            s.value()
                                        } else {
                                            panic!("must label as string");
                                        };
                                        let (category, label) =
                                            label.split_once('/').unwrap_or(("General", &label));

                                        let category = Ident::new(category, Span::call_site());
                                        let range = range
                                            .as_ref()
                                            .expect("slider must have a declared range");
                                        let start = range.start();
                                        let end = range.end();
                                        displayers.push(quote! {
                                            slider(SettingCategory::#category, settings_prerequisites::translate!(t, #label), self.#ident.into(), #start..=#end, |settings, value, browser_storages| {
                                                Self::#setter_name(settings, value, browser_storages);
                                            });
                                        });
                                    }
                                }
                                NestedMeta::Meta(Meta::Path(path)) => {
                                    let mut valid_path = false;
                                    if path.is_ident("finite") {
                                        validations.push(quote! {
                                            if !value.is_finite() {
                                                return None;
                                            }
                                        });
                                        valid_path = true;
                                    } else if path.is_ident("optional") {
                                        optional = true;
                                        valid_path = true;
                                    }
                                    if path.is_ident("preference") {
                                        storage = quote! { preference_storage };
                                        preferences.push(ident_string.clone());
                                        valid_path = true;
                                    } else if path.is_ident("statistic") {
                                        storage = quote! { statistic_storage };
                                        statistics.push(ident_string.clone());
                                        valid_path = true;
                                    } else if path.is_ident("volatile") {
                                        storage = quote! { volatile_storage };
                                        valid_path = true;
                                    } else if path.is_ident("no_store") {
                                        storage = quote! { no_storage };
                                        valid_path = true;
                                    } else if path.is_ident("post") {
                                        assert!(!post);
                                        post = true;
                                        valid_path = true;
                                    }
                                    if !valid_path {
                                        panic!("Unexpected path: {}", path.get_ident().unwrap());
                                    }
                                }
                                _ => panic!("Expected nested name-value pair"),
                            }
                        }
                    } else {
                        panic!("Expected a list");
                    }
                }

                assert!(
                    !optional || validations.is_empty(),
                    "cant be optional and have validations"
                );
                if post {
                    if optional {
                        synchronizers.push(quote! {
                            if let Some(value) = &self.#ident {
                                let string = value.to_string();
                                if known.get(#ident_string) != Some(&string) {
                                    browser_storages.buffer(#ident_string, &string);
                                }
                            } else if known.contains_key(#ident_string) {
                                browser_storages.buffer(#ident_string, "");
                            }
                        });
                    } else {
                        synchronizers.push(quote! {
                            let string = self.#ident.to_string();
                            let plasma = known.get(#ident_string);
                            let stored = browser_storages.#storage().get::<String>(#ident_string);
                            if plasma != Some(&string) && stored.is_some() {
                                browser_storages.buffer(#ident_string, &string);
                            } else if plasma.is_some() && stored.is_none() {
                                browser_storages.buffer(#ident_string, "");
                            }
                        });
                    }
                }
                let loader = if optional {
                    quote! {
                        #ident: {
                            // caib: debug_assert_eq!(default.#ident, None, "optional defaults must be None");
                            // finnb: why?
                            browser_storages.#storage().get(#ident_string).or(default.#ident)
                        },
                    }
                } else {
                    quote! {
                        #ident: browser_storages.#storage().get(#ident_string).and_then(Self::#validator_name).unwrap_or(default.#ident),
                    }
                };
                let getter = quote! {
                    pub fn #getter_name(&self) -> #ty {
                        self.#ident.clone()
                    }
                };

                let setter = if optional {
                    quote! {
                        pub fn #setter_name(&mut self, value: #ty, browser_storages: &mut BrowserStorages) {
                            let string = value.as_ref().map(|v| v.to_string());
                            if #post {
                                browser_storages.buffer(#ident_string, string.as_deref().unwrap_or(""));
                            }
                            let _ = browser_storages.#storage().set(#ident_string, string.as_deref());
                            self.#ident = value;
                        }
                    }
                } else {
                    quote! {
                        pub fn #setter_name(&mut self, value: #ty, browser_storages: &mut BrowserStorages) {
                            if let Some(valid) = Self::#validator_name(value) {
                                let string = valid.to_string();
                                if #post {
                                    browser_storages.buffer(#ident_string, &string);
                                }
                                let _ = browser_storages.#storage().set(#ident_string, Some(&string));
                                self.#ident = valid;
                            }
                        }
                    }
                };

                if !optional {
                    let validator = quote! {
                        fn #validator_name(value: #ty) -> Option<#ty> {
                            #(#validations)*
                            Some(value)
                        }
                    };

                    validators.push(validator);
                }

                loaders.push(loader);
                getters.push(getter);
                setters.push(setter);
            }

            let output = quote! {
                impl settings_prerequisites::LocalSettings for #ident {
                    fn load(browser_storages: &BrowserStorages, default: Self) -> Self {
                        Self {
                            #(#loaders)*
                        }
                    }

                    fn preferences() -> &'static [&'static str] {
                        &[
                            #(#preferences),*
                        ]
                    }

                    fn statistics() -> &'static [&'static str] {
                        &[
                            #(#statistics),*
                        ]
                    }

                    fn synchronize(&self, known: &std::collections::HashMap<String, String>, browser_storages: &mut BrowserStorages) {
                        let _ = (&known, &browser_storages);
                        #(#synchronizers)*
                    }

                    fn display(
                        &self,
                        t: &settings_prerequisites::Translator,
                        mut checkbox: impl FnMut(
                            SettingCategory,
                            String,
                            bool,
                            fn(&mut Self, bool, &mut BrowserStorages)
                        ),
                        mut dropdown: impl FnMut(
                            SettingCategory,
                            String,
                            &'static str,
                            fn(usize) -> Option<(&'static str, &'static str)>,
                            fn(&mut Self, &str, &mut BrowserStorages)
                        ),
                        mut slider: impl FnMut(
                            SettingCategory,
                            String,
                            f32,
                            std::ops::RangeInclusive<f32>,
                            fn(&mut Self, f32, &mut BrowserStorages)
                        ),
                    ) {
                        let _ = (&mut checkbox, &mut dropdown, &mut slider);
                        #(#displayers)*
                    }
                }

                impl #ident {
                    #(#getters)*
                    #(#setters)*
                    #(#validators)*
                }
            };
            output.into()
        } else {
            panic!("Must have named fields.");
        }
    } else {
        panic!("Must be struct");
    }
}
