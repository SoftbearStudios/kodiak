// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: LGPL-3.0-or-later

use glam::{uvec2, UVec2};
use std::fmt;
use std::fmt::Formatter;

pub type Rect = (UVec2, UVec2);

type V = u64;

/// A 2D bitset.
#[derive(Default)]
pub struct Mask2d {
    /// Length must be `dim.x * dim.y`.
    mask: Box<[V]>,
    /// dims.x is always a multiple of `V::BITS`.
    dims: UVec2,
    /// Useful for debug printing since dims.x is rounded up to a multiple of `V::BITS`.
    original_x: u32,
}

impl Clone for Mask2d {
    fn clone(&self) -> Self {
        Self {
            mask: self.mask.clone(),
            dims: self.dims,
            original_x: self.original_x,
        }
    }

    fn clone_from(&mut self, source: &Self) {
        self.mask.clone_from(&source.mask);
        self.dims = source.dims;
        self.original_x = source.original_x;
    }
}

impl Mask2d {
    fn empty(dims: UVec2) -> Self {
        let original_x = dims.x;

        // Round x up to units of V.
        let dims = uvec2(round_x_up(original_x), dims.y);
        let x_dim_v = dims.x / V::BITS;

        let me = Self {
            mask: vec![0 as V; x_dim_v as usize * dims.y as usize].into_boxed_slice(),
            dims,
            original_x,
        };
        debug_assert_eq!(me.mask.len() as u32 * V::BITS, me.dims.y * me.dims.x);
        me
    }

    #[cfg_attr(not(debug_assertions), allow(unused))]
    fn from_rects(rects: impl Iterator<Item = Rect>, dims: UVec2) -> Self {
        let mut me = Self::empty(dims);
        for (s, e) in rects {
            for y in s.y..=e.y {
                for x in s.x..=e.x {
                    me.set(uvec2(x, y))
                }
            }
        }
        me
    }

    /// Creates a [`Mask`] that contains `points`. All `points` must be less than `dim`. Duplicate
    /// `points` are ok.
    ///
    /// **Panics**
    ///
    /// If any one of `points` is >= `dim`.
    #[inline]
    pub fn new(points: impl IntoIterator<Item = UVec2>, dims: UVec2) -> Self {
        let mut me = Self::empty(dims);
        for p in points {
            me.set(p)
        }
        me
    }

    /// Like [`new`][`Self::new`] but expands each point to a square `kernel`. Even `kernel`s will
    /// add more than they subtract.
    ///
    /// **Panics**
    ///
    /// If any one of `points` is >= `dim`.
    /// If `kernel` is zero.
    pub fn new_expanded(points: impl IntoIterator<Item = UVec2>, dims: UVec2, kernel: u32) -> Self {
        assert_ne!(kernel, 0);

        let sub = (kernel - 1) / 2;
        let add = kernel / 2 + 1;

        let mut me = Self::empty(dims);
        let dims = me.dims;

        for p in points {
            assert!(p.cmple(dims).all());

            // TODO use bitwise operators to speed this up for large kernel sizes.
            for y in p.y.saturating_sub(sub)..(p.y + add).min(dims.y) {
                for x in p.x.saturating_sub(sub)..(p.x + add).min(dims.x) {
                    me.set(uvec2(x, y));
                }
            }
        }
        me
    }

    #[inline]
    fn set(&mut self, pos: UVec2) {
        set_2d_mut(&mut self.mask, self.dims, pos.x, pos.y, true);
    }

    /// Like [`take_rects`][`Self::take_rects`] but more efficient since it doesn't have to
    /// allocate. Clears the [`Mask`].
    /// TODO find a way to efficiently implement an iterator version of this.
    #[inline]
    pub fn take_rects_with_fn(&mut self, mut f: impl FnMut(Rect)) {
        let Self {
            mask,
            dims,
            original_x: _,
        } = self;
        let dims = *dims;

        debug_assert_eq!(mask.len(), (dims.x / V::BITS * dims.y) as usize);

        // Use greedy meshing algorithm.
        for y in 0..dims.y {
            let mut previous_x = 0;

            while let Some(x) = first_one_starting_at(index_1d(mask, dims, y), previous_x) {
                let end = if let Some(end) = first_zero_starting_at(index_1d(mask, dims, y), x) {
                    end
                } else {
                    dims.x
                };
                clear_bit_range(index_1d_mut(mask, dims, y), x, end);

                let mut y2 = y + 1;
                while y2 < dims.y {
                    let slice = &mut index_1d_mut(mask, dims, y2);

                    // Make sure we don't scan chunks past end. This is not required for correctness
                    // but it improves the perf of large circles.
                    let slice = &mut slice[..(round_x_up(end) / V::BITS) as usize];

                    let first_zero = first_zero_starting_at(slice, x);
                    let all = if let Some(first_zero) = first_zero {
                        first_zero >= end
                    } else {
                        true
                    };

                    if all {
                        clear_bit_range(slice, x, end);
                    } else {
                        break;
                    }
                    y2 += 1;
                }

                f((uvec2(x, y), uvec2(end - 1, y2 - 1)));
                previous_x = end;
            }
        }
    }

    /// TODO make this public once it doesn't allocate.
    /// Clears the [`Mask`] over every rect returned.
    #[cfg_attr(not(test), allow(unused))]
    pub(crate) fn take_rects_iter(&mut self) -> impl Iterator<Item = Rect> {
        self.take_rects().into_iter()
    }

    /// Returns a the rectangles that cover the [`Mask`]. Rects are inclusive start/end points.
    /// Clears the [`Mask`].
    pub fn take_rects(&mut self) -> Vec<Rect> {
        // Preserve a copy of mask before it's modified in debug mode for assertions.
        #[cfg(debug_assertions)]
        let mask1 = self.clone();

        let mut rects = vec![];
        self.take_rects_with_fn(|rect| rects.push(rect));

        // Check results in debug mode.
        #[cfg(debug_assertions)]
        {
            let dims = mask1.dims;
            let original_dims = uvec2(mask1.original_x, dims.y);

            let mask1 = mask1;
            let mask2 = Self::from_rects(rects.iter().copied(), original_dims);
            if mask1.mask != mask2.mask {
                let mut s = String::from("mask1\n");
                s += &format!("{mask1:?}");
                s += "mask2\n";
                s += &format!("{mask2:?}");
                let mut diff = mask1;
                for y in 0..dims.y {
                    for x in 0..original_dims.x {
                        let v =
                            index_2d(&diff.mask, dims, x, y) != index_2d(&mask2.mask, dims, x, y);
                        set_2d_mut(&mut diff.mask, dims, x, y, v);
                    }
                }
                s += "diff\n";
                s += &format!("{diff:?}");
                panic!("{}", s);
            }
        }
        rects
    }
}

impl fmt::Debug for Mask2d {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        assert_eq!(self.mask.len() as u32 * V::BITS, self.dims.y * self.dims.x);
        for y in 0..self.dims.y {
            for x in 0..self.original_x {
                let v = index_2d(&self.mask, self.dims, x, y);
                write!(f, "{}", (b'0' + v as u8) as char)?;
            }
            writeln!(f)?;
        }
        Ok(())
    }
}

#[inline]
fn index_1d(m: &[V], dims: UVec2, y: u32) -> &[V] {
    let x_dim_v = dims.x / V::BITS;
    &m[(y * x_dim_v) as usize..((y + 1) * x_dim_v) as usize]
}

#[inline]
fn index_2d(m: &[V], dims: UVec2, x: u32, y: u32) -> bool {
    let index = y * dims.x + x;
    let i = index / V::BITS;
    let f = index % V::BITS;
    m[i as usize] & (1 << f) != 0
}

#[inline]
fn index_1d_mut(m: &mut [V], dims: UVec2, y: u32) -> &mut [V] {
    let x_dim_v = dims.x / V::BITS;
    &mut m[(y * x_dim_v) as usize..((y + 1) * x_dim_v) as usize]
}

#[inline]
fn set_2d_mut(m: &mut [V], dims: UVec2, x: u32, y: u32, v: bool) {
    let index = y * dims.x + x;
    let i = index / V::BITS;
    let f = index % V::BITS;

    let bit = 1 << f;
    if v {
        m[i as usize] |= bit;
    } else {
        m[i as usize] &= !bit;
    }
}

fn round_x_up(x: u32) -> u32 {
    let rounded_up = x.next_multiple_of(V::BITS);
    debug_assert_eq!(rounded_up % V::BITS, 0);
    debug_assert!(rounded_up >= x);
    rounded_up
}

fn first_bit_starting_at(slice: &[V], x: u32, bit: bool) -> Option<u32> {
    let i = x / V::BITS;
    let f = x % V::BITS;

    let v = *slice.get(i as usize)?;
    let v = if bit { v } else { !v };

    // Always do this even if f is zero (no mask).
    let mask = !((1 << f) - 1);
    let v = v & mask;

    if v != 0 {
        let f2 = v.trailing_zeros();
        return Some(i * V::BITS + f2);
    }
    let i = i + 1;

    #[allow(clippy::needless_range_loop)]
    for i in i as usize..slice.len() {
        let v = if bit { slice[i] } else { !slice[i] };

        if v != 0 {
            let f2 = v.trailing_zeros();
            return Some(i as u32 * V::BITS + f2);
        }
    }

    None
}

fn first_one_starting_at(slice: &[V], x: u32) -> Option<u32> {
    first_bit_starting_at(slice, x, true)
}

fn first_zero_starting_at(slice: &[V], x: u32) -> Option<u32> {
    first_bit_starting_at(slice, x, false)
}

fn clear_bit_range(slice: &mut [V], start: u32, end: u32) {
    let i = start / V::BITS;
    let f = start % V::BITS;
    let j = end / V::BITS;

    // Start and end are in the same chunk.
    if i == j {
        let mask1 = !((1 << f) - 1);
        let end_f = end % V::BITS;
        let mask2 = (1 << end_f) - 1;
        let clear = mask1 & mask2;

        slice[i as usize] &= !clear;
        return;
    }

    // Always do this even if f is zero (no mask).
    let clear = !((1 << f) - 1);
    slice[i as usize] &= !clear;
    let i = i + 1;

    if i as usize >= slice.len() {
        return;
    }
    for v in &mut slice[i as usize..j as usize] {
        *v = 0 as V;
    }

    let f = end % V::BITS;
    if f != 0 {
        let clear = (1 << f) - 1;
        slice[j as usize] &= !clear;
    }
}
