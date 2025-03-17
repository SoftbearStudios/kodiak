// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: LGPL-3.0-or-later

// Contains actix_response!
mod actix_macro;
#[cfg(feature = "admin")]
mod admin;
mod compression;
mod fence;
mod hash;
mod invitations;
mod leaderboard;
mod owned;
mod system;
mod teams;
mod tests;
mod updates;

// Contains much use of conditional compilation.
#[cfg(feature = "admin")]
pub use admin::*;
// Contains much use of conditional compilation.
pub use self::compression::*;
pub use self::fence::GameFence;
pub use self::hash::{hash_f32, hash_f32_ref, hash_f32s, CompatHasher, Hashable, HbHash};
pub use self::invitations::{
    DeepConnect, DeepConnectError, InstancePickerDto, InvitationDto, InvitationRequest,
    InvitationUpdate, SystemUpdate,
};
pub use self::leaderboard::{
    LeaderboardCaveat, LeaderboardUpdate, LiveboardDto, LiveboardUpdate, YourScoreDto,
};
pub use self::owned::{dedup_into_inner, owned_into_box, owned_into_iter, Dedup, Owned};
pub use self::system::{
    ArenaSettingsDto, EngineArenaSettings, NoGameArenaSettings, SocketQuery, SystemQuery,
    SystemResponse, TranslationRequest, TranslationResponse,
};
pub use self::teams::{TeamRequest, TeamUpdate};
pub use self::updates::{
    ChatRequest, ChatUpdate, ClientRequest, ClientUpdate, CommonRequest, CommonUpdate, MessageDto,
    PlayerDto, PlayerUpdate,
};
