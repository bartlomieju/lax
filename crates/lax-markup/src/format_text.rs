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
pub type ExternalFormatter<'a> = dyn Fn(&str, &str, u32) -> Result<Option<String>> + 'a;

pub fn format_text(_path: &Path, text: &str, config: &Configuration) -> Result<Option<String>> {
  let result = format_text_inner(text, config, None)?;
  if result == text { Ok(None) } else { Ok(Some(result)) }
}

pub fn format_text_with_external(
  path: &Path,
  text: &str,
  config: &Configuration,
  external: &ExternalFormatter,
) -> Result<Option<String>> {
  // Astro files start with a frontmatter block whose body is TypeScript
  if path.extension().and_then(|e| e.to_str()) == Some("astro")
    && let Some((frontmatter, rest)) = split_frontmatter(text)
  {
    let body = match external("ts", &dedent(frontmatter), config.line_width)? {
      Some(formatted) => formatted.trim_end().to_string(),
      None => dedent(frontmatter).trim().to_string(),
    };
    let rest_formatted = format_text_inner(rest, config, Some(external))?;
    let result = format!("---\n{}\n---\n{}", body, rest_formatted);
    return if result == text { Ok(None) } else { Ok(Some(result)) };
  }
  let result = format_text_inner(text, config, Some(external))?;
  if result == text { Ok(None) } else { Ok(Some(result)) }
}

/// Splits a leading `---` fenced frontmatter block, returning its inner text
/// and the remainder of the file.
fn split_frontmatter(text: &str) -> Option<(&str, &str)> {
  let trimmed = text.trim_start();
  let rest = trimmed.strip_prefix("---")?;
  let rest = rest.strip_prefix('\n').or_else(|| rest.strip_prefix("\r\n"))?;
  let mut search_from = 0;
  loop {
    let line_end = rest[search_from..].find('\n').map(|i| search_from + i)?;
    let line = &rest[search_from..line_end];
    if line.trim_end() == "---" {
      return Some((&rest[..search_from], &rest[line_end + 1..]));
    }
    search_from = line_end + 1;
  }
}

/// Strips the longest common leading whitespace prefix from every non empty
/// line.
fn dedent(text: &str) -> String {
  let mut common: Option<&str> = None;
  for line in text.split('\n') {
    if line.trim().is_empty() {
      continue;
    }
    let leading = &line[..line.len() - line.trim_start().len()];
    common = Some(match common {
      None => leading,
      Some(prev) => {
        let len = prev
          .as_bytes()
          .iter()
          .zip(leading.as_bytes())
          .take_while(|(a, b)| a == b)
          .count();
        &prev[..len]
      }
    });
  }
  let common = common.unwrap_or("");
  if common.is_empty() {
    return text.to_string();
  }
  text
    .split('\n')
    .map(|line| line.strip_prefix(common).unwrap_or(line))
    .collect::<Vec<_>>()
    .join("\n")
}

fn format_text_inner(text: &str, config: &Configuration, external: Option<&ExternalFormatter>) -> Result<String> {
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
