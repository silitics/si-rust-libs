//! Annotations attached to a report's context: the causal narrative and suggestions.

use crate::location::SourceLocation;

/// An entry in a report's narrative.
///
/// Carries only text. Fields always belong to the report's [`Context`](crate::Context) as
/// a whole, never to a specific annotation. See [`Report::field`](crate::Report::field).
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[cfg_attr(feature = "serde", serde(tag = "kind"))]
pub enum Annotation {
    /// Part of the causal narrative: what was being attempted, or what happened.
    Message {
        /// Annotation text.
        text: String,
        /// Where the annotation was added.
        location: SourceLocation,
    },
    /// Forward-looking, actionable advice. Not part of the causal narrative.
    Suggestion {
        /// Annotation text.
        text: String,
        /// Where the annotation was added.
        location: SourceLocation,
    },
}

impl Annotation {
    /// Text of this annotation.
    #[must_use]
    pub fn text(&self) -> &str {
        match self {
            Self::Message { text, .. } | Self::Suggestion { text, .. } => text,
        }
    }

    /// Location where this annotation was added.
    #[must_use]
    pub fn location(&self) -> &SourceLocation {
        match self {
            Self::Message { location, .. } | Self::Suggestion { location, .. } => location,
        }
    }
}

/// The most recent [`Annotation::Message`] text, if any.
///
/// Used as the preferred headline for both rendering and export, in favor of the error's
/// own message, which is usually absent for `Whatever` types. Shared here so the two
/// can't drift apart.
pub(crate) fn headline(annotations: &[Annotation]) -> Option<&str> {
    annotations
        .iter()
        .rev()
        .find_map(|annotation| match annotation {
            Annotation::Message { text, .. } => Some(text.as_str()),
            Annotation::Suggestion { .. } => None,
        })
}

/// A message describing what was being attempted, or what happened.
///
/// Constructed by converting a value via [`IntoMessage`], implemented for `&str`/`String`
/// and for closures, so plain string messages work out of the box.
pub struct Message {
    pub(crate) text: String,
}

impl Message {
    /// Create a message.
    #[must_use]
    pub fn new(text: impl Into<String>) -> Self {
        Self { text: text.into() }
    }
}

/// Conversion into a [`Message`].
///
/// Implemented for `&str`/`String`, so `report.message("...")` works directly, and for
/// `FnOnce() -> impl IntoMessage`, so `report.message(|| format!("..."))` works too,
/// without paying for the `format!` unless the message is actually needed:
/// [`Report::message`](crate::Report::message)/
/// [`ResultExt::message`](crate::ResultExt::message)
/// only call [`IntoMessage::into_message`] once they already know there is a report to
/// attach it to.
///
/// ```
/// use reportify::{ResultExt, new_whatever_type};
///
/// new_whatever_type! { AppError }
///
/// let path = "/does/not/exist";
/// let report = std::fs::read_to_string(path)
///     .whatever::<AppError>("unable to read configuration")
///     .message(|| format!("looked in {path}")) // Only formatted if reading actually failed.
///     .unwrap_err();
/// assert!(format!("{report}").contains("looked in /does/not/exist"));
/// ```
pub trait IntoMessage {
    /// Convert `self` into a [`Message`].
    fn into_message(self) -> Message;
}

impl IntoMessage for &str {
    fn into_message(self) -> Message {
        Message::new(self)
    }
}

impl IntoMessage for String {
    fn into_message(self) -> Message {
        Message::new(self)
    }
}

impl<F, M> IntoMessage for F
where
    F: FnOnce() -> M,
    M: IntoMessage,
{
    fn into_message(self) -> Message {
        self().into_message()
    }
}

/// Forward-looking, actionable advice, e.g., "did you forget to run `x init`?".
///
/// Constructed the same way as [`Message`], via [`IntoSuggestion`].
pub struct Suggestion {
    pub(crate) text: String,
}

impl Suggestion {
    /// Create a suggestion.
    #[must_use]
    pub fn new(text: impl Into<String>) -> Self {
        Self { text: text.into() }
    }
}

/// Conversion into a [`Suggestion`].
///
/// Implemented the same way as [`IntoMessage`]: for `&str`/`String`, and for
/// `FnOnce() -> impl IntoSuggestion`, evaluated lazily only once a report actually exists
/// to attach it to.
pub trait IntoSuggestion {
    /// Convert `self` into a [`Suggestion`].
    fn into_suggestion(self) -> Suggestion;
}

impl IntoSuggestion for &str {
    fn into_suggestion(self) -> Suggestion {
        Suggestion::new(self)
    }
}

impl IntoSuggestion for String {
    fn into_suggestion(self) -> Suggestion {
        Suggestion::new(self)
    }
}

impl<F, S> IntoSuggestion for F
where
    F: FnOnce() -> S,
    S: IntoSuggestion,
{
    fn into_suggestion(self) -> Suggestion {
        self().into_suggestion()
    }
}
