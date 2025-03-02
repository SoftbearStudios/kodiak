// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: LGPL-3.0-or-later

use super::actor::ServerActor;
use super::service::ArenaService;
use actix::{ActorContext, Handler, Message};

/// Asks the server to stop itself.
#[derive(Message)]
#[rtype(result = "()")]
pub struct Shutdown;

impl<G: ArenaService> Handler<Shutdown> for ServerActor<G> {
    type Result = ();

    fn handle(&mut self, _request: Shutdown, ctx: &mut Self::Context) -> Self::Result {
        ctx.stop();
    }
}
