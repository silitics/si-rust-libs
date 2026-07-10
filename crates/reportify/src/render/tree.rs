//! Tree-drawing: connectors, indentation, and recursion over a report's narrative,
//! fields, cause, and factors.

use std::fmt::Display;

use crate::annotation::Annotation;
use crate::context::Context;
use crate::erased::ErasedReport;
#[cfg(feature = "color")]
use crate::render::styled;
use crate::render::{Charset, ColorMode, Detail, RenderOptions, resolve_wrap_width, trailer};
use crate::value::Visibility;

struct Connectors {
    mid: &'static str,
    last: &'static str,
    pipe: &'static str,
    blank: &'static str,
    mid_arrow: &'static str,
    last_arrow: &'static str,
    arrow_pipe: &'static str,
    arrow_blank: &'static str,
}

const UNICODE: Connectors = Connectors {
    mid: "├╴",
    last: "╰╴",
    pipe: "│ ",
    blank: "  ",
    mid_arrow: "├─▶ ",
    last_arrow: "╰─▶ ",
    arrow_pipe: "│   ",
    arrow_blank: "    ",
};

const ASCII: Connectors = Connectors {
    mid: "|-",
    last: "\\-",
    pipe: "| ",
    blank: "  ",
    mid_arrow: "|-> ",
    last_arrow: "\\-> ",
    arrow_pipe: "|   ",
    arrow_blank: "    ",
};

/// One child of a rendered node: either a plain, already-formatted line, or another
/// report reached through a cause or a factor.
enum Child<'a> {
    Line(String),
    Report(&'a ErasedReport, ReportKind),
}

/// Whether a nested report was reached through [`Context::cause`] or
/// [`Context::factors`], so the tree can label the two differently instead of rendering
/// them identically.
#[derive(Clone, Copy)]
enum ReportKind {
    Cause,
    Factor,
}

impl ReportKind {
    fn label(self) -> &'static str {
        match self {
            ReportKind::Cause => "cause: ",
            ReportKind::Factor => "factor: ",
        }
    }
}

struct Renderer<'a> {
    out: String,
    indent: String,
    connectors: &'static Connectors,
    color: ColorMode,
    detail: Detail,
    wrap_width: Option<usize>,
    trailers: Vec<&'a Context>,
}

pub(crate) fn render(
    type_name: &'static str,
    message: Option<&dyn Display>,
    code: Option<&'static str>,
    context: &Context,
    options: RenderOptions,
) -> String {
    let mut renderer = Renderer {
        out: String::new(),
        indent: String::new(),
        connectors: match options.charset {
            Charset::Unicode => &UNICODE,
            Charset::Ascii => &ASCII,
        },
        color: options.color,
        detail: options.detail,
        wrap_width: resolve_wrap_width(options.wrap),
        trailers: Vec::new(),
    };
    renderer.render_report(type_name, message, code, context, 0);
    if renderer.detail == Detail::Verbose {
        for (index, context) in renderer.trailers.iter().enumerate() {
            trailer::render_trailer(
                &mut renderer.out,
                options.charset,
                options.color,
                index + 1,
                context,
            );
        }
    }
    renderer.out
}

impl<'a> Renderer<'a> {
    fn render_report(
        &mut self,
        type_name: &'static str,
        message: Option<&dyn Display>,
        code: Option<&'static str>,
        context: &'a Context,
        prefix_len: usize,
    ) {
        let headline = headline(type_name, message, context);
        let styled_headline = match code {
            Some(code) => format!("{} [{code}]", style_headline(self.color, &headline)),
            None => style_headline(self.color, &headline),
        };
        let wrapped_headline = self.wrap(&styled_headline, prefix_len);
        push_multiline(&mut self.out, &self.indent, &wrapped_headline);

        let mut children = Vec::new();

        if self.detail != Detail::Compact {
            children.push(Child::Line(format!(
                "at {}",
                style_location(self.color, context.location())
            )));
        }

        for field in context.fields() {
            let value = if field.visibility == Visibility::Public {
                field.value.to_string()
            } else {
                format!("<redacted:{}>", field.visibility)
            };
            children.push(Child::Line(format!("{}: {value}", field.key)));
        }

        for annotation in other_annotations(context) {
            match annotation {
                Annotation::Message { text, .. } => children.push(Child::Line(text.clone())),
                Annotation::Suggestion { text, .. } => {
                    children.push(Child::Line(style_suggestion(self.color, text)));
                }
            }
        }

        if self.detail == Detail::Verbose && trailer::has_details(context) {
            self.trailers.push(context);
            children.push(Child::Line(format!("BACKTRACE ({})", self.trailers.len())));
        }

        if let Some(cause) = context.cause() {
            children.push(Child::Report(cause, ReportKind::Cause));
        }
        for factor in context.factors() {
            children.push(Child::Report(factor, ReportKind::Factor));
        }

        self.render_children(children);
    }

    fn render_children(&mut self, children: Vec<Child<'a>>) {
        let mut children = children.into_iter().peekable();
        while let Some(child) = children.next() {
            let previous_indent_len = self.indent.len();
            let is_last = children.peek().is_none();
            let is_report = matches!(child, Child::Report(..));

            self.out.push('\n');
            self.out.push_str(&self.indent);

            if is_report {
                self.out.push_str(self.connectors.arrow_pipe);
                self.out.push('\n');
                self.out.push_str(&self.indent);
            }

            if is_last {
                self.out.push_str(if is_report {
                    self.connectors.last_arrow
                } else {
                    self.connectors.last
                });
                self.indent.push_str(if is_report {
                    self.connectors.arrow_blank
                } else {
                    self.connectors.blank
                });
            } else {
                self.out.push_str(if is_report {
                    self.connectors.mid_arrow
                } else {
                    self.connectors.mid
                });
                self.indent.push_str(if is_report {
                    self.connectors.arrow_pipe
                } else {
                    self.connectors.pipe
                });
            }

            match child {
                Child::Line(line) => {
                    let wrapped = self.wrap(&line, 0);
                    push_multiline(&mut self.out, &self.indent, &wrapped);
                }
                Child::Report(report, kind) => {
                    self.out.push_str(kind.label());
                    self.render_report(
                        report.error_type_name(),
                        report.error_message(),
                        report.error_code(),
                        report.context(),
                        kind.label().chars().count(),
                    );
                }
            }

            self.indent.truncate(previous_indent_len);
        }
    }

    /// Word-wrap `text` to fit `self.wrap_width`, reserving `self.indent`'s width on
    /// every wrapped line, plus `prefix_len` additional columns already consumed on the
    /// first line only, e.g., a `"cause: "` label pushed before this call. A no-op
    /// (returns `text` unchanged) if wrapping is off, or if `text` fits as is. Existing
    /// newlines are preserved as hard breaks; wrapping only adds more.
    fn wrap(&self, text: &str, prefix_len: usize) -> String {
        let Some(width) = self.wrap_width else {
            return text.to_owned();
        };
        let indent_width = self.indent.chars().count();
        let mut result = String::new();
        for (paragraph_index, paragraph) in text.split('\n').enumerate() {
            if paragraph_index > 0 {
                result.push('\n');
            }
            let reserved = indent_width + if paragraph_index == 0 { prefix_len } else { 0 };
            let budget = width.saturating_sub(reserved).max(1);
            let wrapped = wrap_line(paragraph, budget);
            result.push_str(&wrapped.join("\n"));
        }
        result
    }
}

/// Write `text` to `out`, prefixing every line after the first with `indent`: a
/// message/suggestion/field value with embedded newlines otherwise breaks the tree's
/// box-drawing, since a bare continuation line has no connector to align under.
fn push_multiline(out: &mut String, indent: &str, text: &str) {
    let mut lines = text.split('\n');
    if let Some(first) = lines.next() {
        out.push_str(first);
    }
    for line in lines {
        out.push('\n');
        out.push_str(indent);
        out.push_str(line);
    }
}

/// Visible width of `text`: with the `color` feature, ANSI styling codes do not count,
/// so wrapping a styled string wraps where it actually looks too long, not where its
/// escape codes happen to push the raw length over budget.
#[cfg(feature = "color")]
fn text_width(text: &str) -> usize {
    console::measure_text_width(text)
}

#[cfg(not(feature = "color"))]
fn text_width(text: &str) -> usize {
    text.chars().count()
}

/// Greedily pack space-separated words from `line` into as few lines as fit `budget`
/// columns each. A single word wider than `budget` on its own gets its own line rather
/// than being split in the middle.
fn wrap_line(line: &str, budget: usize) -> Vec<String> {
    let mut result = Vec::new();
    let mut current = String::new();
    let mut current_width = 0;
    for word in line.split(' ') {
        let word_width = text_width(word);
        let needed = if current.is_empty() {
            word_width
        } else {
            current_width + 1 + word_width
        };
        if !current.is_empty() && needed > budget {
            result.push(std::mem::take(&mut current));
            current_width = 0;
        }
        if !current.is_empty() {
            current.push(' ');
            current_width += 1;
        }
        current.push_str(word);
        current_width += word_width;
    }
    result.push(current);
    result
}

#[cfg(feature = "color")]
fn style_headline(color: ColorMode, text: &str) -> String {
    styled(console::style(text).bold().red(), color).to_string()
}

#[cfg(not(feature = "color"))]
fn style_headline(_color: ColorMode, text: &str) -> String {
    text.to_owned()
}

#[cfg(feature = "color")]
fn style_location(color: ColorMode, location: impl Display) -> String {
    styled(console::style(location).dim(), color).to_string()
}

#[cfg(not(feature = "color"))]
fn style_location(_color: ColorMode, location: impl Display) -> String {
    location.to_string()
}

#[cfg(feature = "color")]
fn style_suggestion(color: ColorMode, text: &str) -> String {
    format!("suggestion: {}", styled(console::style(text).cyan(), color))
}

#[cfg(not(feature = "color"))]
fn style_suggestion(_color: ColorMode, text: &str) -> String {
    format!("suggestion: {text}")
}

fn headline(type_name: &'static str, message: Option<&dyn Display>, context: &Context) -> String {
    crate::annotation::headline(context.annotations())
        .map(ToOwned::to_owned)
        .or_else(|| message.map(ToString::to_string))
        .unwrap_or_else(|| type_name.to_owned())
}

/// Annotations other than the one used as the headline: earlier messages (if `.message()`
/// was called more than once) and every suggestion, in the order they were added.
fn other_annotations(context: &Context) -> impl Iterator<Item = &Annotation> {
    let headline_index = context
        .annotations()
        .iter()
        .rposition(|annotation| matches!(annotation, Annotation::Message { .. }));
    context
        .annotations()
        .iter()
        .enumerate()
        .filter_map(move |(index, annotation)| {
            (Some(index) != headline_index).then_some(annotation)
        })
}
