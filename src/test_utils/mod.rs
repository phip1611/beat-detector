// SPDX-License-Identifier: MIT

//! Test infrastructure: synthetic signals, evaluation, and debug views.

pub mod debug;
pub mod eval;
pub mod synth;

use std::path::PathBuf;

/// Returns `target/test-artifacts/`, honoring `CARGO_TARGET_DIR`.
pub fn target_dir_test_artifacts() -> PathBuf {
    let target_dir = std::env::var_os("CARGO_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("target"));
    target_dir.join("test-artifacts")
}
