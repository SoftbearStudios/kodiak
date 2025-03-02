// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: LGPL-3.0-or-later

#[cfg(test)]
mod angle_tests {
    use super::deterministic_atan2;
    use crate::angle::{Angle, Cardinal4};
    use glam::Vec2;
    use rand::distributions::Standard;
    use rand::prelude::*;
    use test::Bencher;

    #[test]
    fn det_atan2() {
        assert_eq!(deterministic_atan2(0.0, 0.0), 0f32.atan2(0.0));
        assert!(deterministic_atan2(1.0, f32::NAN).is_nan());
        assert!(deterministic_atan2(f32::NAN, 1.0).is_nan());

        let test_cases = (-1000000..=1000000)
            .step_by(10000)
            .map(|n| n as f32)
            .chain((-1000..=1000).step_by(5).map(|n| n as f32))
            .chain((-1000..=1000).map(|n| n as f32 * 0.01))
            .chain((-100..=100).map(|n| n as f32 * 0.0001))
            .chain((-100..=100).map(|n| n as f32 * 0.000001));
        for x in test_cases.clone() {
            for y in test_cases.clone() {
                let standard = y.atan2(x);
                let deterministic = deterministic_atan2(y, x);
                assert!(
                    (standard - deterministic).abs() < 0.001,
                    "atan2({x}, {y}) = std: {standard} det: {deterministic}"
                );
            }
        }
    }

    fn dataset<T>() -> Vec<T>
    where
        Standard: Distribution<T>,
    {
        let mut rng = rand_chacha::ChaCha20Rng::from_seed(Default::default());
        (0..1000).map(|_| rng.gen()).collect()
    }

    #[bench]
    fn bench_to_vec(b: &mut Bencher) {
        let dataset = dataset::<Angle>();
        b.iter(|| {
            let mut sum = Vec2::ZERO;
            for a in dataset.as_slice() {
                sum += a.to_vec();
            }
            sum
        })
    }

    #[bench]
    fn bench_atan2(b: &mut Bencher) {
        let dataset = dataset::<Vec2>();
        b.iter(|| {
            let mut sum = 0.0;
            for v in dataset.as_slice() {
                sum += f32::atan2(v.x, v.y)
            }
            sum
        })
    }

    #[bench]
    fn bench_deterministic_atan2(b: &mut Bencher) {
        let dataset = dataset::<Vec2>();
        b.iter(|| {
            let mut sum = 0.0;
            for v in dataset.as_slice() {
                sum += deterministic_atan2(v.x, v.y)
            }
            sum
        })
    }

    #[test]
    fn radians() {
        for i in -1000..1000 {
            let r = (i as f32) / 100.0;
            let a = Angle::from_radians(r);
            let r2 = a.to_radians();
            let a2 = Angle::from_radians(r2);
            assert!((a - a2).to_radians().abs() < 0.0001, "{:?} -> {:?}", a, a2);
        }
    }

    #[test]
    fn serde() {
        for i in -1000..1000 {
            let r = (i as f32) / 100.0;
            let rs = format!("{}", r);
            let a: Angle = serde_json::from_str(&rs).unwrap();
            let rs2 = serde_json::to_string(&a).unwrap();
            let a2: Angle = serde_json::from_str(&rs2).unwrap();
            assert!((a - a2).to_radians().abs() < 0.0001, "{:?} -> {:?}", a, a2);
        }
    }

    #[test]
    fn pi() {
        // Just less than PI.
        let rs = "3.141592653589793";
        let a: Angle = serde_json::from_str(rs).unwrap();
        assert_eq!(a, Angle::PI);

        // Greater than PI.
        let rs2 = "3.141689";
        let a2: Angle = serde_json::from_str(rs2).unwrap();
        assert!(a2.to_radians() < -3.0, "{a2:?}");
    }

    #[test]
    fn unit_vec() {
        let v = Angle::ZERO.to_vec();
        assert!(v.abs_diff_eq(Vec2::X, 0.0001), "{v:?}");

        let v = Angle::PI_2.to_vec();
        assert!(v.abs_diff_eq(Vec2::Y, 0.0001), "{v:?}");
    }

    #[test]
    fn abs() {
        assert_eq!(Angle::from_radians(0.0).abs(), Angle::from_radians(0.0));
        assert_eq!(Angle::from_radians(0.5).abs(), Angle::from_radians(0.5));
        assert_eq!(Angle::from_radians(-0.5).abs(), Angle::from_radians(0.5));
    }

    #[test]
    fn min() {
        assert_eq!(
            Angle::from_radians(0.5).min(Angle::from_radians(0.6)),
            Angle::from_radians(0.5)
        );
        assert_eq!(
            Angle::from_radians(0.5).min(Angle::from_radians(0.4)),
            Angle::from_radians(0.4)
        );
        assert_eq!(
            Angle::from_radians(-0.5).min(Angle::from_radians(0.6)),
            Angle::from_radians(-0.5)
        );
        assert_eq!(
            Angle::from_radians(-0.5).min(Angle::from_radians(0.4)),
            Angle::from_radians(-0.5)
        );
    }

    #[test]
    fn clamp_magnitude() {
        assert_eq!(
            Angle::from_radians(0.5).clamp_magnitude(Angle::from_radians(0.6)),
            Angle::from_radians(0.5)
        );
        assert_eq!(
            Angle::from_radians(0.5).clamp_magnitude(Angle::from_radians(0.4)),
            Angle::from_radians(0.4)
        );
        assert_eq!(
            Angle::from_radians(-0.5).clamp_magnitude(Angle::from_radians(0.6)),
            Angle::from_radians(-0.5)
        );
        assert_eq!(
            Angle::from_radians(-0.5).clamp_magnitude(Angle::from_radians(0.4)),
            Angle::from_radians(-0.4)
        );
    }

    #[test]
    fn to_bearing() {
        assert_eq!(Angle::PI_2.to_bearing(), 0);
        assert_eq!(Angle::PI.to_bearing(), 270);

        for i in 0..i16::MAX {
            let b = Angle(i).to_bearing();
            assert!(b < 360, "{} -> {} >= 360", i, b);
        }
    }

    #[test]
    fn to_cardinal() {
        // Make sure it doesn't panic.
        for i in 0..=i16::MAX {
            Angle(i).to_cardinal();
        }

        assert_eq!(Angle::ZERO.to_cardinal(), "E");
        assert_eq!(Angle::PI_2.to_cardinal(), "N");
        assert_eq!(Angle::PI.to_cardinal(), "W");
        assert_eq!(Angle(u16::MAX as i16).to_cardinal(), "E");
    }

    #[test]
    fn to_cardinal_4() {
        // Make sure it doesn't panic.
        for i in 0..=i16::MAX {
            Angle(i).to_cardinal_4();
        }

        assert_eq!(Angle::ZERO.to_cardinal_4(), Cardinal4::East);
        assert_eq!(Angle::PI_2.to_cardinal_4(), Cardinal4::North);
        assert_eq!(Angle::PI.to_cardinal_4(), Cardinal4::West);
        assert_eq!((-Angle::PI_2).to_cardinal_4(), Cardinal4::South);
        assert_eq!(Angle(u16::MAX as i16).to_cardinal_4(), Cardinal4::East);
    }

    #[test]
    fn saturating_from_radians() {
        let a = Angle::saturating_from_radians(1000.0);
        let b = Angle::PI;
        assert_eq!(a, b);

        let a = Angle::saturating_from_radians(-1000.0);
        let b = Angle::MIN;
        assert_eq!(a, b);
    }
}

#[cfg(test)]
mod range_tests {
    use super::{map_ranges, map_ranges_fast};

    #[test]
    fn test_map_range() {
        assert_eq!(map_ranges(1.5, 1.0..2.0, -4.0..-8.0, false), -6.0);
        assert_eq!(map_ranges(1.5, 1.0..2.0, -4.0..-8.0, true), -6.0);
        assert_eq!(map_ranges(1.5, 2.0..1.0, -8.0..-4.0, false), -6.0);
        assert_eq!(map_ranges(1.5, 2.0..1.0, -8.0..-4.0, true), -6.0);
        assert_eq!(map_ranges(10.0, 0.0..1.0, 2.0..3.0, true), 3.0);
        assert_eq!(map_ranges(10.0, 1.0..0.0, 3.0..2.0, true), 3.0);
    }

    #[test]
    fn test_map_ranges_fast() {
        assert_eq!(
            map_ranges_fast(1.5, 1.0..2.0, -4.0..-8.0, false, false),
            -6.0
        );
        assert_eq!(map_ranges_fast(10.0, 0.0..1.0, 2.0..3.0, true, true), 3.0);
    }
}
