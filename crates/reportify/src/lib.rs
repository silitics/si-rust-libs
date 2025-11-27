use std::backtrace::{Backtrace, BacktraceStatus};
use std::error::Error as StdError;
use std::fmt::Display;
use std::panic::Location;

use tracing_error::{SpanTrace, SpanTraceStatus};

/// Error with additional context information for reporting.
#[derive(Debug)]
pub struct Report<E> {
    error: E,
    context: Box<ReportContext>,
}

impl<E: Error> Report<E> {
    /// Create a new report from the given error and context.
    pub fn new(error: E, context: ReportContext) -> Self {
        Self {
            error,
            context: Box::new(context),
        }
    }

    /// Underlying error.
    pub fn error(&self) -> &E {
        &self.error
    }

    /// Underlying context.
    pub fn context(&self) -> &ReportContext {
        &self.context
    }

    /// Consume the report and return the underlying error and context.
    pub fn into_parts(self) -> (E, ReportContext) {
        (self.error, *self.context)
    }

    /// Propagate the report converting the error using the given function.
    #[track_caller]
    fn propagate_map<F, M>(self, map: M) -> Report<F>
    where
        M: FnOnce(E) -> F,
    {
        let mut context = self.context;
        let error_item = context.items.get_mut(context.error_item).unwrap();
        // Materialize or discard the error message before mapping the error.
        if let Some(message) = self.error.message() {
            error_item.1 = ReportItem::Message(message.to_string());
        } else {
            error_item.1 = ReportItem::Discarded;
        }
        context.error_item = context.items.len();
        context.items.push((Location::caller(), ReportItem::Error));
        Report {
            error: map(self.error),
            context,
        }
    }
}

// Allow the implicit conversion from `E` to `Report<E>`. Allows propagating errors
// using the `?` operator while automatically capturing the context.
impl<E: Error> From<E> for Report<E> {
    #[track_caller]
    fn from(error: E) -> Self {
        let context = ReportContext::capture();
        Self::new(error, context)
    }
}

impl<E: Error> std::fmt::Display for Report<E> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // TODO: It might make sense to just leave the formatting of the error to the
        // `Error` trait itself, such that it can be easily customized.
        if let Some(message) = self.error.message() {
            writeln!(f, "{}", message)?;
        }
        if let Some(error) = self.error.as_std_error() {
            let mut source = error.source();
            while let Some(error) = source {
                writeln!(f, "  Caused by: {}", error)?;
                source = error.source();
            }
        }
        if !self.context.items.is_empty() {
            writeln!(f, "")?;
            for (location, item) in self.context.items.iter().rev() {
                match item {
                    ReportItem::Message(message) => writeln!(f, "{location}: {message}")?,
                    ReportItem::Error => {
                        if let Some(message) = self.error.message() {
                            writeln!(f, "{location}: {message}")?;
                        }
                    }
                    ReportItem::Discarded => { /* Nothing to do. */ }
                }
            }
        }
        if self.context.backtrace.status() == BacktraceStatus::Captured {
            writeln!(f, "\nBacktrace:\n{}", self.context.backtrace)?;
        }
        if self.context.span_trace.status() == SpanTraceStatus::CAPTURED {
            writeln!(f, "\nSpan Trace:\n{}", self.context.span_trace)?;
        }
        Ok(())
    }
}

/// Context for error reporting.
#[derive(Debug)]
pub struct ReportContext {
    backtrace: Backtrace,
    span_trace: SpanTrace,
    items: Vec<(&'static Location<'static>, ReportItem)>,
    error_item: usize,
}

#[derive(Debug)]
enum ReportItem {
    Message(String),
    Error,
    Discarded,
}

impl ReportContext {
    #[track_caller]
    pub fn capture() -> Self {
        let backtrace = Backtrace::capture();
        let span_trace = SpanTrace::capture();
        Self {
            backtrace,
            span_trace,
            items: vec![(Location::caller(), ReportItem::Error)],
            error_item: 0,
        }
    }
}

/// Error trait for errors that can be reported.
pub trait Error: Send + Sync + 'static {
    /// Error message.
    fn message(&self) -> Option<&dyn Display>;

    /// Try to cast the error into [`StdError`].
    fn as_std_error(&self) -> Option<&dyn StdError> {
        None
    }
}

impl<E: StdError + Send + Sync + 'static> Error for E {
    fn message(&self) -> Option<&dyn Display> {
        Some(self)
    }

    fn as_std_error(&self) -> Option<&dyn StdError> {
        Some(self)
    }
}

/// Error that can be constructed from arbitrary errors.
pub trait Whatever: Error {
    fn new() -> Self;

    #[track_caller]
    fn propagate<E: Error>(report: Report<E>) -> Report<Self>
    where
        Self: Sized,
    {
        report.propagate_map(|_| Self::new())
    }
}

#[macro_export]
macro_rules! new_whatever_type {
    ($(#[$meta:meta])* $vis:vis $name:ident) => {
        $(#[$meta])*
        #[derive(Debug)]
        $vis struct $name(());

        impl $crate::Error for $name {
            fn message(&self) -> Option<&dyn ::std::fmt::Display> {
                None
            }
        }

        impl $crate::Whatever for $name {
            fn new() -> Self {
                $name(())
            }
        }
    };
}

#[macro_export]
macro_rules! bail {
    ($($arg:tt)*) => {
        return Err({
            let error = $crate::Whatever::new();
            Report::new(error, $crate::ReportContext::capture())
        });
    };
}

#[macro_export]
macro_rules! ensure {
    ($cond:expr, $($args:tt)*) => {
        if !$cond {
            $crate::bail!($($args)*)
        }
    };
}

/// Context that can be attached to a report.
pub trait Context<E> {
    /// Attach this context to the given report.
    fn attach_to(self, report: &mut Report<E>);
}

impl<E, F, C> Context<E> for F
where
    F: FnOnce() -> C,
    C: Context<E>,
{
    #[track_caller]
    fn attach_to(self, report: &mut Report<E>) {
        (self)().attach_to(report);
    }
}

impl<E> Context<E> for &str {
    #[track_caller]
    fn attach_to(self, report: &mut Report<E>) {
        self.to_owned().attach_to(report);
    }
}

impl<E> Context<E> for String {
    #[track_caller]
    fn attach_to(self, report: &mut Report<E>) {
        report
            .context
            .items
            .push((Location::caller(), ReportItem::Message(self)))
    }
}

/// Trait for types that can be reported.
pub trait Reportify<O> {
    /// Report this type.
    fn report(self) -> O;
}

impl<E: Error> Reportify<Report<E>> for E {
    #[track_caller]
    fn report(self) -> Report<E> {
        let context = ReportContext::capture();
        Report::new(self, context)
    }
}

impl<T, E: Error> Reportify<Result<T, Report<E>>> for Result<T, E> {
    #[track_caller]
    fn report(self) -> Result<T, Report<E>> {
        match self {
            Ok(value) => Ok(value),
            Err(error) => Err(error.report()),
        }
    }
}

/// Extension trait for [`Result`] that adds additional methods for reporting errors.
pub trait ResultExt {
    /// Value type of the result.
    type Value;
    /// Error type of the result.
    type Error;

    /// Add context to the result.
    fn context<C: Context<Self::Error>>(
        self,
        context: C,
    ) -> Result<Self::Value, Report<Self::Error>>;

    /// Propagate the error without adding context.
    fn propagate<F>(self) -> Result<Self::Value, Report<F>>
    where
        Self::Error: Into<F>;

    /// Propagate an error as another error.
    fn propagate_map<F, M>(self, map: M) -> Result<Self::Value, Report<F>>
    where
        M: FnOnce(Self::Error) -> F;

    /// Propagate the error and add context.
    fn propagate_with<C, F>(self, context: C) -> Result<Self::Value, Report<F>>
    where
        C: Context<F>,
        F: Error,
        Self::Error: Into<F>;

    /// Propagate the error using [`Whatever`] to construct the new error.
    fn whatever<F: Whatever>(self) -> Result<Self::Value, Report<F>>;

    /// Assert that the result is [`Ok`] according to a program invariant.
    ///
    /// Only use this in case an error is a bug in the program, not an external error.
    fn assert_ok(self) -> Self::Value;

    /// Ignore the result and log the error, if any.
    fn ignore(self);
}

impl<T, E: Error> ResultExt for Result<T, E> {
    type Value = T;
    type Error = E;

    #[track_caller]
    fn context<C: Context<Self::Error>>(
        self,
        context: C,
    ) -> Result<Self::Value, Report<Self::Error>> {
        self.report().context(context)
    }

    #[track_caller]
    fn propagate<F>(self) -> Result<Self::Value, Report<F>>
    where
        Self::Error: Into<F>,
    {
        self.report().propagate()
    }

    #[track_caller]
    fn propagate_map<F, M>(self, map: M) -> Result<Self::Value, Report<F>>
    where
        M: FnOnce(Self::Error) -> F,
    {
        self.report().propagate_map(map)
    }

    #[track_caller]
    fn propagate_with<C, F>(self, context: C) -> Result<Self::Value, Report<F>>
    where
        C: Context<F>,
        F: Error,
        Self::Error: Into<F>,
    {
        self.report().propagate_with(context)
    }

    #[track_caller]
    fn whatever<F: Whatever>(self) -> Result<Self::Value, Report<F>> {
        self.report().whatever()
    }

    #[track_caller]
    fn assert_ok(self) -> Self::Value {
        self.report().assert_ok()
    }

    #[track_caller]
    fn ignore(self) {
        self.report().ignore()
    }
}

impl<T, E: Error> ResultExt for Result<T, Report<E>> {
    type Value = T;
    type Error = E;

    #[track_caller]
    fn context<C: Context<Self::Error>>(
        self,
        context: C,
    ) -> Result<Self::Value, Report<Self::Error>> {
        match self {
            Ok(value) => Ok(value),
            Err(mut report) => {
                context.attach_to(&mut report);
                Err(report)
            }
        }
    }

    #[track_caller]
    fn propagate<F>(self) -> Result<Self::Value, Report<F>>
    where
        Self::Error: Into<F>,
    {
        self.propagate_map(|error| error.into())
    }

    #[track_caller]
    fn propagate_map<F, M>(self, map: M) -> Result<Self::Value, Report<F>>
    where
        M: FnOnce(Self::Error) -> F,
    {
        match self {
            Ok(value) => Ok(value),
            Err(report) => Err(report.propagate_map(map)),
        }
    }

    #[track_caller]
    fn propagate_with<C, F>(self, context: C) -> Result<Self::Value, Report<F>>
    where
        C: Context<F>,
        F: Error,
        Self::Error: Into<F>,
    {
        self.propagate().context(context)
    }

    #[track_caller]
    fn whatever<F: Whatever>(self) -> Result<Self::Value, Report<F>> {
        match self {
            Ok(value) => Ok(value),
            Err(report) => Err(F::propagate(report)),
        }
    }

    #[track_caller]
    fn assert_ok(self) -> Self::Value {
        match self {
            Ok(value) => value,
            Err(report) => {
                panic!("BUG: found error but expected value\n\n{report}");
            }
        }
    }

    #[track_caller]
    fn ignore(self) {
        if let Err(report) = self {
            tracing::error!("ignoring error\n\n{report}");
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{Report, ResultExt};

    new_whatever_type!(pub TestError);

    fn example_bail() -> Result<(), Report<TestError>> {
        bail!("test");
    }

    fn example_propagate_whatever() -> Result<(), Report<TestError>> {
        Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "file not found",
        ))
        .whatever()
    }

    #[test]
    fn test_bail() {
        assert!(example_bail().is_err());
    }

    #[test]
    fn test_propagate_whatever() {
        assert!(example_propagate_whatever().is_err());
    }
}
