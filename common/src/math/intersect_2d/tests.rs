// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: LGPL-3.0-or-later

#[cfg(test)]
mod intersect_2d_tests {
    use crate::angle::Angle;
    use crate::collision::SatRect;
    use glam::{vec2, Vec2};
    use test::bench::{black_box, Bencher};

    #[bench]
    fn test_rotrect_bench_collides_with_false(bencher: &mut Bencher) {
        let a = SatRect::new(Vec2::ZERO, Vec2::splat(2.0), Angle::from_degrees(10.0));
        let b = SatRect::new(
            Vec2::splat(100.0),
            vec2(2.0, 1.0),
            Angle::from_degrees(-85.0),
        );

        assert!(!a.collides_with(&b));

        bencher.iter(|| black_box(black_box(&a).collides_with(black_box(&b))))
    }

    #[bench]
    fn test_rotrect_bench_collides_with_true(bencher: &mut Bencher) {
        let a = SatRect::new(Vec2::ZERO, Vec2::splat(2.0), Angle::from_degrees(10.0));
        let b = SatRect::new(Vec2::ONE, vec2(2.0, 1.0), Angle::from_degrees(-85.0));

        assert!(a.collides_with(&b));

        bencher.iter(|| black_box(black_box(&a).collides_with(black_box(&b))))
    }
}
