//! Capturing panics as [`Report<Panicked>`].
//!
//! A bare call to [`std::panic::catch_unwind`] only ever hands back the raw payload. By
//! the time it returns, the stack has already unwound, so there is no useful
//! location or backtrace left to capture. The only place those are still available is
//! inside a panic hook, which runs synchronously on the panicking thread *before*
//! unwinding proceeds. So a global hook captures them into a thread-local the moment a
//! panic starts, and [`catch_unwind`] drains that thread-local right after
//! `std::panic::catch_unwind` returns. This is safe without synchronization, since the
//! hook always runs on the same thread, immediately before, with no possibility of
//! another panic happening in between.

use std::any::Any;
use std::cell::RefCell;
use std::fmt::{Debug, Display};
use std::panic::{PanicHookInfo, UnwindSafe};
use std::sync::{Once, OnceLock};

#[cfg(feature = "spantrace")]
use tracing_error::SpanTrace;

use crate::backtrace::Backtrace;
use crate::error::Error;
use crate::location::SourceLocation;
#[cfg(feature = "color")]
use crate::render::ColorMode;
use crate::report::Report;
use crate::value::Value;

struct PanicCapture {
    location: Option<SourceLocation>,
    backtrace: Backtrace,
    #[cfg(feature = "spantrace")]
    spantrace: SpanTrace,
    message: String,
}

thread_local! {
    static LAST_PANIC: RefCell<Option<PanicCapture>> = const { RefCell::new(None) };
}

/// Payload for a panic whose message is already a fully rendered report, e.g., from
/// [`ResultExt::assert_ok`](crate::ResultExt::assert_ok). The installed hook prints this
/// directly instead of handing it to the default hook, which would otherwise wrap it in
/// its own "thread panicked at" banner and, with `RUST_BACKTRACE` set, a second, raw,
/// unfiltered backtrace right below the one already inside the rendered text.
struct RenderedPanic(String);

/// Panic with an already-rendered message, formatted as cleanly as an uncaught panic can
/// be: printed as-is by the installed hook, with none of the default hook's own
/// wrapping.
#[track_caller]
pub(crate) fn panic_rendered(text: String) -> ! {
    ensure_hook_installed();
    std::panic::panic_any(RenderedPanic(text))
}

/// Marker error type for an ordinary, uncaught panic rendered by
/// [`install_pretty_panic_hook`]. Never exposed: built, rendered, and discarded within
/// the hook itself.
struct UncaughtPanic;

impl Debug for UncaughtPanic {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("UncaughtPanic").finish()
    }
}

impl Error for UncaughtPanic {
    fn message(&self) -> Option<&dyn Display> {
        None
    }

    fn type_name(&self) -> &'static str {
        "panic"
    }
}

/// Options for [`install_pretty_panic_hook_with`].
///
/// `#[non_exhaustive]`, so a new option can be added later without breaking existing
/// callers. Build one with [`PrettyPanicOptions::new`] and the `with_*` methods rather
/// than a struct literal.
///
/// ```
/// use reportify::PrettyPanicOptions;
///
/// let options = PrettyPanicOptions::new()
///     .with_report_to("https://github.com/example/example/issues/new")
///     .with_field("version", env!("CARGO_PKG_VERSION"));
/// ```
#[derive(Debug, Clone, Default)]
#[non_exhaustive]
pub struct PrettyPanicOptions {
    report_to: Option<String>,
    fields: Vec<(String, Value)>,
}

impl PrettyPanicOptions {
    /// Start building options with no reporting instructions or extra fields.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Where to report a panic as a bug, e.g., an issue tracker URL. Shown as a
    /// suggestion alongside every rendered panic.
    #[must_use]
    pub fn with_report_to(mut self, report_to: impl Into<String>) -> Self {
        self.report_to = Some(report_to.into());
        self
    }

    /// Attach information useful for a bug report, e.g., the application's own version
    /// (reportify only knows its own), shown as a field alongside every rendered panic.
    #[must_use]
    pub fn with_field(mut self, key: impl Into<String>, value: impl Into<Value>) -> Self {
        self.fields.push((key.into(), value.into()));
        self
    }
}

/// Render an ordinary (non-[`RenderedPanic`]) panic the same way a [`Report`] renders,
/// for [`install_pretty_panic_hook`]. Captures its own backtrace/span trace independently
/// of the ones captured for `catch_unwind`'s benefit, rather than sharing them, since
/// `std::backtrace::Backtrace` (used without the `backtrace` feature) is not `Clone`;
/// panicking is already the slow, exceptional path, so capturing twice is a fine trade
/// for not threading a shared, cloneable capture through the rest of the crate.
fn render_uncaught_panic(
    info: &PanicHookInfo<'_>,
    message: &str,
    options: &PrettyPanicOptions,
) -> String {
    let location = info
        .location()
        .map_or_else(SourceLocation::caller, SourceLocation::from_std);
    let report = Report::from_capture(
        UncaughtPanic,
        location,
        Backtrace::force_capture(),
        #[cfg(feature = "spantrace")]
        SpanTrace::capture(),
    )
    .message(message.to_owned());
    append_pretty_panic_footer(format!("{report:?}"), options)
}

/// Append `options`' fields and `report_to` suggestion as a plain-text footer after
/// `rendered`, below a rule divider (matching the crate's own backtrace-trailer style).
///
/// Kept out of the report tree entirely, rather than attached as a field/suggestion on
/// whatever report triggered the panic: they describe the panicking *process* as a whole
/// (its version, where to report bugs), not that specific report, and attaching them to
/// it would misleadingly suggest otherwise, e.g., on
/// [`ResultExt::assert_ok`](crate::ResultExt::assert_ok)'s report, which already has its
/// own, more specific cause chain.
pub(crate) fn append_pretty_panic_footer(
    mut rendered: String,
    options: &PrettyPanicOptions,
) -> String {
    use std::fmt::Write as _;

    let _ = write!(rendered, "\n\n━━━━\n\n");
    for (key, value) in &options.fields {
        let _ = writeln!(rendered, "{key}: {value}");
    }
    let suggestion = match &options.report_to {
        Some(report_to) => {
            format!("this indicates a bug in the program; please report it at {report_to}")
        }
        None => "this indicates a bug in the program".to_owned(),
    };
    rendered.push_str(&style_footer_suggestion(&suggestion));
    rendered
}

#[cfg(feature = "color")]
fn style_footer_suggestion(text: &str) -> String {
    crate::render::styled(console::style(text).cyan(), ColorMode::AutoStderr).to_string()
}

#[cfg(not(feature = "color"))]
fn style_footer_suggestion(text: &str) -> String {
    text.to_owned()
}

/// Append the globally configured fields and `report_to` suggestion (set via
/// [`install_pretty_panic_hook_with`]), if any were configured, as a plain-text footer
/// after `rendered`. A no-op if pretty panic printing was never installed. Used to give a
/// panic built from an already-existing [`Report`] (e.g.
/// [`ResultExt::assert_ok`](crate::ResultExt::assert_ok)) the same footer an ordinary
/// uncaught panic gets under [`install_pretty_panic_hook`].
pub(crate) fn decorate_with_pretty_panic_options(rendered: String) -> String {
    match PRETTY_PRINT.get() {
        Some(options) => append_pretty_panic_footer(rendered, options),
        None => rendered,
    }
}

static PRETTY_PRINT: OnceLock<PrettyPanicOptions> = OnceLock::new();

/// Install the panic-capturing hook, if it isn't already installed.
///
/// Chains to whatever hook was previously installed, rather than replacing it. reportify
/// is meant to be composed as a library dependency, not to assume it owns `main`, so
/// something else, e.g., a test harness or another diagnostics crate, may have its own
/// hook that should still run.
fn ensure_hook_installed() {
    static INSTALLED: Once = Once::new();
    INSTALLED.call_once(|| {
        let previous = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info: &PanicHookInfo<'_>| {
            let rendered = info.payload().downcast_ref::<RenderedPanic>();
            let message = rendered.map_or_else(
                || {
                    info.payload_as_str()
                        .map_or_else(|| "thread panicked".to_owned(), str::to_owned)
                },
                |rendered| rendered.0.clone(),
            );

            // Not `.map(|options| ...)`/`.then(|| ...)`: a closure passed to either is a
            // real, uninlined call in a debug build, and its own frame does not contain
            // "reportify", which would trip up `backtrace::render_skipped`'s
            // frame-skipping into ending the "reportify" segment one frame too early
            // (confirmed empirically, not just by inspection: `bool::then` did exactly
            // this during development).
            #[allow(clippy::manual_map)]
            let pretty = if rendered.is_none() {
                match PRETTY_PRINT.get() {
                    Some(options) => Some(render_uncaught_panic(info, &message, options)),
                    None => None,
                }
            } else {
                None
            };

            let capture = PanicCapture {
                location: info.location().map(SourceLocation::from_std),
                backtrace: Backtrace::force_capture(),
                #[cfg(feature = "spantrace")]
                spantrace: SpanTrace::capture(),
                message,
            };
            LAST_PANIC.with(|cell| *cell.borrow_mut() = Some(capture));

            match (rendered, pretty) {
                (Some(rendered), _) => eprintln!("{}", rendered.0),
                (None, Some(text)) => eprintln!("{text}"),
                (None, None) => previous(info),
            }
        }));
    });
}

/// A caught panic.
///
/// Keeps the raw payload, not just an extracted message, so callers can still tell a
/// genuine bug apart from a deliberate, non-error use of `resume_unwind` as a
/// cancellation signal. Downcast [`Panicked::payload`] to check, and
/// [`Panicked::resume_unwind`] to let it keep propagating rather than treating it as an
/// error.
///
/// Never carries its own message (consistent with [`Whatever`](crate::Whatever) types in
/// general): the extracted panic message is attached to the report as an annotation by
/// [`catch_unwind`], not baked into this type.
pub struct Panicked {
    payload: Box<dyn Any + Send>,
}

impl Panicked {
    /// The raw panic payload.
    #[must_use]
    pub fn payload(&self) -> &(dyn Any + Send) {
        &*self.payload
    }

    /// Resume unwinding with the original payload, rather than treating this as an error.
    pub fn resume_unwind(self) -> ! {
        std::panic::resume_unwind(self.payload)
    }
}

impl Debug for Panicked {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Panicked").finish_non_exhaustive()
    }
}

impl Error for Panicked {
    fn message(&self) -> Option<&dyn Display> {
        None
    }
}

/// Install reportify's panic-capturing hook now, rather than waiting for the first call
/// to [`catch_unwind`].
///
/// The hook chains onto whatever was already installed, via [`std::panic::take_hook`]
/// then [`std::panic::set_hook`]. Between those two calls there is a brief window with
/// only the default hook installed: a panic on a different thread during that exact
/// window runs through the default hook instead, missing both the previous hook's
/// behavior and reportify's own capture. `std::panic::update_hook` would close this
/// window atomically, but it is still nightly-only.
///
/// Calling `install_panic_hook` once, near the top of `main`, before spawning any other
/// threads, closes the window in practice, since nothing else can be panicking
/// concurrently yet. Safe to call more than once, and safe to skip entirely: it happens
/// automatically the first time [`catch_unwind`] actually needs it.
///
/// ```
/// use reportify::{catch_unwind, install_panic_hook};
///
/// install_panic_hook();
///
/// let result = catch_unwind(|| 1 + 1);
/// assert_eq!(result.unwrap(), 2);
/// ```
pub fn install_panic_hook() {
    ensure_hook_installed();
}

/// Install reportify's panic hook, additionally taking over how an uncaught panic
/// prints: instead of the default hook's plain "thread panicked at" banner, an ordinary
/// panic (`.unwrap()`, `todo!()`, a bare `panic!(...)`, ...) renders the same way a
/// [`Report`] does: a styled headline, its location, and a frame-skipped
/// backtrace/span trace if one was captured.
///
/// Prints unconditionally, even for a panic an ancestor [`catch_unwind`] goes on to
/// recover from: the hook cannot know in advance whether that will happen, so a panic
/// worth investigating is surfaced immediately rather than only if and however the
/// recovering caller decides to handle the resulting `Report`. `color-eyre`'s own panic
/// hook makes the same tradeoff. If that is not what you want, e.g., because you rely on
/// `catch_unwind` to silently recover from panics inside isolated, per-request work, use
/// [`install_panic_hook`] instead.
///
/// Same eager-install rationale as [`install_panic_hook`]: call this once, near the top
/// of `main`, before spawning any other threads. Shorthand for
/// [`install_pretty_panic_hook_with`] with the default, empty [`PrettyPanicOptions`]; use
/// that instead to point at an issue tracker or attach fields like the application's
/// own version.
///
/// ```
/// use reportify::{catch_unwind, install_pretty_panic_hook};
///
/// install_pretty_panic_hook();
///
/// let result = catch_unwind(|| 1 + 1);
/// assert_eq!(result.unwrap(), 2);
/// ```
pub fn install_pretty_panic_hook() {
    install_pretty_panic_hook_with(PrettyPanicOptions::default());
}

/// Like [`install_pretty_panic_hook`], with [`PrettyPanicOptions`] to point at an issue
/// tracker or attach fields, e.g., the application's own version, to every rendered
/// panic.
///
/// Only the first call's options take effect; later calls still install the hook (if it
/// is not installed yet) but do not replace already-set options.
///
/// ```
/// use reportify::{PrettyPanicOptions, catch_unwind, install_pretty_panic_hook_with};
///
/// install_pretty_panic_hook_with(
///     PrettyPanicOptions::new()
///         .with_report_to("https://github.com/example/example/issues/new")
///         .with_field("version", env!("CARGO_PKG_VERSION")),
/// );
///
/// let result = catch_unwind(|| 1 + 1);
/// assert_eq!(result.unwrap(), 2);
/// ```
pub fn install_pretty_panic_hook_with(options: PrettyPanicOptions) {
    ensure_hook_installed();
    let _ = PRETTY_PRINT.set(options);
}

/// Run `f`, catching a panic (if any) as a [`Report<Panicked>`] with a correct backtrace
/// and location, instead of the ones `std::panic::catch_unwind` alone can recover.
///
/// # Errors
///
/// Returns a [`Report<Panicked>`] if `f` panics.
///
/// ```
/// use reportify::catch_unwind;
///
/// match catch_unwind(|| 1 + 1) {
///     Ok(value) => assert_eq!(value, 2),
///     Err(report) => panic!("unexpected panic: {report}"),
/// }
/// ```
pub fn catch_unwind<F, T>(f: F) -> Result<T, Report<Panicked>>
where
    F: FnOnce() -> T + UnwindSafe,
{
    ensure_hook_installed();
    match std::panic::catch_unwind(f) {
        Ok(value) => Ok(value),
        Err(payload) => {
            let capture = LAST_PANIC.with(|cell| cell.borrow_mut().take());
            let message = capture.as_ref().map_or_else(
                || "thread panicked".to_owned(),
                |capture| capture.message.clone(),
            );
            let report = match capture {
                Some(capture) => Report::from_capture(
                    Panicked { payload },
                    capture.location.unwrap_or_else(SourceLocation::caller),
                    capture.backtrace,
                    #[cfg(feature = "spantrace")]
                    capture.spantrace,
                ),
                // Should not normally happen (our hook is always installed above), but
                // fall back to a fresh capture rather than panicking ourselves.
                None => Report::new(Panicked { payload }),
            };
            Err(report.message(message))
        }
    }
}
