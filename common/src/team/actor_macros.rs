// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: LGPL-3.0-or-later

#[macro_export]
macro_rules! define_player {
    () => {
        impl kodiak_common::actor_model::Message for PlayerEdit {}
        use crate::world::Apply;
        use kodiak_common::bitcode::{self, *};
        use kodiak_common::{JoinUpdate, PlayerId, PlayerStatus, TeamId};

        #[derive(Clone, Debug, Default, PartialEq, Hash, Encode, Decode)]
        pub struct Player {
            pub status: PlayerStatus,
        }

        impl Player {
            pub fn update(&mut self, update: &JoinUpdate) {
                self.status.player_update(update);
            }
        }

        impl kodiak_common::actor_model::Actor for Player {
            type Id = PlayerId;

            const KEEPALIVE: u8 = 1;
        }

        #[derive(Clone, Debug, PartialEq, Hash, Encode, Decode)]
        pub enum PlayerEdit {
            Update(JoinUpdate),
        }

        impl<C: ?Sized> Apply<PlayerEdit, C> for Player {
            fn apply(&mut self, edit: &PlayerEdit, _: &mut C) {
                match edit {
                    PlayerEdit::Update(update) => self.update(update),
                }
            }
        }
    };
}

#[macro_export]
macro_rules! define_team {
    ($data:ident, $manifestation:ident) => {
        pub type Team = kodiak_common::Team<$data, $manifestation>;
        pub type Members = kodiak_common::Members<$manifestation>;
        pub type Member = kodiak_common::Member<$manifestation>;

        impl kodiak_common::actor_model::Actor for Team {
            type Id = kodiak_common::TeamId;

            /// Helps stabilize the team menu.
            const KEEPALIVE: u8 = 30;
        }
    };
}
