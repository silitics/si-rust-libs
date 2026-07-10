//! A captured backtrace, rendered with reportify's own frames skipped.
//!
//! Frame-skipping needs per-frame symbol access that `std::backtrace::Backtrace` does not
//! expose on stable Rust, so a capture goes through the `backtrace` crate instead, behind
//! the `backtrace` feature (enabled by default). Without it, a capture falls back to
//! `std::backtrace::Backtrace` and renders unfiltered.

use std::fmt;
#[cfg(feature = "backtrace")]
use std::fmt::Write as _;

use crate::render::ColorMode;
#[cfg(all(feature = "backtrace", feature = "color"))]
use crate::render::styled;

/// A captured backtrace. See [`Context::backtrace`](crate::Context::backtrace).
pub struct Backtrace(Repr);

#[cfg(feature = "backtrace")]
type Repr = Option<backtrace::Backtrace>;
#[cfg(not(feature = "backtrace"))]
type Repr = std::backtrace::Backtrace;

impl Backtrace {
    #[cfg(feature = "backtrace")]
    pub(crate) fn capture() -> Self {
        Self(if enabled() {
            Some(backtrace::Backtrace::new())
        } else {
            None
        })
    }

    #[cfg(not(feature = "backtrace"))]
    pub(crate) fn capture() -> Self {
        Self(std::backtrace::Backtrace::capture())
    }

    #[cfg(feature = "backtrace")]
    pub(crate) fn force_capture() -> Self {
        Self(Some(backtrace::Backtrace::new()))
    }

    #[cfg(not(feature = "backtrace"))]
    pub(crate) fn force_capture() -> Self {
        Self(std::backtrace::Backtrace::force_capture())
    }

    /// Whether a backtrace was actually captured (`RUST_BACKTRACE=1`, or forced).
    #[cfg(feature = "backtrace")]
    #[must_use]
    pub fn is_captured(&self) -> bool {
        self.0.is_some()
    }

    /// Whether a backtrace was actually captured (`RUST_BACKTRACE=1`, or forced).
    #[cfg(not(feature = "backtrace"))]
    #[must_use]
    pub fn is_captured(&self) -> bool {
        matches!(self.0.status(), std::backtrace::BacktraceStatus::Captured)
    }

    /// Render this backtrace: with the `backtrace` feature, reportify's own frames and
    /// the runtime's startup frames are skipped; without it, the capture renders
    /// unfiltered.
    #[cfg(feature = "backtrace")]
    pub(crate) fn write(&self, out: &mut String, color: ColorMode) -> fmt::Result {
        render_skipped(out, self.0.as_ref(), color)
    }

    /// Render this backtrace: with the `backtrace` feature, reportify's own frames and
    /// the runtime's startup frames are skipped; without it, the capture renders
    /// unfiltered.
    #[cfg(not(feature = "backtrace"))]
    pub(crate) fn write(&self, out: &mut String, _color: ColorMode) -> fmt::Result {
        use std::fmt::Write as _;
        writeln!(out, "{}", self.0)
    }

    /// Every captured frame, unfiltered, unlike [`Backtrace::write`]'s frame-skipped
    /// text: includes reportify's own frames and the runtime's startup frames, for a
    /// machine consumer that wants to do its own filtering.
    #[cfg(feature = "backtrace")]
    pub(crate) fn export_frames(&self) -> Vec<crate::export::ExportedFrame> {
        use crate::export::ExportedFrame;

        let Some(backtrace) = self.0.as_ref() else {
            return Vec::new();
        };

        let mut exported = Vec::new();
        for frame in backtrace.frames() {
            let symbols = frame.symbols();
            if symbols.is_empty() {
                exported.push(ExportedFrame {
                    symbol: None,
                    file: None,
                    line: None,
                    column: None,
                    address: Some(frame.ip() as usize as u64),
                });
            } else {
                for symbol in symbols {
                    exported.push(ExportedFrame {
                        symbol: symbol.name().map(|name| format!("{name:#}")),
                        file: symbol
                            .filename()
                            .map(|file| file.to_string_lossy().into_owned()),
                        line: symbol.lineno(),
                        column: symbol.colno(),
                        address: symbol.addr().map(|addr| addr as usize as u64),
                    });
                }
            }
        }
        exported
    }
}

impl fmt::Debug for Backtrace {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("Backtrace")
            .field(&self.is_captured())
            .finish()
    }
}

/// Whether backtrace capturing is enabled, mirroring the standard library's own
/// `RUST_LIB_BACKTRACE`/`RUST_BACKTRACE` check. Cached, since querying the environment on
/// every capture would be slow.
#[cfg(feature = "backtrace")]
fn enabled() -> bool {
    use std::sync::atomic::{AtomicU8, Ordering};

    static ENABLED: AtomicU8 = AtomicU8::new(0);
    match ENABLED.load(Ordering::Relaxed) {
        0 => {}
        1 => return false,
        _ => return true,
    }
    let enabled = match std::env::var("RUST_LIB_BACKTRACE") {
        Ok(value) => value != "0",
        Err(_) => match std::env::var("RUST_BACKTRACE") {
            Ok(value) => value != "0",
            Err(_) => false,
        },
    };
    ENABLED.store(u8::from(enabled) + 1, Ordering::Relaxed);
    enabled
}

/// Which frames of a raw capture belong to reportify's own machinery versus the caller's
/// code, tracked while walking frames innermost-first.
#[cfg(feature = "backtrace")]
#[derive(Clone, Copy)]
enum Segment {
    /// Frames captured before we have seen any reportify frame at all, e.g., the
    /// `backtrace` crate's own capture machinery. Always skipped.
    Init,
    /// reportify's own frames: `Backtrace::capture`, `Context::capture`, `Report::new`,
    /// and so on. Skipped, with a marker for how many were.
    Reportify,
    /// The caller's frames, printed in full until the runtime's startup frames are
    /// reached.
    User,
}

#[cfg(feature = "backtrace")]
fn render_skipped(
    out: &mut String,
    backtrace: Option<&backtrace::Backtrace>,
    color: ColorMode,
) -> fmt::Result {
    let Some(backtrace) = backtrace else {
        return writeln!(out, "<no backtrace>");
    };

    let mut segment = Segment::Init;
    let frames = backtrace.frames();
    'frame: for (frame_idx, frame) in frames.iter().enumerate() {
        let symbols = frame.symbols();
        for symbol in symbols {
            let Some(name) = symbol.name() else {
                continue;
            };
            let name = name.to_string();
            match segment {
                Segment::Init => {
                    if name.starts_with("reportify") {
                        segment = Segment::Reportify;
                    }
                    continue 'frame;
                }
                Segment::Reportify => {
                    if name.contains("reportify") || is_panic_machinery(&name) {
                        continue 'frame;
                    }
                    segment = Segment::User;
                    writeln!(
                        out,
                        "   ⋮  skipped {} frames\n",
                        frame_idx.saturating_sub(1)
                    )?;
                }
                Segment::User => {
                    if name.starts_with("std::sys::backtrace::__rust_begin_short_backtrace") {
                        writeln!(out, "\n   ⋮  skipped {} frames", frames.len() - frame_idx)?;
                        break 'frame;
                    }
                }
            }
        }

        if symbols.is_empty() {
            writeln!(out, "{frame_idx:>4}: {:?}", frame.ip())?;
        } else {
            for (symbol_idx, symbol) in symbols.iter().enumerate() {
                if symbol_idx == 0 {
                    write!(out, "{frame_idx:>4}:")?;
                } else {
                    write!(out, "     ")?;
                }
                if let Some(name) = symbol.name() {
                    write!(out, " {}", style_symbol(color, &format!("{name:#}")))?;
                }
                if let Some(addr) = symbol.addr() {
                    write!(out, " {}", style_address(color, &format!("({addr:?})")))?;
                }
                if let Some(file) = symbol.filename() {
                    write!(out, "\n      at {}", file.to_string_lossy())?;
                    if let Some(line) = symbol.lineno() {
                        write!(out, ":{line}")?;
                        if let Some(column) = symbol.colno() {
                            write!(out, ":{column}")?;
                        }
                    }
                }
                writeln!(out)?;
            }
        }
    }
    Ok(())
}

/// Whether a frame belongs to Rust's own panic machinery rather than to reportify or the
/// caller: the frames between a panic hook and the code that actually panicked, e.g., for
/// [`install_pretty_panic_hook`](crate::install_pretty_panic_hook), which captures a
/// backtrace from directly inside the hook. Skipped the same way reportify's own frames
/// are, since they carry no more information than "a panic happened here."
///
/// Matches on the distinctive tail of each symbol, not a `std::`/`core::`-prefixed path:
/// with v0 mangling, a crate's disambiguator hash sometimes lands *inside* the path
/// (`std[2b826ac..]::panicking::panic_fmt` rather than `std::panicking::panic_fmt`),
/// so anchoring on the crate name itself misses these frames entirely (confirmed by
/// dumping raw symbol names from a real captured backtrace, not by inspection alone).
#[cfg(feature = "backtrace")]
fn is_panic_machinery(name: &str) -> bool {
    name.contains("panicking::")
        || name.contains("rust_begin_unwind")
        || name.contains("__rust_end_short_backtrace")
        || name.contains("PanicHookInfo")
}

#[cfg(all(feature = "backtrace", feature = "color"))]
fn style_symbol(color: ColorMode, text: &str) -> String {
    styled(console::style(text).cyan(), color).to_string()
}

#[cfg(all(feature = "backtrace", not(feature = "color")))]
fn style_symbol(_color: ColorMode, text: &str) -> String {
    text.to_owned()
}

#[cfg(all(feature = "backtrace", feature = "color"))]
fn style_address(color: ColorMode, text: &str) -> String {
    styled(console::style(text).dim(), color).to_string()
}

#[cfg(all(feature = "backtrace", not(feature = "color")))]
fn style_address(_color: ColorMode, text: &str) -> String {
    text.to_owned()
}
