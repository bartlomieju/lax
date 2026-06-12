use std::path::Path;

use anyhow::Result;
use dprint_core::configuration::resolve_new_line_kind;
use dprint_core::formatting::PrintOptions;

use crate::configuration::Configuration;
use crate::generation;

/// Formats the contents of an embedded block, like CSS in a style element or
/// a script body. Receives the language hint (the element's `lang` or `type`
/// attribute value, or `css`/`js` by element kind), the raw inner text, and
/// the remaining print width. Returning `Ok(None)` keeps the contents
/// verbatim.
pub type ExternalFormatter<'a> =
  dyn Fn(&str, &str, u32) -> Result<Option<String>> + 'a;

pub fn format_text(_path: &Path, text: &str, config: &Configuration) -> Result<Option<String>> {
  let result = format_text_inner(text, config, None)?;
  if result == text { Ok(None) } else { Ok(Some(result)) }
}

pub fn format_text_with_external(
  _path: &Path,
  text: &str,
  config: &Configuration,
  external: &ExternalFormatter,
) -> Result<Option<String>> {
  let result = format_text_inner(text, config, Some(external))?;
  if result == text { Ok(None) } else { Ok(Some(result)) }
}

fn format_text_inner(
  text: &str,
  config: &Configuration,
  external: Option<&ExternalFormatter>,
) -> Result<String> {
  let text = text.strip_prefix('\u{FEFF}').unwrap_or(text);
  let events = generation::tokenize(text);
  if has_ignore_file_comment(&events, &config.ignore_file_comment_text) {
    return Ok(text.to_string());
  }
  let nodes = generation::parse(events);
  if nodes.is_empty() {
    return Ok(String::new());
  }
  let external_error = std::cell::RefCell::new(None);
  let formatted = dprint_core::formatting::format(
    || generation::generate(&nodes, text, config, external, &external_error),
    PrintOptions {
      indent_width: config.indent_width,
      max_width: config.line_width,
      use_tabs: config.use_tabs,
      new_line_text: resolve_new_line_kind(text, config.new_line_kind),
    },
  );
  if let Some(error) = external_error.into_inner() {
    return Err(error);
  }
  // exactly one trailing newline, so verbatim regions at the end of the
  // file cannot accumulate blank lines across passes
  Ok(format!("{}\n", formatted.trim_end()))
}

fn has_ignore_file_comment(events: &[generation::Event], directive: &str) -> bool {
  lax_core::has_ignore_file_comment(
    events.iter().map(|event| match &event.kind {
      generation::EventKind::Whitespace { newlines } => lax_core::HeaderToken::Whitespace { newlines: *newlines },
      generation::EventKind::Comment { text } => lax_core::HeaderToken::Comment(text),
      _ => lax_core::HeaderToken::Other,
    }),
    directive,
  )
}
