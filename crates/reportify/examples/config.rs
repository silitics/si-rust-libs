//! Renders two different failures, one with a cause and one with independent factors,
//! through each of [`RenderOptions`]'s axes.
//!
//! Run with `cargo run --example config`. Run with `RUST_BACKTRACE=1` to see a real,
//! frame-skipped backtrace in the verbose section instead of `<no backtrace>`.

use reportify::render::{ColorMode, RenderOptions};
use reportify::{Report, Whatever, new_whatever_type};

new_whatever_type! { IoError }
new_whatever_type! { ValidationError }
new_whatever_type! { ConfigError }

/// The configuration file itself could not be read at all: a single, strict cause. There
/// is nothing left to validate, since nothing was ever parsed.
fn cause_only() -> Report<ConfigError> {
    Report::<IoError>::whatever("file not found")
        .escalate(ConfigError::new())
        .message("unable to load configuration")
        .field("path", "config.toml")
        .suggestion("create one by copying `config.example.toml` to `config.toml`")
}

/// The configuration file was read successfully, but validating its contents found two
/// independent problems: neither caused the other, and both would need fixing.
fn factors_only() -> Report<ConfigError> {
    Report::<ConfigError>::whatever("configuration is invalid")
        .field("path", "config.toml")
        .with_factors(vec![
            Report::<ValidationError>::whatever("port must be between 1 and 65535"),
            Report::<ValidationError>::whatever("unknown log level `verbos`"),
        ])
}

fn load_config() -> Report<ConfigError> {
    Report::whatever("unable to load configuration")
}

fn run() -> Report<ConfigError> {
    load_config()
}

fn main() {
    println!("-- cause_only(), default: Unicode, color, full detail --\n");
    println!(
        "{}",
        cause_only().render(RenderOptions::new().color(ColorMode::Always))
    );

    println!("-- factors_only(), default --\n");
    println!(
        "{}",
        factors_only().render(RenderOptions::new().color(ColorMode::Always))
    );

    println!("-- factors_only(), ascii(), color(Never) --\n");
    println!(
        "{}",
        factors_only().render(RenderOptions::new().ascii().color(ColorMode::Never))
    );

    println!("-- factors_only(), compact() --\n");
    println!(
        "{}",
        factors_only().render(RenderOptions::new().compact().color(ColorMode::Always))
    );

    println!("-- verbose() --\n");
    println!(
        "{}",
        run().render(RenderOptions::new().verbose().color(ColorMode::Always))
    );
}
