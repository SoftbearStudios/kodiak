// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: LGPL-3.0-or-later

#[cfg(test)]
mod arenamap_tests {
    use super::*;

    #[test]
    fn test_arena_map() {
        let mut map = ArenaMap::new();
        let id = PlayerId::from_index(0);
        let v = "foo";
        map.insert(id, v);
        assert_eq!(map[id], v);
        assert_eq!(*map.get(id).unwrap(), v);
        assert_eq!(*map.get_mut(id).unwrap(), v);
        map.remove(id);
    }

    #[test]
    #[should_panic = "already exists"]
    fn insert_twice() {
        let mut map = ArenaMap::new();
        let id = PlayerId::from_index(0);
        assert_eq!(map.len(), 0);
        map.insert(id, ());
        assert_eq!(map.len(), 1);
        map.insert(id, ());
    }

    #[test]
    #[should_panic = "doesn't exist"]
    fn remove_twice() {
        let mut map = ArenaMap::new();
        let id = PlayerId::from_index(0);
        assert_eq!(map.len(), 0);
        map.insert(id, ());
        assert_eq!(map.len(), 1);
        map.remove(id);
        assert_eq!(map.len(), 0);
        map.remove(id);
    }
}

#[cfg(test)]
mod mask_tests {
    use super::*;
    use rand::prelude::*;
    use rand_chacha::ChaCha20Rng;
    use std::collections::HashSet;
    use test::bench::{black_box, Bencher};

    const TEST_DIM: u32 = 32;

    #[test]
    fn test_first_one_starting_at() {
        let v = [0b101100 as V, 0 as V];
        assert_eq!(first_one_starting_at(&v, 0), Some(2));
        assert_eq!(first_one_starting_at(&v, 3), Some(3));
        assert_eq!(first_one_starting_at(&v, 4), Some(5));

        let v = [0 as V, 0b11110000 as V];
        assert_eq!(first_one_starting_at(&v, 0), Some(V::BITS + 4));
        assert_eq!(first_one_starting_at(&v, V::BITS), Some(V::BITS + 4));
    }

    #[test]
    fn test_first_zero_starting_at() {
        let v = [!(0b101100 as V), !(0 as V)];
        assert_eq!(first_zero_starting_at(&v, 0), Some(2));
        assert_eq!(first_zero_starting_at(&v, 3), Some(3));
        assert_eq!(first_zero_starting_at(&v, 4), Some(5));

        let v = [!(0 as V), !(0b11110000 as V)];
        assert_eq!(first_zero_starting_at(&v, 0), Some(V::BITS + 4));
        assert_eq!(first_zero_starting_at(&v, V::BITS), Some(V::BITS + 4));
    }

    #[test]
    fn test_clear_bit_range() {
        let mut a = [!(0 as V), !(0 as V)];
        clear_bit_range(&mut a, 0, 5);
        let b = [!(0b11111 as V), !(0 as V)];

        for i in 0..a.len() {
            assert!(a[i] == b[i], "not equal[{i}]\n{:0b}, {:0b}", a[i], b[i]);
        }

        if V::BITS == 64 {
            let mut a = [!(0 as V), !(0 as V), !(0 as V)];
            clear_bit_range(&mut a, 32, 32 * 5);
            let b = [u32::MAX as V, 0 as V, !(u32::MAX as V)];

            for i in 0..a.len() {
                assert!(a[i] == b[i], "not equal[{i}]\n{:0b}, {:0b}", a[i], b[i]);
            }
        }
    }

    fn random_mask(dims: UVec2, sample_percent: f32) -> Mask {
        let mut rng = ChaCha20Rng::from_seed(Default::default());
        let samples = ((dims.x * dims.y) as f32 * sample_percent) as u32;
        Mask::new(
            (0..samples).map(|_| uvec2(rng.gen_range(0..dims.x), rng.gen_range(0..dims.y))),
            dims,
        )
    }

    fn circle_mask(dim: u32) -> Mask {
        let center = dim / 2;
        let r2 = ((dim / 2) as f32).powi(2);
        Mask::new(
            (0..dim).flat_map(|y| {
                (0..dim).filter_map(move |x| {
                    let pos = uvec2(x, y);
                    ((pos.as_ivec2() - center as i32).as_vec2().length_squared() < r2)
                        .then_some(pos)
                })
            }),
            UVec2::splat(dim),
        )
    }

    fn bench_mask_into_rects(b: &mut Bencher, mask: Mask) {
        #[cfg(debug_assertions)]
        {
            let mut test = mask.clone();
            test.take_rects();
        }

        let mut copy = mask.clone();
        b.iter(|| {
            // Don't allocate.
            copy.clone_from(&mask);
            black_box(&mut copy).take_rects_with_fn(|r| {
                black_box(r);
            });
        })
    }

    #[bench]
    fn bench_mask_into_rects_full(b: &mut Bencher) {
        let mut full = Mask::empty(UVec2::splat(TEST_DIM));
        for y in 0..TEST_DIM {
            for x in 0..TEST_DIM {
                full.set(uvec2(x, y));
            }
        }
        bench_mask_into_rects(b, full);
    }

    #[bench]
    fn bench_mask_into_rects_empty(b: &mut Bencher) {
        bench_mask_into_rects(b, Mask::empty(UVec2::splat(TEST_DIM)))
    }

    #[bench]
    fn bench_mask_into_rects_random_10(b: &mut Bencher) {
        bench_mask_into_rects(b, random_mask(UVec2::splat(TEST_DIM), 0.1));
    }

    #[bench]
    fn bench_mask_into_rects_random_75(b: &mut Bencher) {
        bench_mask_into_rects(b, random_mask(UVec2::splat(TEST_DIM), 0.75));
    }

    #[bench]
    fn bench_mask_into_rects_random_500(b: &mut Bencher) {
        bench_mask_into_rects(b, random_mask(UVec2::splat(TEST_DIM), 5.0));
    }

    #[bench]
    fn bench_mask_into_rects_circle(b: &mut Bencher) {
        bench_mask_into_rects(b, circle_mask(TEST_DIM));
    }

    #[test]
    fn test_mask1() {
        let points = [uvec2(0, 0), uvec2(1, 1)];
        let dim = UVec2::splat(2);
        let kernel = 1;

        let rects = [(uvec2(0, 0), uvec2(0, 0)), (uvec2(1, 1), uvec2(1, 1))];

        let mut mask = Mask::new_expanded(points, dim, kernel);
        assert_eq!(
            format!("{mask:?}"),
            "\
            10\n\
            01\n\
            "
        );
        let res: HashSet<_> = mask.take_rects_iter().collect();
        assert_eq!(res, rects.into())
    }

    #[test]
    fn test_mask2() {
        let points = [uvec2(1, 0), uvec2(0, 1), uvec2(1, 1)];
        let dim = UVec2::splat(3);
        let kernel = 3;

        let rects = [(uvec2(0, 0), uvec2(2, 2))];

        let mut mask = Mask::new_expanded(points, dim, kernel);
        assert_eq!(
            format!("{mask:?}"),
            "\
            111\n\
            111\n\
            111\n\
            "
        );
        let res: HashSet<_> = mask.take_rects_iter().collect();
        assert_eq!(res, rects.into())
    }

    #[test]
    fn test_mask3() {
        let points = [uvec2(1, 0), uvec2(0, 1), uvec2(1, 1)];
        let dim = UVec2::splat(3);
        let kernel = 2;

        let rects = [(uvec2(1, 0), uvec2(2, 2)), (uvec2(0, 1), uvec2(0, 2))];

        let mut mask = Mask::new_expanded(points, dim, kernel);
        assert_eq!(
            format!("{mask:?}"),
            "\
            011\n\
            111\n\
            111\n\
            "
        );
        let res: HashSet<_> = mask.take_rects_iter().collect();
        assert_eq!(res, rects.into())
    }

    #[test]
    fn test_kiomet_crash() {
        #[rustfmt::skip]
        let mask = Box::from([0, 0, 0, 0, 0, 18446744073709551615, 18446744073709551615, 18446744073709551615, 18374686479671623807, 18374686479671623807, 18374686479671623807, 18374686479671623807, 18374686479671623807, 18374686479671623807, 18374686479671623807, 18374686479671623807, 18374686479671623807, 18374686479671623807, 18374686479671623807, 18374686479671623807, 18374686479671623807, 18374686479671623807, 18374686479671623807, 18374686479671623807, 18374686479671623807, 18374686479671623807, 18374686479671623807, 18374686479671623807, 18374686479671623807, 18374686479671623807, 18374686479671623807, 18374686479671623807, 18374686479671623807, 18374686479671623807, 18374686479671623807, 18374686479671623807, 18374686479671623807, 18374686479671623807, 18374686479671623807, 18374686479671623807, 18374686479671623807, 18374686479671623807, 18374686479671623807, 18374686479671623807, 18374686479671623807, 18374686479671623807, 18374686479671623807, 18374686479671623807, 18374686479671623807, 18374686479671623807, 18374686479671623807, 18374686479671623807, 18374686479671623807, 18374686479671623807, 0, 0, 0, 0, 0]);
        let dims = UVec2::new(64, 59);
        let original_x = 63;
        let mut mask = Mask {
            mask,
            dims,
            original_x,
        };

        mask.take_rects();
    }

    #[test]
    fn fuzz_x() {
        let mut rng = rand_chacha::ChaCha20Rng::from_seed(Default::default());
        for original_x in 0..256 {
            for _ in 0..10 {
                let mut mask = Mask::new(
                    (0..original_x)
                        .map(|x| UVec2 { x, y: 0 })
                        .filter(|_| rng.gen()),
                    UVec2::new(original_x, 1),
                );
                mask.take_rects();
            }
        }
    }
}
