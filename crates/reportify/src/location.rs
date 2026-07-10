//! Source locations captured alongside reports and annotations.

use std::fmt::Display;
use std::panic::Location;

/// Source location captured when a report or annotation is created.
///
/// Owns its file path rather than borrowing it: [`Location::caller`] hands out a
/// `'static` reference, but a panic hook's own [`Location`] is only borrowed for the
/// duration of the hook call, and this type is used for both.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct SourceLocation {
    /// File path.
    pub file: String,
    /// One-based line number.
    pub line: u32,
    /// One-based column number.
    pub column: u32,
}

impl SourceLocation {
    /// Capture the location of the caller.
    #[track_caller]
    #[must_use]
    pub fn caller() -> Self {
        Self::from_std(Location::caller())
    }

    pub(crate) fn from_std(location: &Location<'_>) -> Self {
        Self {
            file: location.file().to_owned(),
            line: location.line(),
            column: location.column(),
        }
    }
}

impl Display for SourceLocation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{}:{}", self.file, self.line, self.column)
    }
}
