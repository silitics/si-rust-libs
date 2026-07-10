#![cfg_attr(docsrs, feature(doc_cfg))]
#![warn(missing_docs)]
//! Typed error reports with structured diagnostic context, causes and contributing
//! factors, configurable rendering, and panic capture.
//!
//! # Overview
//!
//! Errors are part of an API's contract, and deserve the same care as its types and
//! function signatures. Propagating a single type-erased error everywhere, e.g., with
//! `anyhow`, gives callers nothing to match on: the contract at every boundary collapses
//! to "something failed". Turning every internal failure into its own explicit enum
//! variant goes too far the other way: internal implementation detail becomes a public
//! commitment, so a new failure mode deep inside a function turns into a breaking change
//! to its signature.
//!
//! This crate is built around [`Report<E>`]: a typed error, `E`, paired with everything
//! else needed to explain a failure. `E` is the contract, curated to include only what
//! callers are actually meant to handle differently, and propagated with `?`. Everything
//! else, a message, a backtrace, structured fields, a suggestion, is diagnostic detail
//! that doesn't belong in that contract, regardless of whether it ends up read by a
//! person, a log pipeline, or an automated triage system. The two stay separate.
//!
//! # Errors and Context
//!
//! A [`Report<E>`] pairs a typed `error: E`, the value a program branches on, with a
//! [`Context`]: everything else needed to explain the failure. A context holds a
//! narrative of [`Annotation`]s ([`Message`]s describing what happened,
//! [`Suggestion`]s for what to do about it), structured [`Field`]s, a captured backtrace,
//! an optional cause, and any number of contributing factors.
//!
//! Sometimes there is no meaningful specific error type. Some failures will only ever be
//! reported, never matched on. [`Whatever`] is the escape hatch for that case: a marker
//! error type, defined with [`new_whatever_type!`], that carries no data of its own and
//! is used through [`bail!`], [`whatever!`], or [`ResultExt::whatever`]. It never carries
//! an implicit message. Every report still needs an explicit description at the point of
//! failure.
//!
//! # Cause and Factors
//!
//! A cause and a contributing factor make different claims, so `reportify` keeps them
//! separate instead of treating both the same way.
//!
//! [`Report::escalate`] produces a new, differently-typed report with `self` nested
//! inside as its [`Context::cause`]. This is the usual way a failure crosses an
//! abstraction boundary, and it is the only way a cause gets set. There is no way to
//! attach a cause to an already-existing report, only to derive a new report from an old
//! one, so a cause is never ambiguous about whether it actually led to the report: it
//! did, since escalating is what produced the report in the first place.
//!
//! [`Report::with_factor`]/[`Report::with_factors`] attach one or more independent
//! [`Context::factors`] instead, e.g., every validation error found, not just the first
//! one. A factor makes no claim that it alone was necessary or sufficient, unlike a
//! cause. A report can have a cause, factors, or both.
//!
//! # Capturing Panics
//!
//! A panic caught with [`catch_unwind`] becomes a [`Report<Panicked>`], with a real
//! backtrace and location. A bare [`std::panic::catch_unwind`] cannot recover either on
//! its own, since by the time it returns, the stack has already unwound. [`Panicked`]
//! keeps the raw panic payload, not just an extracted message, so callers can still tell
//! a genuine bug apart from a deliberate, non-error use of `resume_unwind`, e.g., as a
//! cancellation signal.
//!
//! Call [`install_panic_hook`] near the top of `main`, before spawning any other threads,
//! to install [`catch_unwind`]'s hook eagerly rather than lazily on first use, closing a
//! narrow race where a panic on a different thread at that exact moment could otherwise
//! slip through uncaptured. [`install_pretty_panic_hook`] additionally takes over how an
//! *uncaught* panic prints, rendering it the same way a [`Report`] does instead of the
//! default hook's plain banner, unconditionally, even for a panic some ancestor
//! [`catch_unwind`] goes on to recover from. Unlike a regular [`Report<E>`], a panic is
//! never something the environment, configuration, or a user did wrong: it always means
//! a bug, so the rendered panic always says so, as a suggestion, alongside whatever
//! [`PrettyPanicOptions`] tells callers about reporting it, e.g., an issue tracker URL
//! or the application's own version.
//!
//! # Logging
//!
//! [`ResultExt::log_error`]/[`ResultExt::log_warning`]/[`ResultExt::log_info`] log a
//! report through `tracing` at the matching level and return the success value as an
//! `Option`, discarding the report either way; [`ResultExt::ignore`] is `log_error` with
//! the value discarded too. The rendered report becomes the event's message; the
//! error's [`Error::type_name`] and [`Error::code`] (when it has one) are attached as
//! separate `error.type`/`error.code` fields, so a structured subscriber can filter or
//! group on them without parsing the message text. `tracing` is consequently always a
//! dependency, not an opt-in feature: these are the only methods that actually consume
//! a report, rather than annotate or propagate it further.
//!
//! # Export
//!
//! [`Report::export`]/[`Report::export_with`] turn a report into an
//! [`export::ExportedReport`] for machine consumption: structured logs, an API error
//! response, whatever needs the failure as data rather than text. It mirrors the report's
//! own cause and factors. Fields marked [`Sensitive`](Visibility::Sensitive) or
//! [`Secret`](Visibility::Secret), and the captured backtrace and span trace, are all
//! excluded by default and only included if the caller opts in through
//! [`export::ExportOptions`]. Unlike [`Report::render`], the backtrace and span trace
//! stay structured data ([`export::ExportedFrame`]/[`export::ExportedSpan`]) rather than
//! pre-rendered text, and the backtrace is the raw, unfiltered capture: no frames are
//! skipped the way a rendered backtrace skips reportify's own frames.
//!
//! # Rendering
//!
//! `Display` (`{report}`) and `Debug` (`{report:?}`) render a report as a tree, causes
//! and factors nested under arrows, the way [`Report::escalate`] and
//! [`Report::with_factor`]/[`Report::with_factors`] built them. `Debug` additionally
//! shows captured backtraces and span traces.
//!
//! [`Report::render`] takes explicit [`render::RenderOptions`] for anything else: plain
//! ASCII instead of Unicode box-drawing, forced or disabled color, or a compact view
//! without locations. [`Report::print`]/[`Report::eprint`] render and print directly to
//! stdout/stderr, correcting the color mode to check whichever stream they actually print
//! to.
//!
//! A message/suggestion/field value wider than the terminal otherwise just soft-wraps
//! however the terminal decides, with no indent under the part that wrapped, since there
//! is no newline there to attach one to. [`render::RenderOptions::wrap`] fixes that by
//! wrapping ahead of time to a fixed width or the actual terminal width, off by default.
//!
//! A verbose backtrace skips reportify's own frames and the runtime's startup frames,
//! keeping only what the caller actually wrote, e.g., `inner`/`middle`/`main` rather than
//! also `Report::new`/`Context::capture` on one end and the runtime's launch machinery on
//! the other. This needs the `backtrace` feature; without it, a captured backtrace
//! renders unfiltered.
//!
//! A configuration file that could not be read at all, escalated into a higher-level
//! error with a field and a suggestion attached, renders like this:
#![doc = r#"
<pre>
<span style="color:#DD3311;"><b>unable to load configuration</b></span>
├╴at <span style="color:#888888;">crates/reportify/examples/config.rs:18:10</span>
├╴path: config.toml
├╴suggestion: <span style="color:#0099DD;">create one by copying `config.example.toml` to `config.toml`</span>
│
╰─▶ cause: <span style="color:#DD3311;"><b>file not found</b></span>
    ╰╴at <span style="color:#888888;">crates/reportify/examples/config.rs:17:5</span>
</pre>
"#]
//!
//! See the [`render`] module for ASCII/no-color output, a compact view, verbose
//! backtraces, and a report with independent factors instead of a cause.
//!
//! # Getting Started
//!
//! ```
//! use reportify::{Report, ResultExt, bail, new_whatever_type};
//!
//! new_whatever_type! {
//!     /// Application-level error.
//!     pub AppError
//! }
//!
//! fn load_config(path: &std::path::Path) -> Result<String, Report<AppError>> {
//!     if path.as_os_str().is_empty() {
//!         bail!("configuration path must not be empty");
//!     }
//!
//!     std::fs::read_to_string(path)
//!         .whatever("unable to read configuration")
//!         .field("path", path)
//! }
//! ```
//!
//! # Features
//!
//! - `backtrace` (enabled by default) captures backtraces through the `backtrace` crate
//!   instead of `std::backtrace`, so a verbose render can skip reportify's own frames and
//!   the runtime's startup frames.
//! - `color` (enabled by default) colors rendered reports through `console`, when the
//!   output looks like it is going to a terminal that supports it. Without it,
//!   [`render::ColorMode::Always`]/[`render::ColorMode::AutoStdout`]/
//!   [`render::ColorMode::AutoStderr`] behave like [`render::ColorMode::Never`].
//! - `spantrace` (enabled by default) captures a `tracing-error` span trace alongside the
//!   backtrace, so a report also shows which `tracing` spans were active when it was
//!   created.
//! - `serde` derives `Serialize` for [`export::ExportedReport`] and the other exported
//!   types, for shipping structured logs.

mod annotation;
mod backtrace;
mod context;
mod erased;
mod error;
pub mod export;
mod ext;
mod location;
mod macros;
mod panic;
pub mod render;
mod report;
#[cfg(test)]
mod tests;
mod value;

pub use annotation::{Annotation, IntoMessage, IntoSuggestion, Message, Suggestion};
pub use backtrace::Backtrace;
pub use context::Context;
pub use erased::ErasedReport;
pub use error::{Error, Whatever};
pub use ext::{ErrorExt, ResultExt};
pub use location::SourceLocation;
pub use panic::{
    Panicked, PrettyPanicOptions, catch_unwind, install_panic_hook, install_pretty_panic_hook,
    install_pretty_panic_hook_with,
};
pub use report::Report;
pub use value::{Field, Value, Visibility};

/// A result whose error is a [`Report`].
pub type Result<T, E> = std::result::Result<T, Report<E>>;
