//! Extension traits for errors and results.

use crate::annotation::{IntoMessage, IntoSuggestion};
use crate::error::{Error, Whatever};
use crate::report::Report;
use crate::value::Value;

/// Extension trait for bare errors.
pub trait ErrorExt: Error + Sized {
    /// Wrap this error in a [`Report`].
    fn report(self) -> Report<Self>;

    /// Escalate this error directly into a differently-typed report.
    fn escalate<F: Error>(self, error: F) -> Report<F>;

    /// Convert this error into a freeform [`Whatever`] report.
    ///
    /// # Panics
    ///
    /// Does not panic; the description is required so there is no way to construct a
    /// report without one.
    fn whatever<F: Whatever>(self, description: impl IntoMessage) -> Report<F>;

    /// Convert this error into a freeform [`Whatever`] report with a lazily computed
    /// description.
    fn whatever_with<F, M>(self, description: impl FnOnce(&Self) -> M) -> Report<F>
    where
        F: Whatever,
        M: IntoMessage;
}

impl<E: Error> ErrorExt for E {
    #[track_caller]
    fn report(self) -> Report<Self> {
        Report::new(self)
    }

    #[track_caller]
    fn escalate<F: Error>(self, error: F) -> Report<F> {
        self.report().escalate(error)
    }

    #[track_caller]
    fn whatever<F: Whatever>(self, description: impl IntoMessage) -> Report<F> {
        let new_error = F::from_error(&self);
        self.report().escalate(new_error).message(description)
    }

    #[track_caller]
    fn whatever_with<F, M>(self, description: impl FnOnce(&Self) -> M) -> Report<F>
    where
        F: Whatever,
        M: IntoMessage,
    {
        let description = description(&self);
        self.whatever(description)
    }
}

/// Extension trait for results, implemented both for `Result<T, E>` and for
/// `Result<T, Report<E>>`. Every method returns `Result<T, Report<Err>>`, which is a new
/// type for the former and simply `Self` for the latter.
pub trait ResultExt<T, Err: Error> {
    /// Wrap the error, if any, in a [`Report`].
    ///
    /// # Errors
    ///
    /// Returns `Err` exactly when `self` was `Err`.
    fn report(self) -> Result<T, Report<Err>>;

    /// Escalate the error, if any, into a differently-typed report.
    ///
    /// # Errors
    ///
    /// Returns `Err` exactly when `self` was `Err`.
    fn escalate<F: Error>(self, error: F) -> Result<T, Report<F>>;

    /// Convert the error, if any, into a freeform [`Whatever`] report.
    ///
    /// # Errors
    ///
    /// Returns `Err` exactly when `self` was `Err`.
    fn whatever<F: Whatever>(self, description: impl IntoMessage) -> Result<T, Report<F>>;

    /// Convert the error, if any, into a freeform [`Whatever`] report with a lazily
    /// computed description.
    ///
    /// # Errors
    ///
    /// Returns `Err` exactly when `self` was `Err`.
    fn whatever_with<F, M>(self, description: impl FnOnce(&Err) -> M) -> Result<T, Report<F>>
    where
        F: Whatever,
        M: IntoMessage;

    /// Add a message to the report, if any.
    ///
    /// # Errors
    ///
    /// Returns `Err` exactly when `self` was `Err`.
    fn message(self, message: impl IntoMessage) -> Result<T, Report<Err>>;

    /// Add a suggestion to the report, if any.
    ///
    /// # Errors
    ///
    /// Returns `Err` exactly when `self` was `Err`.
    fn suggestion(self, suggestion: impl IntoSuggestion) -> Result<T, Report<Err>>;

    /// Attach a public field to the report, if any.
    ///
    /// # Errors
    ///
    /// Returns `Err` exactly when `self` was `Err`.
    fn field(self, key: impl Into<String>, value: impl Into<Value>) -> Result<T, Report<Err>>;

    /// Log the report at error level, if any, and return the value.
    ///
    /// The rendered report is the event's message; `error.type` and `error.code` (when
    /// the error has one) are attached as separate `tracing` fields, so a structured
    /// subscriber can filter or group on them without parsing the message text.
    fn log_error(self) -> Option<T>;

    /// Log the report at warning level, if any, and return the value.
    ///
    /// See [`ResultExt::log_error`] for exactly which fields are attached.
    fn log_warning(self) -> Option<T>;

    /// Log the report at info level, if any, and return the value.
    ///
    /// See [`ResultExt::log_error`] for exactly which fields are attached.
    fn log_info(self) -> Option<T>;

    /// Log the report at error level, if any, and discard the value.
    fn ignore(self);

    /// Unwrap the value, treating an error as a bug in the program rather than an
    /// external failure.
    ///
    /// Only use this where an error genuinely indicates a bug, e.g., an invariant a
    /// caller already checked, never for errors that can legitimately happen, e.g., I/O.
    /// `invariant` documents that assumption, e.g., `"config was already validated
    /// during startup"`; like [`Report::message`](crate::Report::message), it accepts a
    /// plain string or a closure, evaluated lazily only if `self` was actually `Err`.
    ///
    /// # Panics
    ///
    /// Panics, showing `invariant` and the full rendered report, if `self` was `Err`. If
    /// left uncaught, this prints just that rendered text to stderr, not the default
    /// panic hook's own "thread panicked at" banner and (with `RUST_BACKTRACE` set) its
    /// separate, unfiltered backtrace, which would otherwise show up redundantly right
    /// next to the one already inside the rendered report.
    fn assert_ok(self, invariant: impl IntoMessage) -> T;
}

impl<T, E: Error> ResultExt<T, E> for Result<T, E> {
    #[track_caller]
    fn report(self) -> Result<T, Report<E>> {
        // Not `self.map_err(Report::new)`: a `#[track_caller]` function loses caller
        // tracking when passed through `map_err`'s generic `FnOnce`, since the call
        // happens inside `map_err`'s own body, not literally at this call site. Calling
        // it directly in a `match` arm keeps the caller's location correct (verified
        // against a real caller, not just by inspection).
        match self {
            Ok(value) => Ok(value),
            Err(error) => Err(Report::new(error)),
        }
    }

    #[track_caller]
    fn escalate<F: Error>(self, error: F) -> Result<T, Report<F>> {
        match self {
            Ok(value) => Ok(value),
            Err(error_value) => Err(error_value.escalate(error)),
        }
    }

    #[track_caller]
    fn whatever<F: Whatever>(self, description: impl IntoMessage) -> Result<T, Report<F>> {
        match self {
            Ok(value) => Ok(value),
            Err(error) => Err(error.whatever(description)),
        }
    }

    #[track_caller]
    fn whatever_with<F, M>(self, description: impl FnOnce(&E) -> M) -> Result<T, Report<F>>
    where
        F: Whatever,
        M: IntoMessage,
    {
        match self {
            Ok(value) => Ok(value),
            Err(error) => Err(error.whatever_with(description)),
        }
    }

    #[track_caller]
    fn message(self, message: impl IntoMessage) -> Result<T, Report<E>> {
        match self.report() {
            Ok(value) => Ok(value),
            Err(report) => Err(report.message(message)),
        }
    }

    #[track_caller]
    fn suggestion(self, suggestion: impl IntoSuggestion) -> Result<T, Report<E>> {
        match self.report() {
            Ok(value) => Ok(value),
            Err(report) => Err(report.suggestion(suggestion)),
        }
    }

    #[track_caller]
    fn field(self, key: impl Into<String>, value: impl Into<Value>) -> Result<T, Report<E>> {
        // `Report::field` never reads the caller's location, so `map_err` is fine here;
        // only `self.report()`, which does, needs this function itself tracked.
        self.report().map_err(|report| report.field(key, value))
    }

    #[track_caller]
    fn log_error(self) -> Option<T> {
        self.report().log_error()
    }

    #[track_caller]
    fn log_warning(self) -> Option<T> {
        self.report().log_warning()
    }

    #[track_caller]
    fn log_info(self) -> Option<T> {
        self.report().log_info()
    }

    #[track_caller]
    fn ignore(self) {
        self.report().ignore();
    }

    #[track_caller]
    fn assert_ok(self, invariant: impl IntoMessage) -> T {
        self.report().assert_ok(invariant)
    }
}

impl<T, E: Error> ResultExt<T, E> for Result<T, Report<E>> {
    fn report(self) -> Result<T, Report<E>> {
        self
    }

    #[track_caller]
    fn escalate<F: Error>(self, error: F) -> Result<T, Report<F>> {
        match self {
            Ok(value) => Ok(value),
            Err(report) => Err(report.escalate(error)),
        }
    }

    #[track_caller]
    fn whatever<F: Whatever>(self, description: impl IntoMessage) -> Result<T, Report<F>> {
        match self {
            Ok(value) => Ok(value),
            Err(report) => {
                let new_error = F::from_error(report.error());
                Err(report.escalate(new_error).message(description))
            }
        }
    }

    #[track_caller]
    fn whatever_with<F, M>(self, description: impl FnOnce(&E) -> M) -> Result<T, Report<F>>
    where
        F: Whatever,
        M: IntoMessage,
    {
        match self {
            Ok(value) => Ok(value),
            Err(report) => {
                let description = description(report.error());
                let new_error = F::from_error(report.error());
                Err(report.escalate(new_error).message(description))
            }
        }
    }

    #[track_caller]
    fn message(self, message: impl IntoMessage) -> Result<T, Report<E>> {
        match self {
            Ok(value) => Ok(value),
            Err(report) => Err(report.message(message)),
        }
    }

    #[track_caller]
    fn suggestion(self, suggestion: impl IntoSuggestion) -> Result<T, Report<E>> {
        match self {
            Ok(value) => Ok(value),
            Err(report) => Err(report.suggestion(suggestion)),
        }
    }

    fn field(self, key: impl Into<String>, value: impl Into<Value>) -> Result<T, Report<E>> {
        self.map_err(|report| report.field(key, value))
    }

    fn log_error(self) -> Option<T> {
        match self {
            Ok(value) => Some(value),
            Err(report) => {
                tracing::error!(
                    error.r#type = report.error().type_name(),
                    error.code = report.error().code(),
                    "{report}"
                );
                None
            }
        }
    }

    fn log_warning(self) -> Option<T> {
        match self {
            Ok(value) => Some(value),
            Err(report) => {
                tracing::warn!(
                    error.r#type = report.error().type_name(),
                    error.code = report.error().code(),
                    "{report}"
                );
                None
            }
        }
    }

    fn log_info(self) -> Option<T> {
        match self {
            Ok(value) => Some(value),
            Err(report) => {
                tracing::info!(
                    error.r#type = report.error().type_name(),
                    error.code = report.error().code(),
                    "{report}"
                );
                None
            }
        }
    }

    fn ignore(self) {
        let _ = self.log_error();
    }

    #[track_caller]
    fn assert_ok(self, invariant: impl IntoMessage) -> T {
        match self {
            Ok(value) => value,
            Err(report) => crate::panic::panic_rendered(format!(
                "{}: expected a value, found an unexpected error:\n\n{report:?}",
                invariant.into_message().text
            )),
        }
    }
}
