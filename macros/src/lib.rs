// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: LGPL-3.0-or-later

#![feature(array_chunks)]
#![feature(iter_intersperse)]
#![feature(iterator_try_collect)]
#![feature(proc_macro_span)]
#![feature(track_path)]
#![feature(box_into_inner)]

mod audio;
mod hb_hash;
mod layer;
#[cfg(feature = "ply")]
mod ply;
mod settings;
mod smol_routable;
mod sprite_sheet;
mod texture;
mod translate;
mod vertex;

use convert_case::Casing;
use proc_macro::TokenStream;
use proc_macro2::Span;
use syn::{parse_macro_input, Expr, Lit};

#[proc_macro]
pub fn include_audio(item: TokenStream) -> TokenStream {
    audio::include_audio(item)
}

#[cfg(feature = "ply")]
#[proc_macro]
pub fn include_ply(item: TokenStream) -> TokenStream {
    ply::include_ply(item)
}

#[cfg(feature = "ply")]
#[proc_macro]
pub fn include_plys_into_model(item: TokenStream) -> TokenStream {
    ply::include_plys(item, false, true)
}

#[cfg(feature = "ply")]
#[proc_macro]
pub fn include_plys_into_triangles(item: TokenStream) -> TokenStream {
    ply::include_plys(item, false, false)
}

#[cfg(feature = "ply")]
#[proc_macro]
pub fn include_plys_define(item: TokenStream) -> TokenStream {
    ply::include_plys(item, true, false)
}

#[proc_macro]
pub fn include_textures(item: TokenStream) -> TokenStream {
    texture::include_textures(item)
}

#[proc_macro_derive(Layer, attributes(alpha, depth, layer, render, stencil))]
pub fn derive_layer(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as layer::LayerInput);
    layer::derive_layer(input)
}

#[proc_macro_derive(Settings, attributes(setting))]
pub fn derive_settings(input: TokenStream) -> TokenStream {
    settings::derive_settings(input)
}

#[proc_macro_derive(Vertex)]
pub fn derive_vertex(input: TokenStream) -> TokenStream {
    vertex::derive_vertex(input)
}

#[proc_macro_derive(HbHash, attributes(hb_hash))]
pub fn derive_hb_hash(input: TokenStream) -> TokenStream {
    hb_hash::derive_hb_hash(input)
}

#[proc_macro_derive(SmolRoutable, attributes(at, not_found))]
pub fn smol_routable_derive(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as smol_routable::SmolRoutable);
    smol_routable::derive_smol_routable(input)
}

#[proc_macro]
pub fn translate(item: TokenStream) -> TokenStream {
    translate::translate(item)
}

#[proc_macro]
pub fn translated_text(item: TokenStream) -> TokenStream {
    translate::translated_text(item)
}

fn str_lit_to_expr(lit: Lit) -> Expr {
    if let Lit::Str(s) = lit {
        let string = s.value();
        let str = string.as_str();
        syn::parse_str::<Expr>(str).expect(str)
    } else {
        panic!("expected string literal")
    }
}

fn name_to_ident(name: String) -> proc_macro2::Ident {
    let upper_camel = name.to_case(convert_case::Case::UpperCamel);
    proc_macro2::Ident::new(&upper_camel, Span::call_site())
}
