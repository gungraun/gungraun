//! Provide the `ClientRequestError`

use core::fmt::Display;
use std::ffi::FromVecWithNulError;

/// The `ClientRequestError`
#[derive(Debug)]
pub enum ClientRequestError {
    /// The error when printing with valgrind's `VALGRIND_PRINTF` fails
    ValgrindPrintError(FromVecWithNulError),
}

impl std::error::Error for ClientRequestError {}

impl From<FromVecWithNulError> for ClientRequestError {
    fn from(value: FromVecWithNulError) -> Self {
        Self::ValgrindPrintError(value)
    }
}

impl Display for ClientRequestError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::ValgrindPrintError(inner) => {
                write!(
                    f,
                    "client requests: print error: {}: '{}'",
                    inner,
                    String::from_utf8_lossy(inner.as_bytes())
                )
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_client_request_error_display_valgrind_print_error() {
        let expected = "client requests: print error: data provided contains an interior nul byte \
                        at pos 1: 'f\0o'";
        let error: ClientRequestError = std::ffi::CString::from_vec_with_nul(b"f\0o".to_vec())
            .unwrap_err()
            .into();
        assert_eq!(expected, error.to_string());
    }
}
