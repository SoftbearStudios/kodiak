// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: LGPL-3.0-or-later

/// Resettable data build from updates.
pub trait Apply<U>: Default {
    /// Applies an inbound update to the state.
    fn apply(&mut self, update: U);
    /// Resets the state to default.
    fn reset(&mut self) {
        *self = Self::default();
    }
}

impl<T> Apply<T> for () {
    fn apply(&mut self, _update: T) {}
}

impl<T> Apply<T> for Vec<T> {
    fn apply(&mut self, update: T) {
        self.push(update);
    }

    fn reset(&mut self) {
        self.clear();
    }
}
