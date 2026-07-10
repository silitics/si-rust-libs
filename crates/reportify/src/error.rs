//! The [`Error`] and [`Whatever`] traits.

use std::error::Error as StdError;
use std::fmt::{Debug, Display};

/// Abstraction for types that can be carried by a [`Report`](crate::Report).
///
/// Implemented automatically for any [`std::error::Error`]. Implement it directly for
/// types that are not [`std::error::Error`]s but should still be reportable (e.g., simple
/// marker types created with [`new_whatever_type!`](crate::new_whatever_type)).
pub trait Error: 'static + Send + Debug {
    /// A freeform description of this error.
    ///
    /// Returning `None` is expected and normal: a report's message usually comes from an
    /// explicit annotation (added via [`Report::message`](crate::Report::message) or
    /// [`ResultExt::whatever`](crate::ResultExt::whatever)) rather than from the error
    /// type itself.
    fn message(&self) -> Option<&dyn Display>;

    /// Stable, machine-readable code identifying this error.
    fn code(&self) -> Option<&'static str> {
        None
    }

    /// Name of this error type.
    fn type_name(&self) -> &'static str {
        std::any::type_name::<Self>()
    }
}

impl<E> Error for E
where
    E: StdError + Send + 'static,
{
    fn message(&self) -> Option<&dyn Display> {
        Some(self)
    }
}

/// Error type that opts into one-off, freeform diagnostic reports.
///
/// Types implementing this trait can be constructed from arbitrary other errors, which
/// [`ResultExt::whatever`](crate::ResultExt::whatever) and
/// [`ErrorExt::whatever`](crate::ErrorExt::whatever) rely on. Use
/// [`new_whatever_type!`](crate::new_whatever_type) to define one.
///
/// A `Whatever` type never carries its own message. Callers must always describe what
/// failed at the call site, e.g., via `.whatever("unable to read configuration")`,
/// instead of relying on a generic, type-level placeholder.
pub trait Whatever: Error + Sized {
    /// Construct the boundary error for a freeform diagnostic.
    fn new() -> Self;

    /// Construct the boundary error from another error.
    ///
    /// The default implementation ignores the source error; override it to inspect the
    /// source (e.g., to pick a different variant based on its type).
    fn from_error<E>(error: &E) -> Self
    where
        E: Error,
    {
        let _ = error;
        Self::new()
    }
}
