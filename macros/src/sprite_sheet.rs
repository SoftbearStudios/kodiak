// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: LGPL-3.0-or-later

use serde::Deserialize;
use std::collections::HashMap;

// TODO: duplicates kodiak_client/sprite_sheet

#[derive(Deserialize)]
pub struct AudioSpriteSheet {
    /// AudioSprites are addressed by their name, and may have multiple variations.
    pub sprites: HashMap<String, AudioSprite>,
}

#[derive(Debug, Deserialize)]
pub struct AudioSprite {
    pub music: bool,
    pub start: f32,
    pub looping: bool,
    pub loop_start: Option<f32>,
    pub duration: f32,
}
