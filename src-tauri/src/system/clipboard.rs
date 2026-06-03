use crate::errors::AppError;

pub fn write_text(text: String) -> Result<(), AppError> {
    let mut clipboard =
        arboard::Clipboard::new().map_err(|error| AppError::Clipboard(error.to_string()))?;
    clipboard
        .set_text(text)
        .map_err(|error| AppError::Clipboard(error.to_string()))
}
