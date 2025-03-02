// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: LGPL-3.0-or-later

mod arena_picker_dialog;
mod feedback_dialog;
mod licensing_dialog;
#[allow(clippy::module_inception)] // TODO
mod nexus_dialog;
mod play_with_friends_dialog;
mod privacy_dialog;
mod profile_dialog;
mod ranks_dialog;
mod settings_dialog;
mod store_dialog;
mod terms_dialog;

pub use arena_picker_dialog::ArenaPickerDialog;
pub use feedback_dialog::FeedbackDialog;
pub use licensing_dialog::LicensingDialog;
pub use nexus_dialog::{NexusDialog, NexusDialogProps};
pub use play_with_friends_dialog::PlayWithFriendsDialog;
pub use privacy_dialog::PrivacyDialog;
pub use profile_dialog::ProfileDialog;
pub use ranks_dialog::RanksDialog;
pub use settings_dialog::SettingsDialog;
pub use store_dialog::StoreDialog;
pub use terms_dialog::TermsDialog;
