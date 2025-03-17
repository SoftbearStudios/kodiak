// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: LGPL-3.0-or-later

use crate::bitcode::{self, *};
use crate::{PlayerAlias, PlayerId};

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash, Encode, Decode)]
pub enum PlayerIdOrAlias {
    Id(PlayerId),
    Alias(PlayerAlias),
}

impl PlayerIdOrAlias {
    pub fn alias(&self) -> Option<PlayerAlias> {
        if let Self::Alias(alias) = self {
            Some(*alias)
        } else {
            None
        }
    }

    pub fn id(&self) -> Option<PlayerId> {
        if let Self::Id(id) = self {
            Some(*id)
        } else {
            None
        }
    }

    pub fn is_alias(&self) -> bool {
        matches!(self, Self::Alias(_))
    }

    pub fn is_id(&self) -> bool {
        matches!(self, Self::Id(_))
    }
}
