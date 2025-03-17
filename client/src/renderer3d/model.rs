// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: LGPL-3.0-or-later

use crate::renderer::MeshBuilder;
use bytemuck::Pod;
use kodiak_common::glam::*;
use std::mem::size_of;

// Re-export.
pub use crate::include_ply;

/// A static 3D model that has vertices and indices. Its vertices contain positions, normals, uvs,
/// and colors.
#[derive(Debug)]
pub struct Model {
    /// Untyped slice of vertices with alignment of 4.
    pub vertices: &'static [u32],
    /// `u16` indices.
    pub indices: &'static [u16],
    /// If the [`Model`] has [`Vec3`] normals.
    pub normals: bool,
    /// If the [`Model`] has [`Vec2`] uvs.
    pub uvs: bool,
    /// If the [`Model`] has [`Vec4`] colors,
    pub colors: bool,
}

impl Model {
    /// Allocates a model as a [`MeshBuilder`].
    pub fn to_builder<V: Pod>(&self) -> MeshBuilder<V> {
        assert_eq!(
            self.vertex_len() * size_of::<u32>(),
            size_of::<V>(),
            "vertex size mismatch: {:?}",
            self
        );

        let mut builder = MeshBuilder::new();
        let vertices: &'static [V] = bytemuck::cast_slice(self.vertices);
        builder.vertices = vertices.to_owned();
        builder.indices = self.indices.to_owned();
        builder
    }

    /// Returns the size of each [`Vertex`] in untyped `u32`s.
    pub(crate) fn vertex_len(&self) -> usize {
        // positions always Vec3.
        let mut len = 3;
        if self.normals {
            len += 3;
        }
        if self.uvs {
            len += 2;
        }
        if self.colors {
            len += 1;
        }
        len
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use renderer::derive_vertex;

    derive_vertex!(
        #[derive(Debug)]
        struct TestVertex {
            pos: Vec3,
            normal: Vec3,
            uv: Vec2,
        }
    );
    const TEST_MODEL: Model = include_ply!("models/test.ply");

    #[test]
    fn model_to_builder() {
        let model = TEST_MODEL;
        println!("{:?}", model);
        let builder: MeshBuilder<TestVertex, _> = model.to_builder();
        println!("{:?}", builder);
    }
}
