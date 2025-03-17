// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: LGPL-3.0-or-later

use super::{InstanceNumber, InvitationId, PlayerId, ReconnectionToken, TierNumber};
use crate::{
    impl_wrapper_display, impl_wrapper_from_str, InvalidRealmId, InvalidSceneId, RealmId, SceneId,
};
use bitcode::{Decode, Encode};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt::{self, Debug, Display, Formatter, Write};
use std::num::NonZeroU32;
use std::str::FromStr;

#[derive(Copy, Clone, Default, Eq, PartialEq, Hash, Encode, Decode)]
pub struct ArenaId {
    pub realm_id: RealmId,
    pub scene_id: SceneId,
}

impl ArenaId {
    pub fn new(realm_id: RealmId, scene_id: SceneId) -> Self {
        Self { realm_id, scene_id }
    }
}

mod arena_id_serde {
    use super::*;
    use serde::{Deserialize, Serialize};

    #[derive(Serialize, Deserialize)]
    struct ArenaIdPlaceholder {
        pub realm_id: RealmId,
        pub scene_id: SceneId,
    }

    impl Serialize for ArenaId {
        fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: Serializer,
        {
            if serializer.is_human_readable() {
                serializer.collect_str(self)
            } else {
                ArenaIdPlaceholder {
                    realm_id: self.realm_id,
                    scene_id: self.scene_id,
                }
                .serialize(serializer)
            }
        }
    }

    impl<'de> Deserialize<'de> for ArenaId {
        fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where
            D: Deserializer<'de>,
        {
            if deserializer.is_human_readable() {
                <String>::deserialize(deserializer).and_then(|s| {
                    Self::from_str(&s).map_err(|_| serde::de::Error::custom("invalid arena id"))
                })
            } else {
                ArenaIdPlaceholder::deserialize(deserializer).map(|placeholder| ArenaId {
                    realm_id: placeholder.realm_id,
                    scene_id: placeholder.scene_id,
                })
            }
        }
    }
}

impl Display for ArenaId {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), fmt::Error> {
        Display::fmt(&self.realm_id, f)?;
        f.write_char('/')?;
        Display::fmt(&self.scene_id, f)
    }
}

impl Debug for ArenaId {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), fmt::Error> {
        Display::fmt(self, f)
    }
}

#[derive(Debug, Clone)]
pub enum InvalidArenaId {
    InvalidRealmId(InvalidRealmId),
    InvalidSceneId(InvalidSceneId),
    MissingSlash,
}

impl Display for InvalidArenaId {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}

impl std::error::Error for InvalidArenaId {}

impl FromStr for ArenaId {
    type Err = InvalidArenaId;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (realm_id, scene_id) = s.rsplit_once('/').ok_or(InvalidArenaId::MissingSlash)?;
        // Below, "www" is the legacy notation; eventually, it can be removed.
        let realm_id = RealmId::from_str(realm_id).map_err(InvalidArenaId::InvalidRealmId)?;
        let scene_id = SceneId::from_str(scene_id).map_err(InvalidArenaId::InvalidSceneId)?;
        Ok(Self { realm_id, scene_id })
    }
}

#[derive(Copy, Clone, Eq, PartialEq, Encode, Decode)]
pub enum ArenaQuery {
    /// public/default/A0
    /// public/default/A0/123/315313563531563
    Specific(ArenaId, Option<(PlayerId, ReconnectionToken)>),
    /// public/default/A?
    AnyInstance(RealmId, Option<TierNumber>),
    /// temporary
    NewTemporary,
    /// invite/FFFFFF
    Invitation(InvitationId),
}

impl Default for ArenaQuery {
    fn default() -> Self {
        Self::AnyInstance(RealmId::default(), None)
    }
}

impl ArenaQuery {
    pub fn player_id(self) -> Option<PlayerId> {
        if let Self::Specific(_, Some((player_id, _))) = self {
            Some(player_id)
        } else {
            None
        }
    }

    pub fn specific(self) -> Option<ArenaId> {
        if let Self::Specific(arena_id, _) = self {
            Some(arena_id)
        } else {
            None
        }
    }

    pub fn is_specific(self) -> bool {
        self.specific().is_some()
    }

    pub fn player_id_reconnection_token(self) -> Option<(PlayerId, ReconnectionToken)> {
        if let Self::Specific(_, player_id_reconnection_token) = self {
            player_id_reconnection_token
        } else {
            None
        }
    }

    pub fn any_instance(self) -> Option<(RealmId, Option<TierNumber>)> {
        if let Self::AnyInstance(realm_id, tier_number) = self {
            Some((realm_id, tier_number))
        } else {
            None
        }
    }

    pub fn is_any_instance(self) -> bool {
        self.any_instance().is_some()
    }

    pub fn invitation_id(self) -> Option<InvitationId> {
        if let Self::Invitation(invitation_id) = self {
            Some(invitation_id)
        } else {
            None
        }
    }

    pub fn is_invitation_id(self) -> bool {
        self.invitation_id().is_some()
    }

    pub fn is_new_temporary(self) -> bool {
        matches!(self, Self::NewTemporary)
    }

    pub fn realm_id(self) -> Option<RealmId> {
        match self {
            Self::Specific(arena_id, _) => Some(arena_id.realm_id),
            Self::AnyInstance(realm_id, _) => Some(realm_id),
            _ => None,
        }
    }
}

mod arena_query_serde {
    use crate::TierNumber;

    use super::*;
    use serde::{Deserialize, Serialize};

    #[derive(Serialize, Deserialize)]
    enum SymbolicArenaIdPlaceholder {
        Specific(ArenaId, Option<(PlayerId, ReconnectionToken)>),
        AnyInstance(RealmId, Option<TierNumber>),
        NewTemporary,
        Invitation(InvitationId),
    }

    impl Serialize for ArenaQuery {
        fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: Serializer,
        {
            if serializer.is_human_readable() {
                serializer.collect_str(self)
            } else {
                match *self {
                    Self::Specific(arena_id, player_id_reconnection_token) => {
                        SymbolicArenaIdPlaceholder::Specific(arena_id, player_id_reconnection_token)
                    }
                    Self::AnyInstance(realm_id, tier_number) => {
                        SymbolicArenaIdPlaceholder::AnyInstance(realm_id, tier_number)
                    }
                    Self::NewTemporary => SymbolicArenaIdPlaceholder::NewTemporary,
                    Self::Invitation(invitation_id) => {
                        SymbolicArenaIdPlaceholder::Invitation(invitation_id)
                    }
                }
                .serialize(serializer)
            }
        }
    }

    impl<'de> Deserialize<'de> for ArenaQuery {
        fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where
            D: Deserializer<'de>,
        {
            if deserializer.is_human_readable() {
                <String>::deserialize(deserializer).and_then(|s| {
                    Self::from_str(&s)
                        .map_err(|_| serde::de::Error::custom("invalid symbolic arena id"))
                })
            } else {
                SymbolicArenaIdPlaceholder::deserialize(deserializer).map(|placeholder| {
                    match placeholder {
                        SymbolicArenaIdPlaceholder::Specific(
                            arena_id,
                            player_id_reconnection_token,
                        ) => Self::Specific(arena_id, player_id_reconnection_token),
                        SymbolicArenaIdPlaceholder::AnyInstance(realm_id, tier_number) => {
                            Self::AnyInstance(realm_id, tier_number)
                        }
                        SymbolicArenaIdPlaceholder::NewTemporary => Self::NewTemporary,
                        SymbolicArenaIdPlaceholder::Invitation(invitation_id) => {
                            Self::Invitation(invitation_id)
                        }
                    }
                })
            }
        }
    }
}

impl Display for ArenaQuery {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), fmt::Error> {
        match *self {
            Self::Specific(arena_id, player_id_reconnection_token) => {
                Display::fmt(&arena_id, f)?;
                if let Some((player_id, reconnection_token)) = player_id_reconnection_token {
                    f.write_char('/')?;
                    Display::fmt(&player_id, f)?;
                    f.write_char('/')?;
                    Display::fmt(&reconnection_token, f)?;
                }
                Ok(())
            }
            Self::AnyInstance(realm_id, tier_number) => {
                Display::fmt(
                    &ArenaId::new(realm_id, SceneId::new(tier_number, InstanceNumber::new(0))),
                    f,
                )?;
                f.write_char('?')
            }
            Self::NewTemporary => f.write_str("temporary"),
            Self::Invitation(invitation_id) => write!(f, "invite/{invitation_id}"),
        }
    }
}

impl Debug for ArenaQuery {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), fmt::Error> {
        Display::fmt(self, f)
    }
}

impl FromStr for ArenaQuery {
    type Err = InvalidArenaId;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s == "temporary" {
            Ok(Self::NewTemporary)
        } else if let Some(arena_id) = s.strip_suffix('?') {
            let arena_id = ArenaId::from_str(arena_id)?;
            if arena_id.scene_id.instance_number != InstanceNumber::new(0) {
                return Err(InvalidArenaId::InvalidSceneId(
                    InvalidSceneId::InvalidInstanceNumber,
                ));
            }
            Ok(Self::AnyInstance(
                arena_id.realm_id,
                arena_id.scene_id.tier_number,
            ))
        } else if let Some(invitation_id) = s.strip_prefix("invite/") {
            InvitationId::from_str(invitation_id)
                .map(Self::Invitation)
                .map_err(|_| InvalidArenaId::InvalidRealmId(InvalidRealmId::Prefix))
        } else if let Some((rest, reconnection_token)) = s.rsplit_once('/')
            && let Ok(reconnection_token) = ReconnectionToken::from_str(reconnection_token)
            && let Some((rest, player_id)) = rest.rsplit_once('/')
            && let Ok(player_id) = PlayerId::from_str(player_id)
            && let Ok(arena_id) = ArenaId::from_str(rest)
        {
            Ok(Self::Specific(
                arena_id,
                Some((player_id, reconnection_token)),
            ))
        } else {
            ArenaId::from_str(s).map(|arena_id| Self::Specific(arena_id, None))
        }
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash, Serialize, Deserialize, Encode, Decode)]
pub struct ArenaToken(pub NonZeroU32);
impl_wrapper_display!(ArenaToken);
impl_wrapper_from_str!(ArenaToken, NonZeroU32);
