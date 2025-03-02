// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: LGPL-3.0-or-later

/// Types of antialiasing.
#[derive(Copy, Clone)]
pub enum Antialiasing {
    /// No antialiasing (Fastest).
    None,
    /// Fast approximate antialiasing (Fast).
    #[cfg(all(feature = "renderer_fxaa", feature = "renderer_srgb"))]
    Fxaa,
    /// Multisample antialiasing.
    ///
    /// If `not(feature = "srgb")` builtin antialiasing (Fast)
    ///
    /// If `feature = "srgb"` explicit MSAAx4 (Slow).
    Msaa,
}

impl Antialiasing {
    pub(crate) fn is_msaa(self) -> bool {
        matches!(self, Self::Msaa)
    }

    pub(crate) fn is_none(self) -> bool {
        matches!(self, Self::None)
    }

    pub(crate) fn is_some(self) -> bool {
        !self.is_none()
    }
}

/// For backwards compatibility with antialias: bool.
impl From<bool> for Antialiasing {
    fn from(value: bool) -> Self {
        if value {
            Self::Msaa
        } else {
            Self::None
        }
    }
}
