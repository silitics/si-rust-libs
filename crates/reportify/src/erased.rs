//! Type-erased reports, used to hold a heterogeneously-typed cause or factors in a
//! [`Context`](crate::Context).

use std::any::Any;
use std::fmt::{Debug, Display};

use crate::context::Context;
use crate::error::Error;
use crate::report::Report;

/// Object-safe view of a [`Report<E>`] that hides the concrete error type `E`.
///
/// Rendering and export only ever go through this trait, recursing into
/// [`Context::cause`]/[`Context::factors`]. Neither needs to know the concrete type of
/// any of them. Downcasting back to a concrete error, via [`ErasedReport::error_any`], is
/// an escape hatch for the rare case where a caller wants to inspect a specific one,
/// e.g., checking whether it was a caught panic, not the primary way of consuming them.
trait AnyReport: Send {
    fn error_type_name(&self) -> &'static str;
    fn error_message(&self) -> Option<&dyn Display>;
    fn error_code(&self) -> Option<&'static str>;
    fn error_any(&self) -> &dyn Any;
    fn context(&self) -> &Context;
}

impl<E: Error> AnyReport for Report<E> {
    fn error_type_name(&self) -> &'static str {
        self.error().type_name()
    }

    fn error_message(&self) -> Option<&dyn Display> {
        self.error().message()
    }

    fn error_code(&self) -> Option<&'static str> {
        self.error().code()
    }

    fn error_any(&self) -> &dyn Any {
        self.error()
    }

    fn context(&self) -> &Context {
        self.context()
    }
}

/// A [`Report`] with its typed error erased, held as another report's cause or as one of
/// its factors.
pub struct ErasedReport {
    inner: Box<dyn AnyReport>,
}

impl ErasedReport {
    /// Type name of the erased error.
    #[must_use]
    pub fn error_type_name(&self) -> &'static str {
        self.inner.error_type_name()
    }

    /// The erased error's own message, if it has one.
    ///
    /// This is often `None`. `Whatever` types (see
    /// [`new_whatever_type!`](crate::new_whatever_type)) never carry their own message.
    /// For the headline actually shown by rendering or
    /// [`Report::export`](crate::Report::export), prefer [`ErasedReport::context`]'s
    /// narrative, which is what both do.
    #[must_use]
    pub fn error_message(&self) -> Option<&dyn Display> {
        self.inner.error_message()
    }

    /// Stable code of the erased error, if any.
    #[must_use]
    pub fn error_code(&self) -> Option<&'static str> {
        self.inner.error_code()
    }

    /// Downcast the erased error to a concrete type.
    ///
    /// This is an escape hatch, e.g., to check whether a cause was a caught panic with
    /// `cause.downcast_error::<Panicked>()`. Prefer walking [`ErasedReport::context`]
    /// generically wherever possible.
    #[must_use]
    pub fn downcast_error<E: 'static>(&self) -> Option<&E> {
        self.inner.error_any().downcast_ref()
    }

    /// The erased report's context.
    #[must_use]
    pub fn context(&self) -> &Context {
        self.inner.context()
    }
}

impl Debug for ErasedReport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ErasedReport")
            .field("error_type", &self.error_type_name())
            .finish_non_exhaustive()
    }
}

impl<E: Error> From<Report<E>> for ErasedReport {
    fn from(report: Report<E>) -> Self {
        ErasedReport {
            inner: Box::new(report),
        }
    }
}

impl<E: Error> From<E> for ErasedReport {
    /// Wrap a bare error as a freshly captured [`Report`], then erase it.
    #[track_caller]
    fn from(error: E) -> Self {
        Report::new(error).into()
    }
}
