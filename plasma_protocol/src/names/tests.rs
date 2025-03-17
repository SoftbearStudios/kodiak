// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: LGPL-3.0-or-later

#[cfg(test)]
mod tests {
    use crate::Referrer;
    use std::str::FromStr;

    #[test]
    fn test_referrer_crazygames() {
        assert_eq!(
            Referrer::from_hostname("crazygames.mk48.io", "mk48.io"),
            Some(Referrer::from_str("crazygames").unwrap())
        );
    }

    #[test]
    fn test_referrer_localhost() {
        assert_eq!(
            Referrer::from_hostname("ads.localhost:8080", "mk48.io"),
            Some(Referrer::from_str("ads").unwrap())
        );
    }

    #[test]
    fn test_referrer_other() {
        assert_eq!(&Referrer::new("http://foo.bar.com").unwrap(), "bar");
        assert_eq!(&Referrer::new("baz.xyz").unwrap(), "baz");
        assert_eq!(&Referrer::new("foo.com.uk").unwrap(), "foo");
        assert_eq!(&Referrer::new("com.uk").unwrap(), "com");
        assert_eq!(
            &Referrer::new("https://one.two.three.four/five.html").unwrap(),
            "three"
        );
        assert_eq!(Referrer::new(""), None);
    }

    #[test]
    #[cfg(feature = "server")]
    fn test_team_name() {
        use crate::name::TeamName;

        assert_eq!(TeamName::new_sanitized("1234567").as_str(), "123456");
        assert_eq!(TeamName::new_sanitized("❮✰❯").as_str(), "❮✰❯");
        assert_eq!(TeamName::new_sanitized("❮✰❯").as_str(), "❮✰❯");
        assert_eq!(TeamName::new_sanitized("[foo").as_str(), "foo");
        assert_eq!(TeamName::new_sanitized("foo]]").as_str(), "foo");
        assert_eq!(TeamName::new_sanitized("").as_str(), "");
        assert_eq!(TeamName::new_sanitized("[ ]").as_str(), "");
    }
}
