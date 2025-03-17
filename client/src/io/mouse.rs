// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: LGPL-3.0-or-later

use crate::Apply;
use kodiak_common::glam::Vec2;
use strum::EnumIter;

/// Identifies a mouse button (left, middle, or right).
#[derive(Copy, Clone, Debug, Eq, PartialEq, EnumIter)]
pub enum MouseButton {
    Left,
    Middle,
    Right,
}

impl MouseButton {
    /// Converts from JS mouse button, if possible.
    pub fn try_from_button(mouse_button: i16) -> Option<Self> {
        Some(match mouse_button {
            0 => Self::Left,
            1 => Self::Middle,
            2 => Self::Right,
            _ => return None,
        })
    }
}

/// The state of one mouse button.
#[derive(Default, Copy, Clone)]
pub enum MouseButtonState {
    /// The button was pressed and released fast enough to form a click.
    ///
    /// This state will persist until other mouse activity or manually
    /// cleared (see `MouseState::take_click`).
    ///
    /// Stores the time the button was released as part of the click.
    Click(f32),
    /// Stores the time the button was pressed.
    Down(f32),
    #[default]
    Up,
}

impl MouseButtonState {
    /// If the mouse is released within this time, it is considered a click.
    pub const MAX_CLICK_TIME: f32 = 0.180;

    /// Whether the mouse is up (after a click).
    pub fn is_click(&self) -> bool {
        matches!(self, Self::Click(_))
    }

    /// Returns time of last mouse click, if any.
    pub fn click_time(&self) -> Option<f32> {
        if let Self::Click(time) = self {
            Some(*time)
        } else {
            None
        }
    }

    /// Whether the mouse is up (after a click). Resets state back to up (no click).
    pub fn take_click(&mut self) -> bool {
        if self.is_click() {
            *self = Self::Up;
            true
        } else {
            false
        }
    }

    /// Whether the mouse is down (or clicking).
    pub fn is_down(&self) -> bool {
        matches!(self, Self::Down(_))
    }

    /// Whether the mouse is down (for too long to be clicking).
    pub fn is_down_not_click(&self, time: f32) -> bool {
        self.is_down_for(Self::MAX_CLICK_TIME, time)
    }

    /// Whether the mouse is down for a certain amount of time.
    pub fn is_down_for(&self, down_time: f32, time: f32) -> bool {
        if let &Self::Down(t) = self {
            time > t + down_time
        } else {
            false
        }
    }

    /// Whether the mouse is up (no past click).
    pub fn is_up(&self) -> bool {
        matches!(self, Self::Up)
    }
}

/// Any type of mouse event. `Self::Wheel` may be emulated by any zooming intent.
#[derive(Debug)]
pub enum MouseEvent {
    Button {
        button: MouseButton,
        down: bool,
        time: f32,
    },
    Wheel(f32),
    /// Position in view space (-1..1).
    MoveViewSpace(Vec2),
    /// Delta in device specific pixels. Useful for pointer lock.
    DeltaPixels(Vec2),
    /// For non-touchscreen devices.
    Mouse,
    /// For touchscreen devices.
    Touch,
    /// Pointer lock changed.
    #[cfg(feature = "pointer_lock")]
    PointerLock(bool),
}

/// The state of the mouse i.e. buttons and position.
#[derive(Default)]
pub struct MouseState {
    states: [MouseButtonState; std::mem::variant_count::<MouseButton>()],
    /// Position in view space (-1..1).
    /// None if mouse isn't on game.
    pub view_position: Option<Vec2>,
    /// During a pinch to zoom gesture, stores last distance value.
    pub(crate) pinch_distance: Option<f32>,
    /// Whether the player is interacting with the game via a touch-screen.
    pub touch_screen: bool,
    /// Whether pointer-locked.
    #[cfg(feature = "pointer_lock")]
    pub pointer_locked: bool,
}

impl Apply<MouseEvent> for MouseState {
    fn apply(&mut self, event: MouseEvent) {
        match event {
            MouseEvent::Button { button, down, time } => {
                if down {
                    if !self.state(button).is_down() {
                        *self.state_mut(button) = MouseButtonState::Down(time);
                    }
                } else if let MouseButtonState::Down(t) = self.state(button) {
                    *self.state_mut(button) = if time <= t + MouseButtonState::MAX_CLICK_TIME {
                        MouseButtonState::Click(time)
                    } else {
                        MouseButtonState::Up
                    }
                } else {
                    *self.state_mut(button) = MouseButtonState::Up;
                }
            }
            MouseEvent::MoveViewSpace(position) => {
                self.view_position = Some(position);
            }
            MouseEvent::Mouse => {
                self.touch_screen = false;
            }
            MouseEvent::Touch => {
                self.touch_screen = true;
            }
            #[cfg(feature = "pointer_lock")]
            MouseEvent::PointerLock(pointer_locked) => {
                self.pointer_locked = pointer_locked;
            }
            _ => {}
        }
    }

    fn reset(&mut self) {
        #[cfg(feature = "pointer_lock")]
        if self.pointer_locked {
            crate::exit_pointer_lock_with_emulation();
        }
        *self = Self::default();
    }
}

impl MouseState {
    /// Immutable reference to the state of a particular button.
    pub fn state(&self, button: MouseButton) -> &MouseButtonState {
        &self.states[button as usize]
    }

    /// Mutable reference to the state of a particular button.
    pub(crate) fn state_mut(&mut self, button: MouseButton) -> &mut MouseButtonState {
        &mut self.states[button as usize]
    }

    /// See `MouseButtonState::is_click`.
    pub fn is_click(&self, button: MouseButton) -> bool {
        self.state(button).is_click()
    }

    /// See `MouseButtonState::click_time`.
    pub fn click_time(&self, button: MouseButton) -> Option<f32> {
        self.state(button).click_time()
    }

    /// See `MouseButtonState::take_click`.
    pub fn take_click(&mut self, button: MouseButton) -> bool {
        self.state_mut(button).take_click()
    }

    /// See `MouseButtonState::is_down`.
    pub fn is_down(&self, button: MouseButton) -> bool {
        self.state(button).is_down()
    }

    /// See `MouseButtonState::is_down_not_click`.
    pub fn is_down_not_click(&self, button: MouseButton, time: f32) -> bool {
        self.state(button).is_down_not_click(time)
    }

    /// See `MouseButtonState::is_up`.
    pub fn is_up(&self, button: MouseButton) -> bool {
        self.state(button).is_up()
    }
}
