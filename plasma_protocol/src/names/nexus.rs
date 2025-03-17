// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: LGPL-3.0-or-later

use crate::{impl_wrapper_from_str, impl_wrapper_str};
use arrayvec::ArrayString;
use bitcode::{Decode, Encode};
use serde::{Deserialize, Serialize};

#[derive(
    Copy, Clone, Debug, Eq, PartialEq, Hash, Ord, PartialOrd, Serialize, Deserialize, Encode, Decode,
)]
pub struct NexusPath(pub ArrayString<32>);
impl_wrapper_str!(NexusPath);
impl_wrapper_from_str!(NexusPath, ArrayString<32>);
