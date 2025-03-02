// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: LGPL-3.0-or-later

mod json_files;
mod os;
mod static_files;
mod txt_files;

pub use self::json_files::{related_website_json, system_json_file, translation_json_file};
pub use self::os::set_open_file_limit;
pub use self::static_files::{static_size_and_hash, StaticFilesHandler};
pub use self::txt_files::{ads_txt_file, robots_txt_file, sitemap_txt_file};
