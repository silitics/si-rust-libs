//! Everything about a report other than its typed error.

#[cfg(feature = "spantrace")]
use tracing_error::SpanTrace;
#[cfg(feature = "spantrace")]
use tracing_error::SpanTraceStatus;

use crate::annotation::Annotation;
use crate::backtrace::Backtrace;
use crate::erased::ErasedReport;
use crate::location::SourceLocation;
use crate::value::Field;

/// Everything about a [`Report`](crate::Report) other than its typed error: the
/// narrative, structured fields, where and when it was created, its cause, and its
/// factors.
#[derive(Debug)]
pub struct Context {
    pub(crate) annotations: Vec<Annotation>,
    pub(crate) fields: Vec<Field>,
    pub(crate) location: SourceLocation,
    pub(crate) backtrace: Backtrace,
    #[cfg(feature = "spantrace")]
    pub(crate) spantrace: SpanTrace,
    pub(crate) cause: Option<ErasedReport>,
    pub(crate) factors: Vec<ErasedReport>,
}

impl Context {
    #[track_caller]
    pub(crate) fn capture() -> Self {
        Self {
            annotations: Vec::new(),
            fields: Vec::new(),
            location: SourceLocation::caller(),
            backtrace: Backtrace::capture(),
            #[cfg(feature = "spantrace")]
            spantrace: SpanTrace::capture(),
            cause: None,
            factors: Vec::new(),
        }
    }

    /// The report's narrative: messages and suggestions, in the order they were added.
    #[must_use]
    pub fn annotations(&self) -> &[Annotation] {
        &self.annotations
    }

    /// Structured fields attached to the report.
    #[must_use]
    pub fn fields(&self) -> &[Field] {
        &self.fields
    }

    /// Where the report was created.
    #[must_use]
    pub fn location(&self) -> &SourceLocation {
        &self.location
    }

    /// Captured backtrace, if backtraces are enabled (`RUST_BACKTRACE=1`).
    #[must_use]
    pub fn backtrace(&self) -> &Backtrace {
        &self.backtrace
    }

    /// Whether a backtrace was actually captured.
    #[must_use]
    pub fn has_backtrace(&self) -> bool {
        self.backtrace.is_captured()
    }

    /// Captured span trace.
    #[cfg(feature = "spantrace")]
    #[must_use]
    pub fn spantrace(&self) -> &SpanTrace {
        &self.spantrace
    }

    /// Whether a span trace was actually captured.
    #[cfg(feature = "spantrace")]
    #[must_use]
    pub fn has_spantrace(&self) -> bool {
        self.spantrace.status() == SpanTraceStatus::CAPTURED
    }

    /// The one report this report was escalated from, if any.
    ///
    /// Only ever set by [`Report::escalate`](crate::Report::escalate). There is no way to
    /// attach a cause to an already-existing report, only to derive a new report from an
    /// old one, so this is never ambiguous about whether the cause was actually
    /// necessary: it always was, since escalating is what produced this report in the
    /// first place.
    #[must_use]
    pub fn cause(&self) -> Option<&ErasedReport> {
        self.cause.as_ref()
    }

    /// Independent factors attached to this report, e.g., several validation failures
    /// found at once.
    ///
    /// Unlike [`Context::cause`], a factor makes no claim that it alone was necessary or
    /// sufficient for the report to exist. It is simply attached alongside it.
    #[must_use]
    pub fn factors(&self) -> &[ErasedReport] {
        &self.factors
    }
}
