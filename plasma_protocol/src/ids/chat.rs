// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: LGPL-3.0-or-later

use crate::{ArenaId, NonZeroUnixMillis, ServerId, ServerKind, ServerNumber, UnixTime};
use bitcode::{Decode, Encode};
use std::fmt::{self, Debug, Display, Formatter};
use std::num::NonZeroU8;

pub type MessageNumber = u8;

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash, Encode, Decode)]
pub struct ChatId {
    pub arena_id: ArenaId,
    pub message_id: NonZeroUnixMillis,
    pub server_id: ServerId,
}

impl ChatId {
    // For timestamp search filters.
    pub fn timestamp(message_id: NonZeroUnixMillis) -> Self {
        Self {
            arena_id: Default::default(),
            message_id,
            server_id: ServerId {
                kind: ServerKind::Cloud,
                number: ServerNumber(NonZeroU8::new(1).unwrap()),
            },
        }
    }
}

impl Display for ChatId {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let ChatId {
            arena_id,
            message_id,
            server_id,
        } = self;
        let message_id: i64 = (*message_id).to_i64();
        write!(f, "{message_id}@{server_id}/{arena_id}")
    }
}

#[derive(Debug, Clone)]
pub enum InvalidChatId {
    InvalidArenaId,
    InvalidMessageId,
    InvalidServerId,
    MissingAtSign,
    MissingSlash,
}

impl std::str::FromStr for ChatId {
    type Err = InvalidChatId;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (s_message_id, s_arena_path) = s.split_once('@').ok_or(InvalidChatId::MissingAtSign)?;
        let message_id = s_message_id
            .parse()
            .map_err(|_| InvalidChatId::InvalidMessageId)?;
        let (index, _) = s_arena_path
            .match_indices('/')
            .nth(1)
            .ok_or(InvalidChatId::MissingSlash)?;
        let (s_server_id, s_arena_id) = s_arena_path.split_at(index);
        let s_arena_id = &s_arena_id[1..];
        let server_id = s_server_id
            .parse()
            .map_err(|_| InvalidChatId::InvalidServerId)?;
        let arena_id = s_arena_id
            .parse()
            .map_err(|_| InvalidChatId::InvalidArenaId)?;
        Ok(Self {
            arena_id,
            server_id,
            message_id,
        })
    }
}

#[cfg(test)]
mod tests2 {
    use crate::ChatId;
    use std::str::FromStr;

    #[test]
    fn chat_id() {
        let example = "714974570605@Cloud/8/public/default/0";
        let _chat_id = ChatId::from_str(example).unwrap();
    }
}

mod chat_id_serde {
    use crate::ChatId;
    use serde::{Deserialize, Deserializer, Serialize, Serializer};
    use std::str::FromStr;

    impl Serialize for ChatId {
        fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: Serializer,
        {
            serializer.collect_str(self)
        }
    }

    impl<'de> Deserialize<'de> for ChatId {
        fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where
            D: Deserializer<'de>,
        {
            <String>::deserialize(deserializer).and_then(|s| {
                Self::from_str(&s).map_err(|_| serde::de::Error::custom("invalid chat id"))
            })
        }
    }
}
