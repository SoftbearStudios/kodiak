// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: LGPL-3.0-or-later

#[cfg(feature = "audio")]
mod audio;

mod joystick;
mod keyboard;
mod mouse;
mod pan_zoom;

#[cfg(feature = "audio")]
pub use self::audio::{Audio, AudioBufferHandle, AudioPlayer, AudioToneHandle};

pub use self::joystick::Joystick;
pub use self::keyboard::{Key, KeyState, KeyboardEvent, KeyboardState};
pub use self::mouse::{MouseButton, MouseEvent, MouseState};
pub use self::pan_zoom::PanZoom;
