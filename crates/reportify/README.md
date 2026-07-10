# reportify

Errors are part of an API's contract, and deserve the same care as its types and
function signatures. This crate builds typed Rust errors that can still carry rich
diagnostic reports without turning every internal detail into that contract. The
`error` stays the curated, typed boundary programs branch on. Everything else, a
narrative, structured fields, a backtrace, a cause, and contributing factors, is
diagnostic detail that lives in `context` instead, regardless of who or what ends up
reading it.

```rust
use reportify::{Report, ResultExt, bail, new_whatever_type};

new_whatever_type! {
    /// Application-level error.
    pub AppError
}

fn load_config(path: &std::path::Path) -> Result<String, Report<AppError>> {
    if path.as_os_str().is_empty() {
        bail!("configuration path must not be empty");
    }

    std::fs::read_to_string(path)
        .whatever("unable to read configuration")
        .field("path", path)
}
```

A `Whatever` type such as `AppError` above never carries its own message. Every report
needs an explicit description at the call site, instead of falling back to a generic,
type-level placeholder.

`return_error!` is a `?`-like macro for a diverging function whose own return type is
already the bare error/`Report`, e.g., a server loop that only ever exits by failing.
`ResultExt::assert_ok` unwraps a result, treating an error as a bug in the program rather
than an external failure: it panics, showing the full rendered report, instead of
propagating.

## Cause and Factors

`Report::escalate` turns a report into a differently-typed one, keeping the original as its
cause. This is the only way a cause gets set: there is no way to attach a cause to an
already-existing report, only to derive a new report from an old one.

`Report::with_factor`/`Report::with_factors` attach one or more independent, contributing
factors instead, e.g., several hooks that each failed. Unlike a cause, a factor makes no
claim that it alone was necessary or sufficient.

## Capturing Panics

A panic caught with `catch_unwind` becomes a `Report<Panicked>` with a correct backtrace
and location. A bare `std::panic::catch_unwind` cannot recover either on its own, since
by the time it returns, the stack has already unwound. `Panicked` keeps the raw panic
payload, not just an extracted message, so callers can still distinguish a genuine bug
from a deliberate, non-error use of `resume_unwind`, e.g., as a cancellation signal.

Call `install_panic_hook` near the top of `main` to install `catch_unwind`'s hook
eagerly, closing a narrow race where a panic on another thread at that exact moment could
otherwise slip through uncaptured. `install_pretty_panic_hook`/
`install_pretty_panic_hook_with` additionally take over how an *uncaught* panic prints:
instead of the default hook's plain banner, it renders the same way a `Report` does,
always flagged as a bug, optionally with an issue tracker URL and extra fields like the
application's own version through `PrettyPanicOptions`.

## Logging

`ResultExt::log_error`/`log_warning`/`log_info` log a report through `tracing` at the
matching level; `ResultExt::ignore` is `log_error` with the value discarded too. The
rendered report becomes the event's message; the error's type name and code (when it has
one) are attached as separate `error.type`/`error.code` fields, so a structured
subscriber can filter or group on them without parsing the message text.

## Rendering

`Display`/`Debug` render a report as a tree, causes and factors nested under arrows.
`Report::render` takes explicit `render::RenderOptions` for plain ASCII instead of
Unicode box-drawing, forced or disabled color, or a compact view without locations.
`Report::print`/`Report::eprint` render and print directly to stdout/stderr, correcting
the color mode to check whichever stream they actually print to. A verbose backtrace
skips reportify's own frames and the runtime's startup frames, keeping only what the
caller actually wrote. `RenderOptions::wrap` word-wraps long lines to a fixed width or
the actual terminal width, off by default, so a long message/suggestion/field value
still lines up under the tree instead of soft-wrapping wherever the terminal decides. See
the `render` module's docs for what each combination looks like, or run `cargo run
--example config`.

## Export

`Report::export`/`Report::export_with` turn a report into structured data instead of
text. Public fields are exported by default. Sensitive and secret fields, and the
captured backtrace and span trace, require explicit opt-in through
`export::ExportOptions`. The backtrace and span trace export as structured frames/spans,
not rendered text, and the backtrace is the raw, unfiltered capture.

Optional features:

- `backtrace` captures backtraces through the `backtrace` crate instead of
  `std::backtrace`, enabling frame-skipped verbose rendering, and is enabled by default.
- `color` colors rendered reports through `console` and is enabled by default.
- `spantrace` captures `tracing-error` span traces and is enabled by default.
- `serde` derives `Serialize`/`Deserialize` for exported report data, with `camelCase`
  field names (enum variant tags stay Capitalized).
