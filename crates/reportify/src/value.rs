//! Structured values and fields attached to reports.

use std::fmt::Display;
use std::path::{Path, PathBuf};

/// Visibility of a structured field, controlling whether
/// [`Report::export`](crate::Report::export) includes its value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub enum Visibility {
    /// The field can be rendered and exported by default.
    Public,
    /// The field may contain sensitive data and should be opt-in for export.
    Sensitive,
    /// The field contains secret data and should normally never be exported.
    Secret,
}

impl Display for Visibility {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Public => f.write_str("public"),
            Self::Sensitive => f.write_str("sensitive"),
            Self::Secret => f.write_str("secret"),
        }
    }
}

/// Structured value attached to a report field.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(tag = "type", content = "value"))]
#[non_exhaustive]
pub enum Value {
    /// String value.
    String(String),
    /// Boolean value.
    Bool(bool),
    /// Signed integer value.
    Signed(i64),
    /// Unsigned integer value.
    Unsigned(u64),
    /// Floating point value.
    Float(f64),
}

impl Value {
    /// Create a string value from a displayable value.
    #[must_use]
    pub fn display(value: impl Display) -> Self {
        Self::String(value.to_string())
    }
}

impl Display for Value {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::String(value) => f.write_str(value),
            Self::Bool(value) => write!(f, "{value}"),
            Self::Signed(value) => write!(f, "{value}"),
            Self::Unsigned(value) => write!(f, "{value}"),
            Self::Float(value) => write!(f, "{value}"),
        }
    }
}

macro_rules! impl_from_signed {
    ($($ty:ty),*) => {
        $(impl From<$ty> for Value {
            fn from(value: $ty) -> Self {
                Self::Signed(i64::from(value))
            }
        })*
    };
}

macro_rules! impl_from_unsigned {
    ($($ty:ty),*) => {
        $(impl From<$ty> for Value {
            fn from(value: $ty) -> Self {
                Self::Unsigned(u64::from(value))
            }
        })*
    };
}

impl_from_signed!(i8, i16, i32, i64);
impl_from_unsigned!(u8, u16, u32, u64);

impl From<&str> for Value {
    fn from(value: &str) -> Self {
        Self::String(value.to_owned())
    }
}

impl From<String> for Value {
    fn from(value: String) -> Self {
        Self::String(value)
    }
}

impl From<&String> for Value {
    fn from(value: &String) -> Self {
        Self::String(value.clone())
    }
}

impl From<bool> for Value {
    fn from(value: bool) -> Self {
        Self::Bool(value)
    }
}

impl From<f32> for Value {
    fn from(value: f32) -> Self {
        Self::Float(f64::from(value))
    }
}

impl From<f64> for Value {
    fn from(value: f64) -> Self {
        Self::Float(value)
    }
}

impl From<&Path> for Value {
    fn from(value: &Path) -> Self {
        Self::String(value.display().to_string())
    }
}

impl From<PathBuf> for Value {
    fn from(value: PathBuf) -> Self {
        Self::String(value.display().to_string())
    }
}

/// Structured field attached to a report.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
pub struct Field {
    /// Field key.
    pub key: String,
    /// Field value.
    pub value: Value,
    /// Field visibility.
    pub visibility: Visibility,
}

impl Field {
    /// Create a public field.
    #[must_use]
    pub fn public(key: impl Into<String>, value: impl Into<Value>) -> Self {
        Self {
            key: key.into(),
            value: value.into(),
            visibility: Visibility::Public,
        }
    }

    /// Create a sensitive field.
    #[must_use]
    pub fn sensitive(key: impl Into<String>, value: impl Into<Value>) -> Self {
        Self {
            key: key.into(),
            value: value.into(),
            visibility: Visibility::Sensitive,
        }
    }

    /// Create a secret field.
    #[must_use]
    pub fn secret(key: impl Into<String>, value: impl Into<Value>) -> Self {
        Self {
            key: key.into(),
            value: value.into(),
            visibility: Visibility::Secret,
        }
    }
}
