// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: LGPL-3.0-or-later

use super::actor::ServerActor;
use super::service::ArenaService;
use crate::{Referrer, ServerId};
use actix::Addr;
use bytes::Bytes;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

pub struct AppState<G: ArenaService> {
    pub server_id: ServerId,
    pub server: Addr<ServerActor<G>>,
    pub ads_txt: Arc<RwLock<HashMap<Option<Referrer>, Bytes>>>,
}

impl<G: ArenaService> AppState<G> {
    pub fn new(
        server_id: ServerId,
        server: Addr<ServerActor<G>>,
        ads_txt: Arc<RwLock<HashMap<Option<Referrer>, Bytes>>>,
    ) -> Self {
        Self {
            server_id,
            server,
            ads_txt,
        }
    }
}

impl<G: ArenaService> Clone for AppState<G> {
    fn clone(&self) -> Self {
        Self {
            server_id: self.server_id,
            server: self.server.clone(),
            ads_txt: self.ads_txt.clone(),
        }
    }
}
