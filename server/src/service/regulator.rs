// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: LGPL-3.0-or-later

#[derive(Debug, Default)]
pub struct Regulator {
    state: State,
}

#[derive(Debug, Default)]
enum State {
    #[default]
    Initial,
    /// `join` called.
    ///
    /// Will join next tick, and transition to `Joined`.
    Joining,
    /// Was `Joining` last tick, or `join` called with fast path.
    Joined,
    /// `leave` called, presumably after `quit`.
    ///
    /// Transitions to `Leaving` next tick.
    WaitingToLeave,
    /// Was `WaitingToLeave` last tick.
    ///
    /// Will leave next tick, and transition to `Left`.
    Leaving,
    /// Was `Leaving` last tick.
    ///
    /// Transitions to `Initial` next tick.
    Left,
}

#[allow(unused)]
impl State {
    fn is_initial(&self) -> bool {
        matches!(self, Self::Initial)
    }

    fn is_joining(&self) -> bool {
        matches!(self, Self::Joining)
    }

    fn is_joined(&self) -> bool {
        matches!(self, Self::Joined)
    }

    fn is_leaving(&self) -> bool {
        matches!(self, Self::Leaving)
    }

    fn is_waiting_to_leave(&self) -> bool {
        matches!(self, Self::WaitingToLeave)
    }

    fn is_left(&self) -> bool {
        matches!(self, Self::Left)
    }
}

impl Regulator {
    /// Returns `true` iff the join is fast-path. Otherwise, the join will happen
    /// on the next `tick`.
    ///
    /// # Panics
    ///
    /// If called twice with no `leave` in between.
    #[must_use = "fast path must exist"]
    pub fn join(&mut self) -> bool {
        match self.state {
            State::Initial => {
                // Fast path.
                self.state = State::Joined;
                true
            }
            State::Joining => panic!("already joining"),
            State::Joined => panic!("already joined"),
            State::WaitingToLeave | State::Leaving => {
                // Just stop leaving.
                self.state = State::Joined;
                false
            }
            State::Left => {
                // Slow path.
                self.state = State::Joining;
                false
            }
        }
    }

    /// # Panics
    ///
    /// If not initialized by a `join`.
    pub fn leave(&mut self) {
        match self.state {
            State::Initial => {
                panic!("not joined");
            }
            State::Joining => {
                // Just stop joining.
                self.state = State::Initial;
            }
            State::Joined => {
                self.state = State::WaitingToLeave;
            }
            State::WaitingToLeave => {
                panic!("already waiting to leave");
            }
            State::Leaving => {
                panic!("already leaving");
            }
            State::Left => {
                panic!("already left");
            }
        }
    }

    pub(crate) fn leave_now(&mut self) {
        self.leave();
        if self.state.is_waiting_to_leave() {
            assert_eq!(self.tick(), None);
            assert_eq!(self.tick(), Some(false));
        } else {
            debug_assert!(false);
        }
    }

    /// Is currently 'ingame'
    pub fn active(&self) -> bool {
        self.state.is_joined()
    }

    /// Safe to forget/delete.
    pub fn can_forget(&self) -> bool {
        self.state.is_initial()
    }

    #[must_use = "return = add/remove"]
    pub fn tick(&mut self) -> Option<bool> {
        match self.state {
            State::Initial => {
                // No-op (will forget).
                None
            }
            State::Joining => {
                self.state = State::Joined;
                Some(true)
            }
            State::Joined => {
                // Steady-state.
                None
            }
            State::WaitingToLeave => {
                // Take the next step.
                self.state = State::Leaving;
                None
            }
            State::Leaving => {
                self.state = State::Left;
                Some(false)
            }
            State::Left => {
                self.state = State::Initial;
                None
            }
        }
    }
}
