//! Numbered backtrace/span-trace trailers, rendered after the tree.

use std::fmt::Write as _;

use crate::context::Context;
use crate::render::{Charset, ColorMode};

/// Whether a context has anything a [`Detail::Verbose`](crate::render::Detail::Verbose)
/// render should reference with a numbered marker.
pub(super) fn has_details(context: &Context) -> bool {
    context.has_backtrace() || has_spantrace(context)
}

#[cfg(feature = "spantrace")]
fn has_spantrace(context: &Context) -> bool {
    context.has_spantrace()
}

#[cfg(not(feature = "spantrace"))]
fn has_spantrace(_context: &Context) -> bool {
    false
}

/// Render the trailer for one numbered marker: the backtrace, and the span trace if any.
pub(super) fn render_trailer(
    out: &mut String,
    charset: Charset,
    color: ColorMode,
    number: usize,
    context: &Context,
) {
    let rule = match charset {
        Charset::Unicode => "━━━━",
        Charset::Ascii => "----",
    };
    let _ = writeln!(out, "\n\n{rule} BACKTRACE ({number})\n");
    if context.has_backtrace() {
        let _ = context.backtrace().write(out, color);
    }
    #[cfg(feature = "spantrace")]
    if context.has_spantrace() {
        let _ = writeln!(out, "Span trace:\n{}", context.spantrace());
    }
}
