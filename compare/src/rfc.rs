//! [RFC 8428](https://www.rfc-editor.org/rfc/rfc8428) test vectors, read by
//! rsenml-tests from the cited excerpts in `crates/rsenml-tests/fixtures/rfc8428`,
//! so each vector is traceable to its section of the RFC.

use std::path::{Path, PathBuf};

pub use rsenml_tests::{hex_dumps, json_examples, JsonExample};

/// The rsenml repository root (the parent of `compare/`).
pub fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .map_or_else(|| PathBuf::from(".."), Path::to_path_buf)
}
