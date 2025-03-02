// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: LGPL-3.0-or-later

use crate::bitcode::{self, Decode, Encode};
use crate::{InvitationId, Owned, PlayerId, RealmId, RegionId, SceneId, ServerId};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::cmp::Ordering;
use std::fmt::{self, Display, Formatter};
use std::str::FromStr;

/// Deep connect query.
#[derive(Clone, Copy, Debug)]
pub enum DeepConnect {
    Invitation(InvitationId),
    Realm(RealmId),
}

impl DeepConnect {
    /// Note: Also includes temporary realms.
    pub fn invitation_id(&self) -> Option<InvitationId> {
        if let Self::Invitation(invitation_id) | Self::Realm(RealmId::Temporary(invitation_id)) =
            self
        {
            Some(*invitation_id)
        } else {
            None
        }
    }

    pub fn realm_id(&self) -> Option<RealmId> {
        if let Self::Realm(realm_id) = self {
            Some(*realm_id)
        } else {
            None
        }
    }
}

#[derive(Debug)]
pub enum DeepConnectError {
    InvalidInvitation,
    InvalidPrefix,
    InvalidRealm,
    MissingSlash,
}

impl FromStr for DeepConnect {
    type Err = DeepConnectError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (prefix, arg) = s.split_once('/').ok_or(DeepConnectError::MissingSlash)?;
        match prefix {
            "invitation" => Ok(DeepConnect::Invitation(
                InvitationId::from_str(arg).map_err(|_| DeepConnectError::InvalidInvitation)?,
            )),
            "realm" => Ok(DeepConnect::Realm(
                RealmId::from_str(arg).map_err(|_| DeepConnectError::InvalidRealm)?,
            )),
            _ => Err(DeepConnectError::InvalidPrefix),
        }
    }
}

impl Display for DeepConnect {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), fmt::Error> {
        write!(
            f,
            "{}",
            match self {
                DeepConnect::Invitation(invitation_id) => format!("invitation/{invitation_id}"),
                DeepConnect::Realm(realm_name) => format!("realm/{realm_name}"),
            },
        )
    }
}

impl Serialize for DeepConnect {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for DeepConnect {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        <String>::deserialize(deserializer).and_then(|s| {
            Self::from_str(&s).map_err(|_| serde::de::Error::custom("invalid deep connect "))
        })
    }
}

#[derive(Copy, Clone, Debug, Serialize, Deserialize, PartialEq, Encode, Decode)]
pub struct InvitationDto {
    pub invitation_id: InvitationId,
    /// Who sent it.
    pub player_id: PlayerId,
}

/// The Instance Picker Data Transfer Object (DTO) is used to populate the server/instance picker.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, Encode, Decode)]
pub struct InstancePickerDto {
    // The field order below is used for sorting.
    pub server_id: ServerId,
    pub scene_id: SceneId,
    pub region_id: RegionId,
    /// Last self-reported player count.
    pub player_count: u16,
    /// As opposed to omitting closing instances from the instance picker, including unsanctioned instances
    /// allows the client to easily show the player's current instance/player count despite it closing.
    pub sanctioned: bool,
}

impl PartialOrd for InstancePickerDto {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for InstancePickerDto {
    fn cmp(&self, other: &Self) -> Ordering {
        self.server_id.cmp(&other.server_id)
    }
}

/// Invitation related request from client to server.
#[derive(Clone, Debug, Serialize, Deserialize, Encode, Decode)]
pub enum InvitationRequest {
    Create,
    /// Accept an invitation (or clear previously accepted invitation).
    Accept(Option<InvitationId>),
}

/// Invitation related update from server to client.
#[derive(Clone, Debug, Serialize, Deserialize, Encode, Decode)]
pub enum InvitationUpdate {
    Created(InvitationId),
    Accepted(Option<InvitationId>),
}

/// Update from game server to client to populate instance picker.
#[derive(Clone, Debug, Encode, Decode)]
pub enum SystemUpdate {
    Added(Owned<[InstancePickerDto]>),
    Removed(Owned<[(ServerId, SceneId)]>),
}
