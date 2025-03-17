// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: LGPL-3.0-or-later

use js_sys::Function;
use wasm_bindgen::JsValue;

pub fn eval_snippet(snippet: &str) {
    // Do NOT use `eval`, since it runs in the local scope and therefore
    // prevents minification.
    // TODO: send result back to server.
    let _ = Function::new_no_args(snippet).call0(&JsValue::NULL);
}
