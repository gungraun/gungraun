use std::fmt::Display;

use crate::__internal::ModulePath;

#[derive(Debug)]
pub struct Error {
    message: String,
    module_path: ModulePath,
}

impl Error {
    pub fn new(module_path: &ModulePath, message: &str) -> Self {
        Self {
            module_path: module_path.clone(),
            message: message.to_owned(),
        }
    }
}

impl Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!(
            "Error in {}: {}",
            self.module_path, self.message
        ))
    }
}

/// An error aggregator to collect all errors first and then print them to stderr
#[derive(Debug, Default)]
pub struct Errors(pub Vec<Error>);

impl Errors {
    pub fn add(&mut self, error: Error) {
        self.0.push(error);
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl Display for Errors {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("gungraun: Error: The following errors occurred:\n")?;

        for error in &self.0 {
            f.write_fmt(format_args!("--> {error}\n"))?;
        }

        Ok(())
    }
}
