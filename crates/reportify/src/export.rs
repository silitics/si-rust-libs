//! Machine-readable export of a [`Report`](crate::Report), with redaction.

use crate::annotation;
use crate::annotation::Annotation;
use crate::context::Context;
use crate::value::{Value, Visibility};

/// Options controlling machine export.
///
/// `#[non_exhaustive]`, so a new option can be added later without breaking existing
/// callers. Build one with [`ExportOptions::new`] and the `with_*` methods rather than a
/// struct literal:
///
/// ```
/// use reportify::export::ExportOptions;
///
/// let options = ExportOptions::new().with_sensitive().with_secrets();
/// assert!(options.include_sensitive);
/// assert!(options.include_secrets);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[non_exhaustive]
// Four independent, orthogonal opt-in toggles, each redacted/excluded by default; not a
// state machine, so a bitflag or enum encoding would only obscure the `with_*` builder.
#[allow(clippy::struct_excessive_bools)]
pub struct ExportOptions {
    /// Include sensitive fields.
    pub include_sensitive: bool,
    /// Include secret fields.
    pub include_secrets: bool,
    /// Include the captured backtrace, if any.
    pub include_backtrace: bool,
    /// Include the captured span trace, if any.
    pub include_spantrace: bool,
}

impl ExportOptions {
    /// Start building export options with nothing beyond public fields included.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Include sensitive fields in the export.
    #[must_use]
    pub fn with_sensitive(mut self) -> Self {
        self.include_sensitive = true;
        self
    }

    /// Include secret fields in the export.
    #[must_use]
    pub fn with_secrets(mut self) -> Self {
        self.include_secrets = true;
        self
    }

    /// Include the captured backtrace, if any, in the export.
    #[must_use]
    pub fn with_backtrace(mut self) -> Self {
        self.include_backtrace = true;
        self
    }

    /// Include the captured span trace, if any, in the export.
    #[must_use]
    pub fn with_spantrace(mut self) -> Self {
        self.include_spantrace = true;
        self
    }

    fn includes(self, visibility: Visibility) -> bool {
        match visibility {
            Visibility::Public => true,
            Visibility::Sensitive => self.include_sensitive,
            Visibility::Secret => self.include_secrets,
        }
    }
}

/// An exported annotation: part of the narrative, or a suggestion.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(tag = "kind", content = "text"))]
#[non_exhaustive]
pub enum ExportedAnnotation {
    /// Part of the causal narrative.
    Message(String),
    /// Forward-looking, actionable advice.
    Suggestion(String),
}

/// An exported, possibly redacted, field.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
pub struct ExportedField {
    /// Field key.
    pub key: String,
    /// Field value, absent when redacted.
    pub value: Option<Value>,
    /// Field visibility.
    pub visibility: Visibility,
    /// Whether the value was redacted.
    pub redacted: bool,
}

/// One frame of an exported backtrace, unfiltered: includes reportify's own frames and
/// the runtime's startup frames, unlike the frame-skipped text
/// [`Report::render`](crate::Report::render) produces. Requires the `backtrace` feature;
/// see [`ExportedReport::backtrace`].
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
pub struct ExportedFrame {
    /// The symbol name, demangled, if resolved.
    pub symbol: Option<String>,
    /// The source file, if resolved.
    pub file: Option<String>,
    /// The source line, if resolved.
    pub line: Option<u32>,
    /// The source column, if resolved.
    pub column: Option<u32>,
    /// The instruction address. Only meaningful within the process that captured it,
    /// e.g., for correlating with a symbol server; not stable across runs or builds.
    pub address: Option<u64>,
}

/// One span of an exported span trace: which `tracing` span was active, and where it was
/// entered.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
pub struct ExportedSpan {
    /// The span's name.
    pub name: String,
    /// The span's target, usually the module path it was created in.
    pub target: String,
    /// The source file the span was created in, if known.
    pub file: Option<String>,
    /// The source line the span was created at, if known.
    pub line: Option<u32>,
    /// The span's recorded fields, formatted as text: `tracing`'s own instrumentation
    /// records field values as formatted text, not structured data, so this is as
    /// structured as a span's fields can get without a custom subscriber.
    pub fields: String,
}

/// A machine-readable report, mirroring a [`Report`](crate::Report)'s cause and factors.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
pub struct ExportedReport {
    /// Error type name.
    pub error_type: String,
    /// Error code.
    pub error_code: Option<String>,
    /// Headline message: the most recent [`Message`](crate::Message) annotation, falling
    /// back to the error's own message. Usually the former. `Whatever` types never carry
    /// their own message, so this is where their description actually lives.
    pub error_message: Option<String>,
    /// The report's narrative.
    pub annotations: Vec<ExportedAnnotation>,
    /// Structured fields, redacted according to the requested [`ExportOptions`].
    pub fields: Vec<ExportedField>,
    /// Captured backtrace, as unfiltered frames, if requested through
    /// [`ExportOptions::include_backtrace`] and actually captured. Requires the
    /// `backtrace` feature; without it, always `None`.
    pub backtrace: Option<Vec<ExportedFrame>>,
    /// Captured span trace, as a list of spans, if requested through
    /// [`ExportOptions::include_spantrace`] and actually captured. Requires the
    /// `spantrace` feature; without it, always `None`.
    pub spantrace: Option<Vec<ExportedSpan>>,
    /// The report this report was escalated from, if any, recursively exported.
    pub cause: Option<Box<ExportedReport>>,
    /// Independent, contributing factors attached to this report, recursively exported.
    pub factors: Vec<ExportedReport>,
}

pub(crate) fn build(
    error_type: &'static str,
    error_message: Option<String>,
    error_code: Option<&'static str>,
    context: &Context,
    options: ExportOptions,
) -> ExportedReport {
    ExportedReport {
        error_type: error_type.to_owned(),
        error_code: error_code.map(str::to_owned),
        error_message: annotation::headline(context.annotations())
            .map(str::to_owned)
            .or(error_message),
        annotations: context
            .annotations()
            .iter()
            .map(|annotation| match annotation {
                Annotation::Message { text, .. } => ExportedAnnotation::Message(text.clone()),
                Annotation::Suggestion { text, .. } => ExportedAnnotation::Suggestion(text.clone()),
            })
            .collect(),
        fields: context
            .fields()
            .iter()
            .map(|field| {
                let include = options.includes(field.visibility);
                ExportedField {
                    key: field.key.clone(),
                    value: include.then(|| field.value.clone()),
                    visibility: field.visibility,
                    redacted: !include,
                }
            })
            .collect(),
        #[cfg(feature = "backtrace")]
        backtrace: (options.include_backtrace && context.has_backtrace())
            .then(|| context.backtrace().export_frames()),
        #[cfg(not(feature = "backtrace"))]
        backtrace: None,
        #[cfg(feature = "spantrace")]
        spantrace: (options.include_spantrace && context.has_spantrace()).then(|| {
            let mut spans = Vec::new();
            context.spantrace().with_spans(|metadata, fields| {
                spans.push(ExportedSpan {
                    name: metadata.name().to_owned(),
                    target: metadata.target().to_owned(),
                    file: metadata.file().map(str::to_owned),
                    line: metadata.line(),
                    fields: fields.to_owned(),
                });
                true
            });
            spans
        }),
        #[cfg(not(feature = "spantrace"))]
        spantrace: None,
        cause: context.cause().map(|cause| {
            Box::new(build(
                cause.error_type_name(),
                cause.error_message().map(ToString::to_string),
                cause.error_code(),
                cause.context(),
                options,
            ))
        }),
        factors: context
            .factors()
            .iter()
            .map(|factor| {
                build(
                    factor.error_type_name(),
                    factor.error_message().map(ToString::to_string),
                    factor.error_code(),
                    factor.context(),
                    options,
                )
            })
            .collect(),
    }
}
