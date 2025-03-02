// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: LGPL-3.0-or-later

use crate::{ArenaId, PlayerId};
use actix::prelude::*;
use tokio::sync::mpsc::UnboundedSender;

#[derive(Message)]
#[rtype(result = "()")]
pub struct ObserverMessage<I, O>
where
    O: actix::Message + std::marker::Send,
    <O as actix::Message>::Result: std::marker::Send,
{
    pub arena_id: ArenaId,
    pub body: ObserverMessageBody<I, O>,
}

pub enum ObserverMessageBody<I, O>
where
    O: actix::Message + std::marker::Send,
    <O as actix::Message>::Result: std::marker::Send,
{
    Request {
        player_id: PlayerId,
        request: I,
    },
    RoundTripTime {
        player_id: PlayerId,
        /// Unique measurement of the round trip time, in milliseconds.
        rtt: u16,
    },
    Register {
        player_id: PlayerId,
        observer: UnboundedSender<ObserverUpdate<O>>,
        supports_unreliable: bool,
    },
    Unregister {
        player_id: PlayerId,
        observer: UnboundedSender<ObserverUpdate<O>>,
    },
}

#[derive(Message, Debug)]
#[rtype(result = "()")]
pub enum ObserverUpdate<O>
where
    O: actix::Message + std::marker::Send,
    <O as actix::Message>::Result: std::marker::Send,
{
    Close,
    Send { message: O, reliable: bool },
}
