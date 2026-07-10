//! The [`Report`] type.

use std::fmt::{Debug, Display};

#[cfg(feature = "spantrace")]
use tracing_error::SpanTrace;

use crate::annotation::{Annotation, IntoMessage, IntoSuggestion};
use crate::backtrace::Backtrace;
use crate::context::Context;
use crate::erased::ErasedReport;
use crate::error::{Error, Whatever};
use crate::export::{ExportOptions, ExportedReport};
use crate::location::SourceLocation;
use crate::render;
use crate::value::{Field, Value};

/// A typed error and its [`Context`], stored behind one allocation.
///
/// `Report<E>` is a single pointer, regardless of the size of `E` or how much narrative,
/// fields, or factors accumulate in the context: growing either never grows
/// `size_of::<Report<E>>()`, which stays constant since `Report<E>` is what every `?` on
/// a `Result<T, Report<E>>` moves.
struct Inner<E> {
    error: E,
    context: Context,
}

/// A typed error (`E`) together with everything else needed to report it: a narrative,
/// structured fields, a backtrace, and, optionally, a cause and contributing factors.
///
/// Its `error` is the machine-readable value programs branch on; everything else lives in
/// [`Context`] and is reached through [`Report::context`].
pub struct Report<E> {
    inner: Box<Inner<E>>,
}

impl<E> Report<E> {
    /// The underlying typed error.
    #[must_use]
    pub fn error(&self) -> &E {
        &self.inner.error
    }

    /// The underlying typed error, mutably.
    #[must_use]
    pub fn error_mut(&mut self) -> &mut E {
        &mut self.inner.error
    }

    /// Consume the report, returning the typed error and discarding the context.
    #[must_use]
    pub fn into_error(self) -> E {
        self.inner.error
    }

    /// Everything about this report other than its typed error.
    #[must_use]
    pub fn context(&self) -> &Context {
        &self.inner.context
    }
}

impl<E: Error> Report<E> {
    /// Create a new report for the given error, capturing a backtrace and location here.
    #[track_caller]
    #[must_use]
    pub fn new(error: E) -> Self {
        Self {
            inner: Box::new(Inner {
                error,
                context: Context::capture(),
            }),
        }
    }

    /// Create a new report using a [`Whatever`] error, with an explicit description.
    ///
    /// There is no way to construct a `Whatever` report without a description. See
    /// [`Whatever`].
    #[track_caller]
    #[must_use]
    pub fn whatever(description: impl IntoMessage) -> Self
    where
        E: Whatever,
    {
        Self::new(E::new()).message(description)
    }

    /// Add a message describing what was being attempted, or what happened.
    #[track_caller]
    #[must_use]
    pub fn message(mut self, message: impl IntoMessage) -> Self {
        let message = message.into_message();
        let location = SourceLocation::caller();
        self.inner.context.annotations.push(Annotation::Message {
            text: message.text,
            location,
        });
        self
    }

    /// Add forward-looking, actionable advice. Not part of the causal narrative.
    ///
    /// ```
    /// use reportify::{new_whatever_type, Report};
    ///
    /// new_whatever_type! { ConfigError }
    ///
    /// let report = Report::<ConfigError>::whatever("no configuration file found")
    ///     .suggestion("create one by copying `config.example.toml` to `config.toml`");
    /// assert_eq!(report.context().annotations().len(), 2);
    /// ```
    #[track_caller]
    #[must_use]
    pub fn suggestion(mut self, suggestion: impl IntoSuggestion) -> Self {
        let suggestion = suggestion.into_suggestion();
        let location = SourceLocation::caller();
        self.inner.context.annotations.push(Annotation::Suggestion {
            text: suggestion.text,
            location,
        });
        self
    }

    /// Attach a public field.
    #[must_use]
    pub fn field(mut self, key: impl Into<String>, value: impl Into<Value>) -> Self {
        self.inner.context.fields.push(Field::public(key, value));
        self
    }

    /// Attach a public field, formatting `value` with [`Display`].
    ///
    /// Shorthand for `.field(key, value.to_string())`, for a displayable value with no
    /// [`Value`] conversion of its own, e.g., a `SocketAddr`.
    #[must_use]
    pub fn field_display(self, key: impl Into<String>, value: impl Display) -> Self {
        self.field(key, value.to_string())
    }

    /// Attach a public field, formatting `value` with [`Debug`].
    ///
    /// Shorthand for `.field(key, format!("{value:?}"))`, for a value with no [`Display`]
    /// impl at all, e.g., a [`Duration`](std::time::Duration).
    #[must_use]
    pub fn field_debug(self, key: impl Into<String>, value: impl Debug) -> Self {
        self.field(key, format!("{value:?}"))
    }

    /// Attach a sensitive field (excluded from export unless explicitly requested).
    #[must_use]
    pub fn sensitive_field(mut self, key: impl Into<String>, value: impl Into<Value>) -> Self {
        self.inner.context.fields.push(Field::sensitive(key, value));
        self
    }

    /// Attach a secret field (excluded from export unless explicitly requested).
    #[must_use]
    pub fn secret_field(mut self, key: impl Into<String>, value: impl Into<Value>) -> Self {
        self.inner.context.fields.push(Field::secret(key, value));
        self
    }

    /// Attach an independent, contributing factor to this report.
    ///
    /// Unlike [`Report::escalate`], this makes no claim that `factor` alone was necessary
    /// or sufficient. It is simply attached alongside this report. See
    /// [`Context::factors`](crate::Context::factors).
    #[must_use]
    pub fn with_factor(mut self, factor: impl Into<ErasedReport>) -> Self {
        self.inner.context.factors.push(factor.into());
        self
    }

    /// Attach several independent, contributing factors to this report at once, e.g.,
    /// merging every validation error found instead of stopping at the first one.
    ///
    /// ```
    /// use reportify::{new_whatever_type, Report};
    ///
    /// new_whatever_type! { FieldError }
    /// new_whatever_type! { ValidationError }
    ///
    /// let problems = vec![
    ///     Report::<FieldError>::whatever("email is not a valid address"),
    ///     Report::<FieldError>::whatever("password must be at least 8 characters"),
    /// ];
    /// let report = Report::<ValidationError>::whatever("validation failed").with_factors(problems);
    /// assert_eq!(report.context().factors().len(), 2);
    /// ```
    #[must_use]
    pub fn with_factors<I>(mut self, factors: I) -> Self
    where
        I: IntoIterator,
        I::Item: Into<ErasedReport>,
    {
        self.inner
            .context
            .factors
            .extend(factors.into_iter().map(Into::into));
        self
    }

    /// Produce a new, differently-typed report from this one, keeping `self` as its
    /// cause.
    ///
    /// This does not modify `self`. It becomes an immutable
    /// [`Context::cause`](crate::Context::cause) nested inside a fresh `Report<F>`
    /// for `error`. This is the only way a cause gets set: there is no way to attach
    /// a cause to an already-existing report, only to derive a new report from an old
    /// one.
    ///
    /// ```
    /// use reportify::{new_whatever_type, Report, Whatever};
    ///
    /// new_whatever_type! { IoError }
    /// new_whatever_type! { ConfigError }
    ///
    /// let io_report = Report::<IoError>::whatever("file not found");
    /// let config_report = io_report.escalate(ConfigError::new());
    /// assert!(config_report.context().cause().is_some());
    /// ```
    #[track_caller]
    #[must_use]
    pub fn escalate<F: Error>(self, error: F) -> Report<F> {
        let mut escalated = Report::new(error);
        escalated.inner.context.cause = Some(self.into());
        escalated
    }

    /// Export this report for machine consumption, redacting sensitive and secret fields.
    ///
    /// Equivalent to `self.export_with(ExportOptions::default())`. See
    /// [`Report::export_with`] for an example of opting into redacted fields.
    #[must_use]
    pub fn export(&self) -> ExportedReport {
        self.export_with(ExportOptions::default())
    }

    /// Export this report for machine consumption with explicit redaction options.
    ///
    /// ```
    /// use reportify::export::ExportOptions;
    /// use reportify::{new_whatever_type, Report};
    ///
    /// new_whatever_type! { AuthError }
    ///
    /// let report = Report::<AuthError>::whatever("login failed").secret_field("token", "abc123");
    /// assert!(report.export().fields[0].redacted);
    ///
    /// let options = ExportOptions::new().with_secrets();
    /// assert!(!report.export_with(options).fields[0].redacted);
    /// ```
    #[must_use]
    pub fn export_with(&self, options: ExportOptions) -> ExportedReport {
        crate::export::build(
            self.inner.error.type_name(),
            self.inner.error.message().map(ToString::to_string),
            self.inner.error.code(),
            &self.inner.context,
            options,
        )
    }

    /// Render this report as text, with explicit control over charset, color, and detail.
    ///
    /// `Display` (`{report}`) is equivalent to `render(RenderOptions::default())`;
    /// `Debug` (`{report:?}`) is equivalent to
    /// `render(RenderOptions::default().verbose())`, which additionally includes captured
    /// backtraces and span traces. Both check stderr for terminal color support by
    /// default; see [`render::ColorMode`].
    ///
    /// ```
    /// use reportify::render::RenderOptions;
    /// use reportify::{new_whatever_type, Report};
    ///
    /// new_whatever_type! { ConfigError }
    ///
    /// let report = Report::<ConfigError>::whatever("unable to load configuration");
    /// let text = report.render(RenderOptions::new().ascii().color(reportify::render::ColorMode::Never));
    /// assert!(text.starts_with("unable to load configuration"));
    /// ```
    #[must_use]
    pub fn render(&self, options: render::RenderOptions) -> String {
        render::render(
            self.inner.error.type_name(),
            self.inner.error.message(),
            self.inner.error.code(),
            &self.inner.context,
            options,
        )
    }

    /// Render and print this report to stdout, followed by a newline.
    ///
    /// Corrects [`ColorMode::AutoStdout`](render::ColorMode::AutoStdout)/
    /// [`ColorMode::AutoStderr`](render::ColorMode::AutoStderr) to check stdout via
    /// [`RenderOptions::for_stdout`](render::RenderOptions::for_stdout), regardless
    /// of which one `options` was carrying, since that is where this method
    /// actually prints.
    pub fn print(&self, options: render::RenderOptions) {
        println!("{}", self.render(options.for_stdout()));
    }

    /// Render and print this report to stderr, followed by a newline.
    ///
    /// Corrects [`ColorMode::AutoStdout`](render::ColorMode::AutoStdout)/
    /// [`ColorMode::AutoStderr`](render::ColorMode::AutoStderr) to check stderr via
    /// [`RenderOptions::for_stderr`](render::RenderOptions::for_stderr), regardless
    /// of which one `options` was carrying, since that is where this method
    /// actually prints.
    pub fn eprint(&self, options: render::RenderOptions) {
        eprintln!("{}", self.render(options.for_stderr()));
    }

    /// Construct a report using an already-captured location/backtrace/span trace, rather
    /// than capturing fresh ones here.
    ///
    /// Used by [`catch_unwind`](crate::catch_unwind): capturing a backtrace *after*
    /// `std::panic::catch_unwind` returns would point at the wrapper, not the actual
    /// panic site, since the stack has already unwound by then.
    pub(crate) fn from_capture(
        error: E,
        location: SourceLocation,
        backtrace: Backtrace,
        #[cfg(feature = "spantrace")] spantrace: SpanTrace,
    ) -> Self {
        Self {
            inner: Box::new(Inner {
                error,
                context: Context {
                    annotations: Vec::new(),
                    fields: Vec::new(),
                    location,
                    backtrace,
                    #[cfg(feature = "spantrace")]
                    spantrace,
                    cause: None,
                    factors: Vec::new(),
                },
            }),
        }
    }
}

impl<E: Error> Display for Report<E> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.render(render::RenderOptions::default()))
    }
}

impl<E: Error> Debug for Report<E> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.render(render::RenderOptions::default().verbose()))
    }
}
