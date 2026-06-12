use std::path::Path;

use anyhow::Result;
use dprint_core::configuration::resolve_new_line_kind;
use dprint_core::formatting::PrintOptions;

use crate::configuration::Configuration;
use crate::generation;

pub fn format_text(_path: &Path, text: &str, config: &Configuration) -> Result<Option<String>> {
  let result = format_text_inner(text, config)?;
  if result == text { Ok(None) } else { Ok(Some(result)) }
}

fn format_text_inner(text: &str, config: &Configuration) -> Result<String> {
  let text = text.strip_prefix('\u{FEFF}').unwrap_or(text);
  let tokens = generation::tokenize(text);
  if has_ignore_file_comment(&tokens, &config.ignore_file_comment_text) {
    return Ok(text.to_string());
  }
  let statements = generation::parse(&tokens, text);
  if statements.is_empty() {
    return Ok(String::new());
  }
  Ok(dprint_core::formatting::format(
    || generation::generate(&statements, text, config),
    PrintOptions {
      indent_width: config.indent_width,
      max_width: config.line_width,
      use_tabs: config.use_tabs,
      new_line_text: resolve_new_line_kind(text, config.new_line_kind),
    },
  ))
}

fn has_ignore_file_comment(tokens: &[generation::Token], directive: &str) -> bool {
  lax_core::has_ignore_file_comment(
    tokens.iter().map(|token| match token.kind {
      generation::TokenKind::Whitespace { newlines } => lax_core::HeaderToken::Whitespace { newlines },
      generation::TokenKind::LineComment | generation::TokenKind::BlockComment => {
        lax_core::HeaderToken::Comment(token.text)
      }
      _ => lax_core::HeaderToken::Other,
    }),
    directive,
  )
}
