// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: LGPL-3.0-or-later

use super::{Manifestation, Member};
use crate::bitcode::{self, *};
use crate::PlayerId;

#[derive(Clone, Debug, PartialEq, Hash, Encode, Decode)]
pub struct Members<M>(pub Vec<Member<M>>);

impl<M> Default for Members<M> {
    fn default() -> Self {
        Self(Default::default())
    }
}

impl<M: Manifestation> Members<M> {
    pub fn contains(&self, player_id: PlayerId) -> bool {
        self.iter().any(|m| m.player_id == player_id)
    }

    pub fn iter(&self) -> impl Iterator<Item = &Member<M>> {
        self.0.iter()
    }

    pub fn iter_mut(&mut self) -> impl Iterator<Item = &mut Member<M>> {
        self.0.iter_mut()
    }

    pub fn is_empty(&self) -> bool {
        self.iter().next().is_none()
    }

    pub fn len(&self) -> usize {
        self.iter().count()
    }

    pub fn push(&mut self, member: Member<M>) {
        assert!(self.0.iter().all(|m| m.player_id != member.player_id));
        self.0.push(member);
    }

    pub fn remove(&mut self, player_id: PlayerId) {
        self.0.remove(
            self.0
                .iter()
                .position(|m| m.player_id == player_id)
                .unwrap(),
        );
    }
}
