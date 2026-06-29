//! Shared internal helpers (safe casts, file permissions, etc.).

use std::path::{Path, PathBuf};

use crate::sqlite::error::WalletStorageError;

pub mod permissions;
pub mod safe_cast;

/// Canonicalize a path that is expected to exist.
pub(crate) fn canonicalize_path(path: &Path) -> Result<PathBuf, WalletStorageError> {
    path.canonicalize().map_err(WalletStorageError::Io)
}
