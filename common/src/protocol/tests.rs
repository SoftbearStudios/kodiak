// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: LGPL-3.0-or-later

#[cfg(test)]
mod hash_tests {
    use crate::hash::CompatHasher;
    use std::hash::Hasher;

    #[test]
    fn compat_hasher() {
        const N: u32 = 0x01000193;

        let mut a = CompatHasher::default();
        let mut b = CompatHasher::default();
        a.write_usize(N as usize);
        b.write_u32(N);
        assert_eq!(a.finish(), b.finish());
    }
}
