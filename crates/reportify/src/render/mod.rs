//! Configurable rendering of a [`Report`](crate::Report) as text.
//!
//! Used by [`Display`]/[`Debug`] with default options, and directly through
//! [`Report::render`](crate::Report::render) for a caller who wants ASCII output, a
//! compact view, or forced color. [`RenderOptions`] has three independent axes:
//!
//! - [`Charset`]: [`Unicode`](Charset::Unicode) box-drawing (the default), or plain
//!   [`Ascii`](Charset::Ascii).
//! - [`ColorMode`]: detect a terminal on
//!   [`AutoStdout`](ColorMode::AutoStdout)/[`AutoStderr`](ColorMode::AutoStderr) (the
//!   default), [`Always`](ColorMode::Always), or [`Never`](ColorMode::Never).
//! - [`Detail`]: [`Compact`](Detail::Compact) (no locations), [`Full`](Detail::Full) (the
//!   default), or [`Verbose`](Detail::Verbose) (also backtraces and span traces).
//!
//! # Examples
//!
//! Two different failures, rendered under each axis. The full runnable version lives at
//! `examples/config.rs` in the crate's repository; run it with `cargo run --example
//! config` (add `RUST_BACKTRACE=1` to see a real backtrace in the last example instead of
//! `<no backtrace>`).
//!
//! The first configuration file could not be read at all, so there is nothing left to
//! validate: a single, strict cause, labeled `cause: ` in the tree. The second was read
//! successfully, but validating its contents found two independent problems, neither
//! caused by the other: labeled `factor: ` instead.
//!
//! ```
//! use reportify::render::{ColorMode, RenderOptions};
//! use reportify::{Report, Whatever, new_whatever_type};
//!
//! new_whatever_type! { IoError }
//! new_whatever_type! { ValidationError }
//! new_whatever_type! { ConfigError }
//!
//! let cause_only = Report::<IoError>::whatever("file not found")
//!     .escalate(ConfigError::new())
//!     .message("unable to load configuration");
//! let rendered = cause_only.render(RenderOptions::new());
//! assert!(rendered.contains("cause: "));
//! assert!(!rendered.contains("factor: "));
//!
//! let factors_only = Report::<ConfigError>::whatever("configuration is invalid")
//!     .with_factors(vec![
//!         Report::<ValidationError>::whatever("port must be between 1 and 65535"),
//!         Report::<ValidationError>::whatever("unknown log level `verbos`"),
//!     ]);
//! let rendered = factors_only.render(RenderOptions::new().ascii().color(ColorMode::Never));
//! assert!(rendered.contains("factor: "));
//! assert!(!rendered.contains("cause: "));
//! assert!(!rendered.contains('├'));
//! ```
//!
//! The first report (the one with a cause), rendered with the default options (Unicode,
//! color, full detail):
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
//! The second report (the one with factors), same default options: two independent
//! factors, both `factor: `, neither claiming the other was necessary or sufficient on
//! its own:
#![doc = r#"
<pre>
<span style="color:#DD3311;"><b>configuration is invalid</b></span>
├╴at <span style="color:#888888;">crates/reportify/examples/config.rs:27:5</span>
├╴path: config.toml
│
├─▶ factor: <span style="color:#DD3311;"><b>port must be between 1 and 65535</b></span>
│   ╰╴at <span style="color:#888888;">crates/reportify/examples/config.rs:30:13</span>
│
╰─▶ factor: <span style="color:#DD3311;"><b>unknown log level `verbos`</b></span>
    ╰╴at <span style="color:#888888;">crates/reportify/examples/config.rs:31:13</span>
</pre>
"#]
//!
//! The factors report with `.ascii().color(ColorMode::Never)`, safe for a log file or a
//! terminal without Unicode/color support:
#![doc = r"
<pre>
configuration is invalid
|-at crates/reportify/examples/config.rs:27:5
|-path: config.toml
|
|-&gt; factor: port must be between 1 and 65535
|   \-at crates/reportify/examples/config.rs:30:13
|
\-&gt; factor: unknown log level `verbos`
    \-at crates/reportify/examples/config.rs:31:13
</pre>
"]
//!
//! The factors report with `.compact()`, dropping locations, keeping only the narrative
//! and the factor tree:
#![doc = r#"
<pre>
<span style="color:#DD3311;"><b>configuration is invalid</b></span>
├╴path: config.toml
│
├─▶ factor: <span style="color:#DD3311;"><b>port must be between 1 and 65535</b></span>
│
╰─▶ factor: <span style="color:#DD3311;"><b>unknown log level `verbos`</b></span>
</pre>
"#]
//!
//! A third, simpler report with `.verbose()`, adding a numbered backtrace section. The
//! frame-skip heuristic that hides reportify's own frames and the runtime's startup
//! frames is name-based, so it is not perfect: it can leave a frame or two of the
//! runtime's own startup machinery visible right before it gives up and collapses the
//! rest into a final `skipped N frames` marker, as happens here:
#![doc = r#"
<pre>
<span style="color:#DD3311;"><b>unable to load configuration</b></span>
├╴at <span style="color:#888888;">crates/reportify/examples/config.rs:36:5</span>
╰╴BACKTRACE (1)


━━━━ BACKTRACE (1)

   ⋮  skipped 3 frames

   4: <span style="color:#0099DD;">config::load_config</span> <span style="color:#888888;">(0x74178)</span>
      at crates/reportify/examples/config.rs:36:5
   5: <span style="color:#0099DD;">config::run</span> <span style="color:#888888;">(0x74405)</span>
      at crates/reportify/examples/config.rs:40:5
   6: <span style="color:#0099DD;">config::main</span> <span style="color:#888888;">(0x74db1)</span>
      at crates/reportify/examples/config.rs:71:9
   7: <span style="color:#0099DD;">core::ops::function::FnOnce::call_once</span> <span style="color:#888888;">(0x7336a)</span>
      at /home/develop/.rustup/toolchains/stable-x86_64-unknown-linux-gnu/lib/rustlib/src/rust/library/core/src/ops/function.rs:250:5

   ⋮  skipped 9 frames
</pre>
"#]

mod trailer;
mod tree;

use std::fmt::Display;

use crate::context::Context;

/// Options controlling how a report renders as text.
///
/// `#[non_exhaustive]`, so a new option can be added later without breaking existing
/// callers. Build one with [`RenderOptions::new`] and the chainable methods:
///
/// ```
/// use reportify::render::{ColorMode, RenderOptions};
///
/// let options = RenderOptions::new().ascii().compact().color(ColorMode::Never);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct RenderOptions {
    /// Which characters draw the tree.
    pub charset: Charset,
    /// Whether to color the output.
    pub color: ColorMode,
    /// How much to show.
    pub detail: Detail,
    /// Whether/how to wrap long lines.
    pub wrap: WrapMode,
}

impl Default for RenderOptions {
    fn default() -> Self {
        Self {
            charset: Charset::Unicode,
            color: ColorMode::AutoStderr,
            detail: Detail::Full,
            wrap: WrapMode::Never,
        }
    }
}

impl RenderOptions {
    /// Start building render options at the defaults: Unicode, stderr-checking automatic
    /// color detection, full detail without backtraces.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Draw the tree with plain ASCII instead of Unicode box-drawing characters.
    #[must_use]
    pub fn ascii(mut self) -> Self {
        self.charset = Charset::Ascii;
        self
    }

    /// Draw the tree with Unicode box-drawing characters. The default.
    #[must_use]
    pub fn unicode(mut self) -> Self {
        self.charset = Charset::Unicode;
        self
    }

    /// Set the character set directly.
    #[must_use]
    pub fn charset(mut self, charset: Charset) -> Self {
        self.charset = charset;
        self
    }

    /// Set the color mode.
    #[must_use]
    pub fn color(mut self, mode: ColorMode) -> Self {
        self.color = mode;
        self
    }

    /// Correct [`ColorMode::AutoStdout`]/[`ColorMode::AutoStderr`] and
    /// [`WrapMode::AutoStdout`]/[`WrapMode::AutoStderr`] to check stdout, leaving every
    /// other variant untouched. Used by [`Report::print`](crate::Report::print); also
    /// useful when rendering directly for a caller about to print to stdout.
    #[must_use]
    pub fn for_stdout(mut self) -> Self {
        if matches!(self.color, ColorMode::AutoStdout | ColorMode::AutoStderr) {
            self.color = ColorMode::AutoStdout;
        }
        if matches!(self.wrap, WrapMode::AutoStdout | WrapMode::AutoStderr) {
            self.wrap = WrapMode::AutoStdout;
        }
        self
    }

    /// Same as [`RenderOptions::for_stdout`], but for stderr. Used by
    /// [`Report::eprint`](crate::Report::eprint).
    #[must_use]
    pub fn for_stderr(mut self) -> Self {
        if matches!(self.color, ColorMode::AutoStdout | ColorMode::AutoStderr) {
            self.color = ColorMode::AutoStderr;
        }
        if matches!(self.wrap, WrapMode::AutoStdout | WrapMode::AutoStderr) {
            self.wrap = WrapMode::AutoStderr;
        }
        self
    }

    /// Show only the narrative and the cause/factor tree. No locations, no backtraces.
    #[must_use]
    pub fn compact(mut self) -> Self {
        self.detail = Detail::Compact;
        self
    }

    /// Show locations in addition to the narrative and tree, but no backtraces. The
    /// default.
    #[must_use]
    pub fn full(mut self) -> Self {
        self.detail = Detail::Full;
        self
    }

    /// Show everything, including captured backtraces and span traces.
    #[must_use]
    pub fn verbose(mut self) -> Self {
        self.detail = Detail::Verbose;
        self
    }

    /// Set the detail level directly.
    #[must_use]
    pub fn detail(mut self, detail: Detail) -> Self {
        self.detail = detail;
        self
    }

    /// Set the wrap mode. Off (the default) unless set explicitly.
    #[must_use]
    pub fn wrap(mut self, mode: WrapMode) -> Self {
        self.wrap = mode;
        self
    }
}

/// Which characters [`RenderOptions`] draws the tree with.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum Charset {
    /// Unicode box-drawing characters, e.g. `├╴`/`╰─▶`.
    Unicode,
    /// Plain ASCII, e.g. `|-`/`\->`.
    Ascii,
}

/// Whether [`RenderOptions`] colors the output.
///
/// [`ColorMode::AutoStdout`] and [`ColorMode::AutoStderr`] check different streams, since
/// a report is diagnostic output, and diagnostic output conventionally goes to stderr,
/// whether printed with `{report}`, `{report:?}` (which `main`'s own error printing
/// always goes through), or explicitly with
/// [`Report::print`](crate::Report::print)/[`Report::eprint`](crate::Report::eprint).
/// [`RenderOptions::default`] uses [`ColorMode::AutoStderr`] for exactly that reason.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum ColorMode {
    /// Color if stdout looks like a terminal that supports it. Without the `color`
    /// feature, behaves like [`ColorMode::Never`].
    AutoStdout,
    /// Color if stderr looks like a terminal that supports it. The default. Without the
    /// `color` feature, behaves like [`ColorMode::Never`].
    AutoStderr,
    /// Always color, regardless of where the output goes.
    Always,
    /// Never color.
    Never,
}

/// How much [`RenderOptions`] shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum Detail {
    /// The narrative and the cause/factor tree, without locations or backtraces.
    Compact,
    /// [`Detail::Compact`], plus where each report was created.
    Full,
    /// [`Detail::Full`], plus captured backtraces and span traces.
    Verbose,
}

/// Whether/how [`RenderOptions`] wraps long lines to fit a terminal width.
///
/// Off by default: without it, a message/suggestion/field value wider than the terminal
/// just soft-wraps however the terminal decides, with no indent under the part that
/// wrapped, since there is no newline there for reportify to attach one to. Wrapping
/// measures visible width only, ignoring both ANSI styling codes and reportify's own
/// indent/connectors, and never breaks a single word in the middle to make it fit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum WrapMode {
    /// Never wrap. The default.
    Never,
    /// Wrap to a fixed width, regardless of the actual terminal.
    Fixed(usize),
    /// Wrap to stdout's width, if it looks like a terminal. Without the `color`
    /// feature, or if stdout is not a terminal, behaves like [`WrapMode::Never`].
    AutoStdout,
    /// Wrap to stderr's width, if it looks like a terminal. Without the `color`
    /// feature, or if stderr is not a terminal, behaves like [`WrapMode::Never`].
    AutoStderr,
}

pub(crate) fn render(
    type_name: &'static str,
    message: Option<&dyn Display>,
    code: Option<&'static str>,
    context: &Context,
    options: RenderOptions,
) -> String {
    tree::render(type_name, message, code, context, options)
}

/// Apply a [`ColorMode`] to a `console` style: check stdout or stderr for
/// [`ColorMode::AutoStdout`]/[`ColorMode::AutoStderr`], or force it on or off.
#[cfg(feature = "color")]
pub(crate) fn styled<D: Display>(
    styled: console::StyledObject<D>,
    color: ColorMode,
) -> console::StyledObject<D> {
    match color {
        ColorMode::AutoStdout => styled.for_stdout(),
        ColorMode::AutoStderr => styled.for_stderr(),
        ColorMode::Always => styled.force_styling(true),
        ColorMode::Never => styled.force_styling(false),
    }
}

/// Resolve a [`WrapMode`] to a concrete column width, `None` meaning "do not wrap".
#[cfg(feature = "color")]
pub(crate) fn resolve_wrap_width(wrap: WrapMode) -> Option<usize> {
    match wrap {
        WrapMode::Never => None,
        WrapMode::Fixed(width) => Some(width),
        WrapMode::AutoStdout => console::Term::stdout()
            .size_checked()
            .map(|(_rows, columns)| columns as usize),
        WrapMode::AutoStderr => console::Term::stderr()
            .size_checked()
            .map(|(_rows, columns)| columns as usize),
    }
}

/// Resolve a [`WrapMode`] to a concrete column width, `None` meaning "do not wrap".
/// Without the `color` feature, there is no way to detect a real terminal's width, so
/// only [`WrapMode::Fixed`] has any effect.
#[cfg(not(feature = "color"))]
pub(crate) fn resolve_wrap_width(wrap: WrapMode) -> Option<usize> {
    match wrap {
        WrapMode::Fixed(width) => Some(width),
        WrapMode::Never | WrapMode::AutoStdout | WrapMode::AutoStderr => None,
    }
}
