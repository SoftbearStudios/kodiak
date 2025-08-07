// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: LGPL-3.0-or-later

#[cfg(feature = "server")]
mod _trait;
// define_player!, define_team!
mod actor_macros;
mod color;
mod id_or_alias;
mod joined_status;
mod members;
mod team;

#[cfg(feature = "server")]
pub use self::_trait::PlayerTeamModel;
pub use self::color::{InvalidTeamColor, TeamColor};
pub use self::id_or_alias::PlayerIdOrAlias;
pub use self::joined_status::{JoinUpdate, JoinedStatus, PlayerStatus};
pub use self::members::Members;
#[cfg(feature = "server")]
pub use self::team::random_bot_team_name;
pub use self::team::{allocate_team_id, Manifestation, Member, MemberId, Team};
