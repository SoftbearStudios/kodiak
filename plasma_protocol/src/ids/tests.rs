// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: LGPL-3.0-or-later

#[cfg(test)]
mod tests {
    use crate::{InvitationId, PlayerId, ServerNumber};
    use std::str::FromStr;

    /*#[test]
    #[cfg(feature = "server")]
    fn invitation_id() {
        use crate::id::{InvitationId, ServerId};
        use std::num::NonZeroU8;

        for i in 1..=u8::MAX {
            let sid = ServerId(NonZeroU8::new(i).unwrap());
            let iid = InvitationId::generate(Some(sid));
            assert_eq!(iid.server_id(), Some(sid));
        }
    }*/

    #[test]
    fn invite_encode_decode() {
        for n in [1, 123, 12345] {
            let id = InvitationId {
                server_number: ServerNumber::new(n as u8 % 10).unwrap(),
                number: n,
            };
            let code = id.to_string();
            println!("invite {n} -> {}", code);
            assert_eq!(id, InvitationId::from_str(&code).unwrap());
        }
    }

    #[test]
    fn player_id() {
        for i in 0..u16::MAX as usize * 2 {
            if let Some(bot) = PlayerId::nth_bot(i) {
                assert!(bot.is_bot());
            }
            if let Some(bot) = PlayerId::nth_client(i) {
                assert!(bot.is_client());
            }
        }
    }
}
