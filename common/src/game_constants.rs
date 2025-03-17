// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: LGPL-3.0-or-later

use crate::{DomainName, GameId, SceneId, ServerId, ServerKind, ServerNumber};
use std::fmt::Write;
use std::ops::Deref;

pub struct GameConstants {
    /// The game domain.  For example, "foobar.com".
    pub domain: &'static str,
    /// The game ID.  For example, "FooBar".
    pub game_id: &'static str,
    /// Whether the game wants geodns to route players to the closest server right away.
    ///
    /// Costs more money. Not defaulted since choice must be intentional and accurate.
    pub geodns_enabled: bool,
    /// The game name.  For example, "Foo Bar" or "FooBar.com".
    pub name: &'static str,
    /// The trademark.  For example, "FooBar".
    pub trademark: &'static str,
    /// The server names.  For example, "Jupiter", "Mars", and "Venus".
    /// TODO: Move to client.
    pub server_names: &'static [&'static str],
    //pub rank_names: &'static [&'static str],
    pub defaulted: DefaultedGameConstants,
}

impl Deref for GameConstants {
    type Target = DefaultedGameConstants;

    fn deref(&self) -> &Self::Target {
        &self.defaulted
    }
}

/// More game constants, that have defaults.
pub struct DefaultedGameConstants {
    /// Whether the game wants UDP allowed by the firewall.
    pub udp_enabled: bool,
    /// Minimum bots in a temporary server.
    pub min_temporary_server_bots: u16,
    /// Maximum bots in a temporary server.
    pub max_temporary_server_bots: u16,
    /// Default bots in a temporary server.
    pub default_temporary_server_bots: u16,
}

impl DefaultedGameConstants {
    /// Can't use the `Default` trait since that is not `const`.
    pub const fn new() -> Self {
        Self {
            udp_enabled: false,
            min_temporary_server_bots: 0,
            max_temporary_server_bots: 8,
            default_temporary_server_bots: 4,
        }
    }

    pub const fn udp_enabled(mut self) -> Self {
        self.udp_enabled = true;
        self
    }

    pub const fn min_temporary_server_bots(mut self, val: u16) -> Self {
        self.min_temporary_server_bots = val;
        self
    }

    pub const fn max_temporary_server_bots(mut self, val: u16) -> Self {
        self.max_temporary_server_bots = val;
        self
    }

    pub const fn default_temporary_server_bots(mut self, val: u16) -> Self {
        self.default_temporary_server_bots = val;
        self
    }
}

impl GameConstants {
    pub fn domain_name(&self) -> DomainName {
        DomainName::new(&self.domain).unwrap()
    }

    pub fn game_id(&self) -> GameId {
        GameId::new(&self.game_id)
    }

    /// The hostname that corresponds with the specified server number, e.g. "0.foo.com".
    pub fn hostname(&self, server_id: ServerId) -> String {
        match server_id.kind {
            ServerKind::Cloud => format!("{}.{}", server_id.number, self.domain),
            ServerKind::Local => "localhost:8443".into(),
        }
    }

    /// The in-game server name that corresponds with the specified server number, e.g. "Jupiter".
    pub fn server_name(&self, server_number: ServerNumber) -> &'static str {
        self.server_names[server_number.0.get() as usize % self.server_names.len()]
    }

    /// The tier name that corresponds with the specified server number and scene ID, e.g. "Jupiter/A".
    pub fn tier_name(&self, server_number: ServerNumber, scene_id: SceneId) -> String {
        let server_name = self.server_name(server_number);
        let mut ret = String::with_capacity(24);
        write!(&mut ret, "{server_name}/").unwrap();
        if let Some(tier_number) = scene_id.tier_number {
            write!(&mut ret, "{tier_number}").unwrap();
        } else {
            write!(&mut ret, "entry").unwrap();
        }
        if scene_id.instance_number.0 != 0 {
            write!(&mut ret, "{}", scene_id.instance_number).unwrap();
        }
        ret
    }
}

impl PartialEq for GameConstants {
    fn eq(&self, _: &Self) -> bool {
        true
    }
}
