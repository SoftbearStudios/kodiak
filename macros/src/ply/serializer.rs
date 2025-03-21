// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: LGPL-3.0-or-later

use crate::ply::parser::{ElementType, Ply};
use proc_macro2::TokenStream;
use quote::quote;

impl Ply {
    pub(crate) fn into_triangle_tokens(mut self) -> TokenStream {
        let vertices = self.elements.remove(
            self.elements
                .iter()
                .position(|e| e._type == ElementType::Vertex)
                .expect("no vertices"),
        );
        let faces = self.elements.remove(
            self.elements
                .iter()
                .position(|e| e._type == ElementType::Face)
                .expect("no faces"),
        );
        assert!(self.elements.is_empty());

        let mut positions = 0;
        for position in ["x", "y", "z"] {
            positions += vertices
                .properties
                .iter()
                .any(|property| property.name == position) as u8;
        }
        assert_eq!(positions, 3, "must have Vec3 positions");

        let mut position_arrays = Vec::new();
        for vertex in &vertices.data {
            let position = ["x", "y", "z"].map(|position| {
                let index = vertices
                    .properties
                    .iter()
                    .position(|property| property.name == position)
                    .unwrap();

                vertex[index] as f32
            });
            position_arrays.push(position);
        }

        let mut index_ints = Vec::new();
        for face in &faces.data {
            assert_eq!(face.len(), 3, "must triangulate");
            for &index in face {
                // A mesh with indices over u16::MAX would add megabytes to the binary.
                let i: u16 = (index as u32)
                    .try_into()
                    .expect("indices must be <= u16::MAX");
                index_ints.push(i);
            }
        }

        let vertices: Vec<_> = index_ints
            .array_chunks()
            .flat_map(|&triangle: &[u16; 3]| {
                triangle
                    .into_iter()
                    .flat_map(|i| position_arrays[i as usize])
            })
            .collect();
        let vertex_slice = vertices.as_slice();

        // TODO might be slower to compile compared to alternatives.
        quote! {
            &[#(#vertex_slice),*]
        }
    }

    pub(crate) fn into_model_tokens(mut self) -> TokenStream {
        let vertices = self.elements.remove(
            self.elements
                .iter()
                .position(|e| e._type == ElementType::Vertex)
                .expect("no vertices"),
        );
        let faces = self.elements.remove(
            self.elements
                .iter()
                .position(|e| e._type == ElementType::Face)
                .expect("no faces"),
        );
        assert!(self.elements.is_empty());

        let mut positions = 0;
        for position in ["x", "y", "z"] {
            positions += vertices
                .properties
                .iter()
                .any(|property| property.name == position) as u8;
        }
        assert_eq!(positions, 3, "must have Vec3 positions");

        let mut normals = 0;
        for normal in ["nx", "ny", "nz"] {
            normals += vertices
                .properties
                .iter()
                .any(|property| property.name == normal) as u8;
        }
        assert!(normals == 0 || normals == positions);
        let normals = normals != 0;

        let mut uvs = 0;
        for uv in ["u", "v", "s", "t"] {
            uvs += vertices
                .properties
                .iter()
                .any(|property| property.name == uv) as u8;
        }
        assert!(matches!(uvs, 0 | 2));
        let uvs = uvs != 0;

        let mut colors = 0;
        let color_names = ["red", "green", "blue", "alpha"].map(|name| [name, &name[..1]]);
        for names in color_names {
            colors += vertices
                .properties
                .iter()
                .any(|property| names.contains(&&*property.name)) as u8;
        }
        assert!(matches!(colors, 0 | 4));
        let colors = colors != 0;

        let mut vertex_data = Vec::new();
        for vertex in &vertices.data {
            let mut push_float = |v: f64| vertex_data.push((v as f32).to_bits());
            for position in ["x", "y", "z"] {
                let index = vertices
                    .properties
                    .iter()
                    .position(|property| property.name == position)
                    .unwrap();
                push_float(vertex[index]);
            }
            if normals {
                for normal in ["nx", "ny", "nz"] {
                    let index = vertices
                        .properties
                        .iter()
                        .position(|property| property.name == normal)
                        .unwrap();
                    push_float(vertex[index]);
                }
            }
            for uv in ["u", "v", "s", "t"] {
                if let Some(index) = vertices
                    .properties
                    .iter()
                    .position(|property| property.name == uv)
                {
                    push_float(vertex[index]);
                }
            }
            if colors {
                let color = color_names.map(|names| {
                    let index = vertices
                        .properties
                        .iter()
                        .position(|property| names.contains(&&*property.name))
                        .unwrap();
                    vertex[index] as u8
                });
                vertex_data.push(u32::from_le_bytes(color));
            }
        }
        let vertex_slice = vertex_data.as_slice();

        let mut index_ints = Vec::new();
        for face in &faces.data {
            assert_eq!(face.len(), 3, "must triangulate");
            for &index in face {
                // A mesh with indices over u16::MAX would add megabytes to the binary.
                let i: u16 = (index as u32)
                    .try_into()
                    .expect("indices must be <= u16::MAX");
                index_ints.push(i);
            }
        }
        let index_slice = index_ints.as_slice();

        let c = if std::env::var("CARGO_PKG_NAME").unwrap() == "renderer3d" {
            quote!(crate)
        } else {
            quote!(renderer3d)
        };

        // TODO might be slower to compile compared to alternatives.
        quote! {
            #c::Model {
                vertices: &[#(#vertex_slice),*],
                indices: &[#(#index_slice),*],
                normals: #normals,
                uvs: #uvs,
                colors: #colors,
            }
        }
    }
}
