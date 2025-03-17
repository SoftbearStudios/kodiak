// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: LGPL-3.0-or-later

use super::MessageAttribution;
use crate::bitcode::{self, *};
use crate::{Dedup, MessageDto, MessageNumber};
use kodiak_common::arrayvec::ArrayVec;
use std::sync::Arc;

#[derive(Clone, Default, Debug, Encode, Decode)]
pub struct ChatInbox {
    /// sender_ip = slots[message_number % slots.len()]
    slots: ArrayVec<ChatInboxSlot, 10>,
    next_message_number: u8,
}

#[derive(Clone, Debug, Encode, Decode)]
pub struct ChatInboxSlot {
    message: Arc<MessageDto>,
    attribution: Option<MessageAttribution>,
    // Once sent.
    message_number: Option<MessageNumber>,
}

impl ChatInbox {
    pub fn write(&mut self, message: Arc<MessageDto>, attribution: Option<MessageAttribution>) {
        if self.slots.is_full() {
            self.slots.remove(0);
        }
        self.slots.push(ChatInboxSlot {
            message,
            attribution,
            message_number: None,
        });
    }

    pub fn read_is_empty(&self) -> bool {
        self.slots.iter().all(|s| s.message_number.is_some())
    }

    pub fn read(&mut self) -> Box<[(MessageNumber, Dedup<MessageDto>)]> {
        self.slots
            .iter_mut()
            .filter_map(|slot| {
                if slot.message_number.is_some() {
                    // Already sent.
                    return None;
                }

                let ret = Some((self.next_message_number, Arc::clone(&slot.message)));
                slot.message_number = Some(self.next_message_number);
                self.next_message_number = self.next_message_number.wrapping_add(1);
                ret
            })
            .collect()
    }

    pub fn mark_unread(&mut self) {
        self.slots.iter_mut().for_each(|slot| {
            slot.message_number = None;
        });
    }

    pub fn attribute(&self, message_number: MessageNumber) -> Option<MessageAttribution> {
        self.slots
            .iter()
            .find(|s| s.message_number == Some(message_number))
            .and_then(|s| s.attribution)
    }
}
