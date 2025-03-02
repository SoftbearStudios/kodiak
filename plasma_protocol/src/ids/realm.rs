// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: LGPL-3.0-or-later

use crate::{InvalidInvitationId, InvalidRealmName, InvitationId, RealmName};
use bitcode::{Decode, Encode};
use std::fmt::{self, Debug, Display, Formatter};
use std::str::FromStr;

#[derive(Copy, Clone, Debug, Default, Eq, PartialEq, Hash, Encode, Decode, Ord, PartialOrd)]
pub enum RealmId {
    Named(RealmName),
    #[default]
    PublicDefault,
    Temporary(InvitationId),
}

#[derive(Debug, Clone)]
pub enum InvalidRealmId {
    InvalidRealmName(InvalidRealmName),
    InvalidTemporaryRealmInvitationId(InvalidInvitationId),
    MissingSlash,
    Prefix,
}

impl RealmId {
    // Deprecate eventually. For backward compatibility.
    pub fn from_opt(realm_id_opt: Option<Self>) -> Self {
        match realm_id_opt {
            None => RealmId::PublicDefault,
            Some(realm_id) => realm_id,
        }
    }

    // Deprecate eventually. For backward compatibility.
    pub fn into_opt(self) -> Option<Self> {
        match self {
            RealmId::PublicDefault => None,
            _ => Some(self),
        }
    }

    pub fn is_public_default(self) -> bool {
        matches!(self, Self::PublicDefault)
    }

    pub fn is_named(self) -> bool {
        matches!(self, Self::Named(_))
    }

    pub fn is_temporary(self) -> bool {
        matches!(self, Self::Temporary(_))
    }

    pub fn temporary(self) -> Option<InvitationId> {
        if let Self::Temporary(invitation_id) = self {
            Some(invitation_id)
        } else {
            None
        }
    }
}

impl Display for RealmId {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), fmt::Error> {
        match self {
            RealmId::PublicDefault => write!(f, "public/default"),
            RealmId::Named(realm_name) => write!(f, "named/{realm_name}"),
            RealmId::Temporary(index) => write!(f, "temporary/{index}"),
        }
    }
}

impl FromStr for RealmId {
    type Err = InvalidRealmId;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s == "public/default" {
            Ok(RealmId::PublicDefault)
        } else {
            let (prefix, value) = s.split_once('/').ok_or(InvalidRealmId::MissingSlash)?;
            match prefix {
                "named" => value
                    .parse()
                    .map(Self::Named)
                    .map_err(InvalidRealmId::InvalidRealmName),
                "temporary" => value
                    .parse()
                    .map(Self::Temporary)
                    .map_err(InvalidRealmId::InvalidTemporaryRealmInvitationId),
                _ => Err(InvalidRealmId::Prefix),
            }
        }
    }
}

mod realm_id_serde {
    use crate::RealmId;
    use serde::{Deserialize, Deserializer, Serialize, Serializer};
    use std::str::FromStr;

    impl Serialize for RealmId {
        fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: Serializer,
        {
            serializer.collect_str(self)
        }
    }

    impl<'de> Deserialize<'de> for RealmId {
        fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where
            D: Deserializer<'de>,
        {
            <String>::deserialize(deserializer).and_then(|s| {
                Self::from_str(&s).map_err(|_| serde::de::Error::custom("invalid realm id"))
            })
        }
    }
}
