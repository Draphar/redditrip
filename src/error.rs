/*
 * Copyright 2020 Joshua Prieth
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 * http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */

/*!
Utilities for handling errors.
*/

use std::{error, fmt, result};

/// A helpful message if a suspected bug occurred.
pub const HELP_BUG: &'static str = "This is likely a bug in this application and not your fault.\nPlease update the application. If the error persists, open an issue on GitHub.";

/// A helpful message if a network error occurred.
pub const HELP_NETWORK: &'static str = "Do you have an internet connection?";

/// A helpful message if invalid JSON was encountered.
pub const HELP_JSON: &'static str = "This is likely caused by a broken backend and not your fault.\nPlease update the application. If the error persists, open an issue on GitHub.";

/// A helpful message if ffmpeg failed.
pub const HELP_FFMPEG: &'static str = "This was an error with ffmpeg. Consider updating your local copy, or use a different '--vreddit-mode'.";

/// A convenient type for fallible operations.
pub type Result<T> = result::Result<T, Error>;

/// A generalized error type.
#[derive(Debug)]
pub enum Error {
    String(String),
    Inner(Box<dyn error::Error + 'static>),
}

impl Error {
    /// Creates a new error from a simple string.
    /// Useful if there are no underlying errors.
    pub fn new(message: impl Into<String>) -> Error {
        Error::String(message.into())
    }

    /// Creates an error with a (hopefully) helpful message.
    pub fn bug() -> Error {
        Error::String(String::from(HELP_BUG))
    }

    /// Returns the source of the error, if any.
    pub fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        match self {
            Error::String(_) => None,
            Error::Inner(e) => Some(e.as_ref()),
        }
    }

    /// Converts into the underlying error, if any.
    pub fn into_source(self) -> Option<Box<dyn error::Error + 'static>> {
        match self {
            Error::String(_) => None,
            Error::Inner(e) => Some(e),
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Error::String(message) => write!(f, "Error: {}", message),
            Error::Inner(e) => write!(f, "{}", e),
        }
    }
}

impl<E: std::error::Error + 'static> From<E> for Error {
    fn from(e: E) -> Self {
        Error::Inner(Box::new(e))
    }
}

/*
This is intentionally not implemented because of the generic
implementation above. An implementation loop would be the side effect.
impl error::Error for Error {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        match &self {
            Error::String(message) => None,
            Error::Err(e) => Some(e.as_ref())
        }
    }
}
*/
