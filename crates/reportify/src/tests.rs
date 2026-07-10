use std::panic::AssertUnwindSafe;

use crate::export::ExportOptions;
use crate::render::{ColorMode, RenderOptions, WrapMode};
use crate::{
    ErrorExt, Report, ResultExt, Value, Visibility, catch_unwind, install_pretty_panic_hook,
    new_whatever_type,
};

new_whatever_type! {
    /// Test error.
    pub TestError
}

new_whatever_type! {
    pub OtherError
}

#[derive(Debug, thiserror::Error)]
#[error("source failed")]
struct SourceError;

fn bail_example() -> crate::Result<(), TestError> {
    crate::bail!("something failed");
}

#[test]
fn report_size_is_one_pointer_regardless_of_error_size() {
    #[derive(Debug)]
    #[allow(dead_code)]
    struct LargeError([u8; 512]);

    impl crate::Error for LargeError {
        fn message(&self) -> Option<&dyn std::fmt::Display> {
            None
        }
    }

    let pointer_size = std::mem::size_of::<usize>();
    assert_eq!(std::mem::size_of::<Report<TestError>>(), pointer_size);
    assert_eq!(std::mem::size_of::<Report<LargeError>>(), pointer_size);
}

#[test]
fn bail_captures_message() {
    let report = bail_example().expect_err("example must fail");
    assert!(format!("{report}").contains("something failed"));
}

#[test]
fn whatever_type_never_carries_its_own_message() {
    use crate::Error;
    assert!(TestError(()).message().is_none());
}

#[test]
fn result_whatever_preserves_source_as_cause() {
    let report = std::result::Result::<(), SourceError>::Err(SourceError)
        .whatever::<TestError>("unable to do work")
        .expect_err("example must fail");
    let cause = report.context().cause().expect("must have a cause");
    assert!(cause.downcast_error::<SourceError>().is_some());
    assert_eq!(
        cause.error_message().map(ToString::to_string).as_deref(),
        Some("source failed")
    );
}

#[test]
fn error_ext_report_wraps_bare_error() {
    let report = SourceError.report();
    assert!(report.context().cause().is_none());
    assert!(report.context().factors().is_empty());
}

// `ResultExt`'s methods all go through `Result::map_err`-shaped call chains at some
// point. A `#[track_caller]` function loses caller tracking when invoked through
// `map_err`'s generic `FnOnce` (the call happens inside `map_err`'s own body, not
// literally at the call site), silently falling back to `ext.rs`'s or even a std
// internal's own location instead. These pin every `ResultExt` method to the real call
// site, right below where `expected_line` is captured, so a future edit reintroducing a
// bare `map_err` fails loudly instead of silently mis-attributing locations.

#[test]
fn result_ext_report_captures_the_real_caller_location() {
    let result: std::result::Result<(), SourceError> = Err(SourceError);
    let expected_line = line!() + 1;
    let report = result.report().expect_err("must fail");
    assert_eq!(report.context().location().file, file!());
    assert_eq!(report.context().location().line, expected_line);
}

#[test]
fn result_ext_escalate_captures_the_real_caller_location() {
    let result: std::result::Result<(), SourceError> = Err(SourceError);
    let expected_line = line!() + 1;
    let report = result.escalate(TestError(())).expect_err("must fail");
    assert_eq!(report.context().location().file, file!());
    assert_eq!(report.context().location().line, expected_line);
}

#[test]
fn result_ext_whatever_captures_the_real_caller_location() {
    let result: std::result::Result<(), SourceError> = Err(SourceError);
    let expected_line = line!() + 1;
    let result = result.whatever::<TestError>("wrapped");
    let report = result.expect_err("must fail");
    assert_eq!(report.context().location().file, file!());
    assert_eq!(report.context().location().line, expected_line);
}

#[test]
fn result_ext_message_captures_the_real_caller_location() {
    let result: std::result::Result<(), SourceError> = Err(SourceError);
    let expected_line = line!() + 1;
    let report = result.message("more context").expect_err("must fail");
    assert_eq!(report.context().location().file, file!());
    assert_eq!(report.context().location().line, expected_line);
}

#[test]
fn message_and_suggestion_accept_lazy_closures() {
    let evaluated = std::cell::Cell::new(false);
    let ok_result: std::result::Result<(), SourceError> = Ok(());
    let ok_result = ok_result.message(|| {
        evaluated.set(true);
        "should not run"
    });
    assert!(ok_result.is_ok());
    assert!(!evaluated.get(), "closure must not run for an Ok result");

    let attempt = 2;
    let err_result: std::result::Result<(), SourceError> = Err(SourceError);
    let report = err_result
        .message(|| format!("attempt {attempt} failed"))
        .expect_err("must fail")
        .suggestion(|| "try again later");
    let annotations = report.context().annotations();
    assert_eq!(annotations[0].text(), "attempt 2 failed");
    assert_eq!(annotations[1].text(), "try again later");
}

#[test]
fn result_ext_field_captures_the_real_caller_location() {
    let result: std::result::Result<(), SourceError> = Err(SourceError);
    let expected_line = line!() + 1;
    let report = result.field("k", "v").expect_err("must fail");
    assert_eq!(report.context().location().file, file!());
    assert_eq!(report.context().location().line, expected_line);
}

#[test]
fn field_display_and_field_debug_format_the_value() {
    let addr: std::net::SocketAddr = "127.0.0.1:8080".parse().expect("valid socket address");
    let report = Report::<TestError>::whatever("failed")
        .field_display("addr", addr)
        .field_debug("timeout", std::time::Duration::from_secs(5));
    let fields = report.context().fields();
    assert_eq!(fields[0].value.to_string(), "127.0.0.1:8080");
    assert_eq!(fields[1].value.to_string(), "5s");
}

#[test]
fn result_ext_field_display_and_field_debug_work_on_both_impls() {
    let addr: std::net::SocketAddr = "127.0.0.1:8080".parse().expect("valid socket address");
    let timeout = std::time::Duration::from_secs(5);

    let bare: std::result::Result<(), SourceError> = Err(SourceError);
    let report = bare
        .field_display("addr", addr)
        .field_debug("timeout", timeout)
        .expect_err("must fail");
    assert_eq!(
        report.context().fields()[0].value.to_string(),
        "127.0.0.1:8080"
    );
    assert_eq!(report.context().fields()[1].value.to_string(), "5s");

    let already: crate::Result<(), TestError> = Err(Report::whatever("inner"));
    let report = already
        .field_display("addr", addr)
        .field_debug("timeout", timeout)
        .expect_err("must fail");
    assert_eq!(
        report.context().fields()[0].value.to_string(),
        "127.0.0.1:8080"
    );
    assert_eq!(report.context().fields()[1].value.to_string(), "5s");
}

#[test]
fn result_ext_assert_ok_returns_the_value_when_ok() {
    let result: std::result::Result<i32, SourceError> = Ok(42);
    assert_eq!(result.assert_ok("value expected"), 42);

    let result: crate::Result<i32, TestError> = Ok(42);
    assert_eq!(result.assert_ok("value expected"), 42);
}

#[test]
fn result_ext_assert_ok_captures_the_real_caller_location_and_shows_the_report_when_err() {
    let result: std::result::Result<(), SourceError> = Err(SourceError);
    let expected_line = line!() + 1;
    let outcome = catch_unwind(AssertUnwindSafe(|| result.assert_ok("must always succeed")));
    let panic_report = outcome.expect_err("must panic");
    assert_eq!(panic_report.context().location().file, file!());
    assert_eq!(panic_report.context().location().line, expected_line);
    assert!(format!("{panic_report}").contains("must always succeed"));
    assert!(format!("{panic_report}").contains("source failed"));
}

#[test]
fn result_ext_on_report_assert_ok_captures_the_real_caller_location_and_shows_the_report() {
    let original_line = line!() + 1;
    let result: crate::Result<(), TestError> = Err(Report::whatever("must not happen"));
    let expected_line = line!() + 1;
    let outcome = catch_unwind(AssertUnwindSafe(|| result.assert_ok("must always succeed")));
    let panic_report = outcome.expect_err("must panic");
    assert_eq!(panic_report.context().location().file, file!());
    assert_eq!(panic_report.context().location().line, expected_line);
    assert!(format!("{panic_report}").contains("must always succeed"));
    assert!(format!("{panic_report}").contains("must not happen"));
    // The rendered text embedded in `panic_report`'s message must show two distinct
    // locations: where the original report was created, and where `assert_ok` itself
    // was called (i.e., where the invariant was assumed) - not just the latter, which
    // `panic_report.context().location()` above already captures via a different route
    // (`#[track_caller]` on `panic_any` itself, unrelated to escalating the report).
    let rendered = format!("{panic_report}");
    assert!(rendered.contains(&format!("{}:{original_line}", file!())));
    assert!(rendered.contains(&format!("{}:{expected_line}", file!())));
}

// Tested directly against `apply_pretty_panic_options`, not through the
// `install_pretty_panic_hook_with`/`PRETTY_PRINT` global: that `OnceLock` can only ever
// be set once for the whole test binary, so a test relying on it would be dependent on
// whichever test happens to set it first.
#[test]
fn assert_ok_panic_includes_report_to_suggestion_and_fields_as_a_footer() {
    let report = Report::<TestError>::whatever("must not happen");
    let rendered = format!("{report:?}");
    let options = crate::PrettyPanicOptions::new()
        .with_report_to("https://example.com/issues/new")
        .with_field("version", "1.2.3");
    let footed = crate::panic::append_pretty_panic_footer(rendered.clone(), &options);
    // The footer is appended after the report's own rendering, not attached to it.
    assert!(footed.starts_with(&rendered));
    assert!(footed.contains("please report it at https://example.com/issues/new"));
    assert!(footed.contains("version: 1.2.3"));
}

#[test]
fn result_ext_assert_ok_evaluates_invariant_lazily() {
    let evaluated = std::cell::Cell::new(false);
    let result: crate::Result<i32, TestError> = Ok(42);
    let value = result.assert_ok(|| {
        evaluated.set(true);
        "should not run"
    });
    assert_eq!(value, 42);
    assert!(!evaluated.get(), "invariant must not be evaluated when Ok");
}

#[test]
fn result_ext_on_report_escalate_captures_the_real_caller_location() {
    let result: crate::Result<(), TestError> = Err(Report::whatever("inner"));
    let expected_line = line!() + 1;
    let report = result.escalate(OtherError(())).expect_err("must fail");
    assert_eq!(report.context().location().file, file!());
    assert_eq!(report.context().location().line, expected_line);
}

#[test]
fn result_ext_on_report_whatever_captures_the_real_caller_location() {
    let result: crate::Result<(), TestError> = Err(Report::whatever("inner"));
    let expected_line = line!() + 1;
    let result = result.whatever::<OtherError>("wrapped");
    let report = result.expect_err("must fail");
    assert_eq!(report.context().location().file, file!());
    assert_eq!(report.context().location().line, expected_line);
}

#[test]
fn result_ext_on_report_message_captures_the_real_caller_location() {
    // Unlike the bare-`E` case above, `.message()` on an already-existing `Report` does
    // not move `context().location()` (that stays at the report's original creation
    // site); it only adds a new annotation, which has its own location.
    let result: crate::Result<(), TestError> = Err(Report::whatever("inner"));
    let expected_line = line!() + 1;
    let report = result.message("more context").expect_err("must fail");
    let annotation = report
        .context()
        .annotations()
        .last()
        .expect("must have an annotation");
    assert_eq!(annotation.location().file, file!());
    assert_eq!(annotation.location().line, expected_line);
}

#[test]
fn escalate_nests_self_as_cause_without_modifying_it() {
    let inner = Report::<TestError>::whatever("inner failure").field("attempt", 1_u32);
    let outer = inner.escalate(OtherError(()));
    let cause = outer.context().cause().expect("must have a cause");
    assert!(cause.downcast_error::<TestError>().is_some());
    assert_eq!(cause.context().fields()[0].key, "attempt");
}

#[test]
fn with_factors_merges_multiple_independent_failures() {
    let a = Report::<TestError>::whatever("hook a failed");
    let b = Report::<TestError>::whatever("hook b failed");
    let merged = Report::new(OtherError(())).with_factors(vec![a, b]);
    assert_eq!(merged.context().factors().len(), 2);
}

#[test]
fn export_redacts_non_public_fields_by_default() {
    let report = Report::<TestError>::whatever("login failed")
        .field("user", "alice")
        .secret_field("token", "secret-token");
    let exported = report.export();
    let token = exported
        .fields
        .iter()
        .find(|field| field.key == "token")
        .expect("token field must be exported");
    assert_eq!(token.visibility, Visibility::Secret);
    assert!(token.redacted);
    assert_eq!(token.value, None);

    let exported = report.export_with(ExportOptions::new().with_sensitive().with_secrets());
    let token = exported
        .fields
        .iter()
        .find(|field| field.key == "token")
        .expect("token field must be exported");
    assert_eq!(token.value, Some(Value::from("secret-token")));
}

#[test]
fn export_mirrors_cause() {
    let inner = Report::<TestError>::whatever("inner failure");
    let report = inner.escalate(OtherError(()));
    let exported = report.export();
    let cause = exported.cause.expect("must have a cause");
    assert_eq!(cause.error_type, std::any::type_name::<TestError>());
    assert_eq!(cause.error_message.as_deref(), Some("inner failure"));
}

#[test]
fn export_mirrors_factors() {
    let factor = Report::<TestError>::whatever("factor");
    let report = Report::new(OtherError(())).with_factor(factor);
    let exported = report.export();
    assert_eq!(exported.factors.len(), 1);
    assert_eq!(
        exported.factors[0].error_type,
        std::any::type_name::<TestError>()
    );
    assert_eq!(exported.factors[0].error_message.as_deref(), Some("factor"));
}

#[test]
fn export_error_message_falls_back_to_message_annotation() {
    // `Whatever` types never carry their own message (see
    // `whatever_type_never_carries_its_own_message`), so `error_message` must come from
    // the narrative instead, matching what `Display` renders.
    let report = Report::<TestError>::whatever("something failed");
    assert_eq!(
        report.export().error_message.as_deref(),
        Some("something failed")
    );
}

#[cfg(feature = "backtrace")]
#[test]
fn export_excludes_backtrace_by_default_includes_when_requested() {
    let report = Report::<TestError>::whatever("failed");
    if !report.context().has_backtrace() {
        // Backtraces are opt-in (`RUST_BACKTRACE=1`); nothing to check without one.
        return;
    }
    assert_eq!(report.export().backtrace, None);
    let frames = report
        .export_with(ExportOptions::new().with_backtrace())
        .backtrace
        .expect("backtrace must be exported when requested");
    assert!(!frames.is_empty());
}

#[cfg(feature = "spantrace")]
#[test]
fn export_excludes_spantrace_by_default_includes_when_requested() {
    let report = Report::<TestError>::whatever("failed");
    if !report.context().has_spantrace() {
        return;
    }
    assert_eq!(report.export().spantrace, None);
    let spans = report
        .export_with(ExportOptions::new().with_spantrace())
        .spantrace
        .expect("spantrace must be exported when requested");
    assert!(!spans.is_empty());
}

#[cfg(feature = "serde")]
#[test]
fn serde_export_is_stable_json() {
    let report = Report::<TestError>::whatever("failed").field("attempt", 2_u32);
    let json = serde_json::to_value(report.export()).expect("report export must serialize");
    assert_eq!(json["errorMessage"], "failed");
    assert_eq!(json["fields"][0]["key"], "attempt");
}

#[cfg(feature = "serde")]
#[test]
fn serde_export_uses_camel_case_keys() {
    let report = Report::<TestError>::whatever("outer")
        .escalate(OtherError(()))
        .field("attempt_count", 2_u32)
        .suggestion("try again");
    let json = serde_json::to_value(report.export()).expect("report export must serialize");

    // `ExportedReport`'s own multi-word fields: camelCase.
    assert!(json.get("errorType").is_some());
    assert!(json.get("errorMessage").is_some());
    assert!(json.get("error_type").is_none());
    assert!(json.get("error_message").is_none());

    // `Visibility`/`Value`/`ExportedAnnotation` enum tags stay Capitalized, unlike field
    // names: they're names of variants, not fields, so Rust's own PascalCase convention
    // applies rather than JSON's typical camelCase-for-fields one.
    assert_eq!(json["fields"][0]["visibility"], "Public");
    assert_eq!(json["fields"][0]["value"]["type"], "Unsigned");
    assert_eq!(json["annotations"][0]["kind"], "Suggestion");
}

#[cfg(feature = "serde")]
#[test]
fn serde_export_round_trips_through_json() {
    let inner = Report::<TestError>::whatever("inner failure").field("attempt", 1_u32);
    let factor = Report::<TestError>::whatever("a factor");
    let report = inner
        .escalate(OtherError(()))
        .with_factor(factor)
        .secret_field("token", "abc123");
    let exported = report.export_with(ExportOptions::new().with_sensitive().with_secrets());

    let json = serde_json::to_string(&exported).expect("export must serialize");
    let round_tripped: crate::export::ExportedReport =
        serde_json::from_str(&json).expect("export must deserialize");

    assert_eq!(round_tripped, exported);
}

#[cfg(all(feature = "serde", feature = "backtrace"))]
#[test]
fn serde_export_backtrace_is_structured_frames() {
    let report = Report::<TestError>::whatever("failed");
    if !report.context().has_backtrace() {
        // Backtraces are opt-in (`RUST_BACKTRACE=1`); nothing to check without one.
        return;
    }
    let exported = report.export_with(ExportOptions::new().with_backtrace());
    let json = serde_json::to_value(exported).expect("report export must serialize");
    let frame = &json["backtrace"][0];
    for key in ["symbol", "file", "line", "column", "address"] {
        assert!(frame.get(key).is_some(), "frame must have a {key:?} key");
    }
}

#[test]
fn catch_unwind_captures_message_and_location() {
    let result = catch_unwind(AssertUnwindSafe(|| {
        panic!("boom");
    }));
    let report = result.expect_err("closure must have panicked");
    assert!(format!("{report}").contains("boom"));
}

#[derive(Debug)]
struct CancellationSignal;

#[test]
fn catch_unwind_preserves_raw_payload_for_downcast_and_resume() {
    let result = catch_unwind(AssertUnwindSafe(|| {
        std::panic::panic_any(CancellationSignal);
    }));
    let report = result.expect_err("closure must have panicked");
    assert!(
        report
            .error()
            .payload()
            .downcast_ref::<CancellationSignal>()
            .is_some()
    );

    let resumed = std::panic::catch_unwind(AssertUnwindSafe(|| {
        report.into_error().resume_unwind();
    }));
    assert!(resumed.is_err());
}

#[test]
fn catch_unwind_does_not_catch_successful_execution() {
    let result = catch_unwind(AssertUnwindSafe(|| 1 + 1));
    assert_eq!(result.expect("closure must not have panicked"), 2);
}

// `install_pretty_panic_hook` flips a process-global flag with no way to unset it, so
// this only checks that doing so leaves `catch_unwind`'s own captured `Report` correct;
// actually verifying the pretty-printed stderr text needs a real subprocess, checked
// manually with a throwaway crate instead.
#[test]
fn install_pretty_panic_hook_does_not_disturb_catch_unwind() {
    install_pretty_panic_hook();
    let result = catch_unwind(AssertUnwindSafe(|| {
        panic!("boom");
    }));
    let report = result.expect_err("closure must have panicked");
    assert!(format!("{report}").contains("boom"));
}

#[test]
fn render_shows_every_message_not_just_the_headline() {
    let report = Report::<TestError>::whatever("first message").message("second message");
    let text = format!("{report}");
    assert!(text.contains("first message"));
    assert!(text.contains("second message"));
}

#[test]
fn render_indents_continuation_lines_of_multiline_fields_and_suggestions() {
    let report = Report::<TestError>::whatever("outer")
        .field("path", "first line\nsecond line")
        .suggestion("try this\nor that");
    let text = report.render(RenderOptions::new());
    for continuation in ["second line", "or that"] {
        assert!(
            !text.lines().any(|line| line == continuation),
            "a continuation line must be indented under the tree's connector, not bare, got:\n{text}"
        );
        assert!(text.contains(continuation));
    }
}

#[test]
fn render_indents_continuation_lines_of_a_multiline_nested_headline() {
    let inner = Report::<TestError>::whatever("inner first\ninner second");
    let report = Report::<OtherError>::whatever("outer").with_factor(inner);
    let text = report.render(RenderOptions::new());
    assert!(
        !text.lines().any(|line| line == "inner second"),
        "a nested report's continuation line must be indented, not bare, got:\n{text}"
    );
    assert!(text.contains("inner second"));
}

#[test]
fn render_does_not_wrap_by_default() {
    let long = "word ".repeat(30);
    let report = Report::<TestError>::whatever(long.trim_end().to_owned());
    let text = report.render(RenderOptions::new().color(ColorMode::Never));
    assert_eq!(
        text.lines().count(),
        2,
        "unwrapped headline plus location, got:\n{text}"
    );
}

#[test]
fn render_wraps_long_lines_to_a_fixed_width_with_indent() {
    // `.compact()` drops the "at <location>" line: a file path has no spaces, so it is
    // a single unbreakable "word" that may legitimately exceed the requested width,
    // unlike the space-separated headline/suggestion text this test checks.
    let report = Report::<TestError>::whatever("word ".repeat(30).trim_end().to_owned())
        .suggestion("advice ".repeat(30).trim_end().to_owned());
    let text = report.render(
        RenderOptions::new()
            .color(ColorMode::Never)
            .compact()
            .wrap(WrapMode::Fixed(20)),
    );
    for line in text.lines() {
        assert!(
            line.chars().count() <= 20,
            "line exceeds the requested wrap width, got:\n{text}"
        );
    }
    // Continuation lines of the wrapped suggestion still align under the tree, not bare.
    assert!(
        !text.lines().any(|line| line.starts_with("advice")),
        "a continuation line must be indented under the tree's connector, not bare, got:\n{text}"
    );
}

#[test]
fn render_wrap_never_breaks_a_single_word_mid_word() {
    let word = "x".repeat(50);
    let report = Report::<TestError>::whatever(word.clone());
    let text = report.render(
        RenderOptions::new()
            .color(ColorMode::Never)
            .wrap(WrapMode::Fixed(10)),
    );
    assert!(
        text.contains(&word),
        "an over-long word must not be split, got:\n{text}"
    );
}

#[test]
fn render_wrap_accounts_for_the_nested_report_label_on_the_first_line() {
    let inner = Report::<TestError>::whatever("word ".repeat(30).trim_end().to_owned());
    let report = Report::<OtherError>::whatever("outer").with_factor(inner);
    let text = report.render(
        RenderOptions::new()
            .color(ColorMode::Never)
            .wrap(WrapMode::Fixed(20)),
    );
    let factor_line = text
        .lines()
        .find(|line| line.contains("factor: "))
        .expect("must have a factor line");
    assert!(
        factor_line.chars().count() <= 20,
        "the factor label must count against the first line's wrap budget, got:\n{text}"
    );
}

#[test]
fn render_ascii_uses_no_unicode_box_drawing() {
    let inner = Report::<TestError>::whatever("inner failure");
    let report = inner
        .escalate(OtherError(()))
        .with_factor(Report::<TestError>::whatever("a factor"));
    let text = report.render(RenderOptions::new().ascii());
    for unicode_connector in ['├', '╰', '│', '╴', '▶'] {
        assert!(
            !text.contains(unicode_connector),
            "ascii render must not contain {unicode_connector:?}, got:\n{text}"
        );
    }
}

#[test]
fn render_labels_cause_and_factor_differently() {
    let cause_only = Report::<TestError>::whatever("inner failure").escalate(OtherError(()));
    let text = cause_only.render(RenderOptions::new());
    assert!(text.contains("cause: "), "got:\n{text}");
    assert!(!text.contains("factor: "), "got:\n{text}");

    let factors_only = Report::<OtherError>::whatever("outer failure").with_factors(vec![
        Report::<TestError>::whatever("factor a"),
        Report::<TestError>::whatever("factor b"),
    ]);
    let text = factors_only.render(RenderOptions::new());
    assert!(!text.contains("cause: "), "got:\n{text}");
    assert_eq!(text.matches("factor: ").count(), 2, "got:\n{text}");
}

#[test]
fn render_compact_omits_locations_full_includes_them() {
    let report = Report::<TestError>::whatever("failed");
    let compact = report.render(RenderOptions::new().compact());
    let full = report.render(RenderOptions::new().full());
    assert!(!compact.contains("at "));
    assert!(full.contains("at "));
}

#[test]
fn render_verbose_shows_backtrace_marker_exactly_when_captured() {
    let report = Report::<TestError>::whatever("failed");
    let text = report.render(RenderOptions::new().verbose());
    assert_eq!(text.contains("BACKTRACE"), report.context().has_backtrace());
}

#[cfg(feature = "color")]
#[test]
fn render_wrap_measures_visible_width_not_ansi_escaped_length() {
    // The headline is always styled (bold/red), so with `ColorMode::Always` its escape
    // codes are part of the raw string handed to the wrap logic. If wrapping measured
    // raw length instead of visible width, the escape codes would count against the
    // budget and the colored version would wrap onto more lines than the plain one, for
    // identical visible content.
    let text = "word ".repeat(10).trim_end().to_owned();
    let report = Report::<TestError>::whatever(text);
    let plain = report.render(
        RenderOptions::new()
            .color(ColorMode::Never)
            .wrap(WrapMode::Fixed(40)),
    );
    let colored = report.render(
        RenderOptions::new()
            .color(ColorMode::Always)
            .wrap(WrapMode::Fixed(40)),
    );
    assert_eq!(
        plain.lines().count(),
        colored.lines().count(),
        "styling must not change where a line wraps, plain:\n{plain}\ncolored:\n{colored}"
    );
}

#[cfg(feature = "color")]
#[test]
fn render_color_mode_controls_ansi_codes() {
    let report = Report::<TestError>::whatever("failed");
    let colored = report.render(RenderOptions::new().color(ColorMode::Always));
    let plain = report.render(RenderOptions::new().color(ColorMode::Never));
    assert!(colored.contains("\u{1b}["));
    assert!(!plain.contains("\u{1b}["));
}

#[test]
fn debug_alternate_and_non_alternate_render_the_same_tree() {
    let report = Report::<TestError>::whatever("failed");
    let debug = format!("{report:?}");
    let debug_alternate = format!("{report:#?}");
    assert_eq!(debug, debug_alternate);
    assert!(!debug_alternate.starts_with("Report {"));
}

#[cfg(feature = "color")]
#[test]
fn render_options_default_color_checks_stderr() {
    assert_eq!(RenderOptions::default().color, ColorMode::AutoStderr);
}

#[cfg(feature = "color")]
#[test]
fn for_stdout_and_for_stderr_correct_only_the_auto_variants() {
    for options in [
        RenderOptions::new().color(ColorMode::AutoStdout),
        RenderOptions::new().color(ColorMode::AutoStderr),
    ] {
        assert_eq!(options.for_stdout().color, ColorMode::AutoStdout);
        assert_eq!(options.for_stderr().color, ColorMode::AutoStderr);
    }
    for options in [
        RenderOptions::new().color(ColorMode::Always),
        RenderOptions::new().color(ColorMode::Never),
    ] {
        assert_eq!(options.for_stdout().color, options.color);
        assert_eq!(options.for_stderr().color, options.color);
    }
}

#[test]
fn print_and_eprint_do_not_panic() {
    let report = Report::<TestError>::whatever("failed");
    report.print(RenderOptions::new());
    report.eprint(RenderOptions::new());
}

#[cfg(feature = "backtrace")]
#[test]
fn render_verbose_backtrace_skips_reportifys_own_frames() {
    let report = Report::<TestError>::whatever("failed");
    if !report.context().has_backtrace() {
        // Backtraces are opt-in (`RUST_BACKTRACE=1`); nothing to check without one.
        return;
    }
    let text = report.render(RenderOptions::new().verbose());
    // Every frame below this call, including this test function itself, lives inside the
    // `reportify` crate (its own test suite), so it is all skipped as reportify's own
    // machinery: there is no "external caller" frame left to assert is still shown, unlike
    // a real downstream consumer. What's left to check from in here is that skipping
    // happened at all, and that the raw capture frame is never printed unskipped.
    assert!(
        text.contains("skipped"),
        "reportify's own frames must be skipped, got:\n{text}"
    );
    assert!(
        !text.contains("reportify::backtrace::Backtrace::capture"),
        "reportify's own capture frame must be skipped, got:\n{text}"
    );
}

/// Minimal `tracing::Subscriber` that records every event's fields as strings, just
/// enough to assert on `log_error`/`log_warning`/`log_info`'s structured fields without
/// pulling in `tracing-subscriber` as a dev-dependency.
#[derive(Clone, Default)]
struct RecordingSubscriber {
    fields: std::sync::Arc<std::sync::Mutex<Vec<(String, String)>>>,
}

impl tracing::Subscriber for RecordingSubscriber {
    fn enabled(&self, _metadata: &tracing::Metadata<'_>) -> bool {
        true
    }

    fn new_span(&self, _span: &tracing::span::Attributes<'_>) -> tracing::span::Id {
        tracing::span::Id::from_u64(1)
    }

    fn record(&self, _span: &tracing::span::Id, _values: &tracing::span::Record<'_>) {}

    fn record_follows_from(&self, _span: &tracing::span::Id, _follows: &tracing::span::Id) {}

    fn event(&self, event: &tracing::Event<'_>) {
        struct Recorder<'a>(&'a mut Vec<(String, String)>);

        impl tracing::field::Visit for Recorder<'_> {
            fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
                self.0.push((field.name().to_owned(), format!("{value:?}")));
            }

            fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
                self.0.push((field.name().to_owned(), value.to_owned()));
            }
        }

        let mut fields = self.fields.lock().expect("not poisoned");
        event.record(&mut Recorder(&mut fields));
    }

    fn enter(&self, _span: &tracing::span::Id) {}

    fn exit(&self, _span: &tracing::span::Id) {}
}

impl RecordingSubscriber {
    fn field(&self, name: &str) -> Option<String> {
        self.fields
            .lock()
            .expect("not poisoned")
            .iter()
            .find(|(key, _)| key == name)
            .map(|(_, value)| value.clone())
    }
}

#[derive(Debug)]
struct CodedError;

impl crate::Error for CodedError {
    fn message(&self) -> Option<&dyn std::fmt::Display> {
        None
    }

    fn code(&self) -> Option<&'static str> {
        Some("E_RATE_LIMIT")
    }
}

#[test]
fn log_error_attaches_structured_type_and_code_fields() {
    let subscriber = RecordingSubscriber::default();
    let result: crate::Result<(), TestError> = Err(Report::whatever("boom"));
    tracing::subscriber::with_default(subscriber.clone(), || {
        let _ = result.log_error();
    });
    assert!(
        subscriber
            .field("error.type")
            .expect("error.type must be set")
            .contains("TestError")
    );
    assert_eq!(subscriber.field("error.code"), None);

    let subscriber = RecordingSubscriber::default();
    let result: crate::Result<(), CodedError> = Err(CodedError.report());
    tracing::subscriber::with_default(subscriber.clone(), || {
        let _ = result.log_error();
    });
    assert!(
        subscriber
            .field("error.type")
            .expect("error.type must be set")
            .contains("CodedError")
    );
    assert_eq!(
        subscriber.field("error.code").as_deref(),
        Some("E_RATE_LIMIT")
    );
}
