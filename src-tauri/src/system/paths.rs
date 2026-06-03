use crate::errors::AppError;
use std::{fs, path::Path};

pub fn ensure_parent(path: impl AsRef<Path>) -> Result<(), AppError> {
    if let Some(parent) = path.as_ref().parent() {
        fs::create_dir_all(parent)?;
    }
    Ok(())
}
