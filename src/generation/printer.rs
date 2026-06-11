use dprint_core::formatting::PrintItems;
use dprint_core::formatting::Signal;

use super::parser::Statement;
use super::parser::StatementKind;
use super::tokenizer::Token;
use super::tokenizer::TokenKind;
use crate::configuration::Configuration;

pub fn generate(statements: &[Statement], _config: &Configuration) -> PrintItems {
  let mut items = PrintItems::new();
  gen_statements(statements, &mut items);
  items
}

fn gen_statements(statements: &[Statement], items: &mut PrintItems) {
  for (i, statement) in statements.iter().enumerate() {
    if i > 0 && statement.blank_line_before {
      items.push_signal(Signal::NewLine);
    }
    gen_statement(statement, items);
    if let Some(comment) = statement.trailing_comment {
      items.push_space();
      push_text(items, comment.text);
    }
    items.push_signal(Signal::NewLine);
  }
}

fn gen_statement(statement: &Statement, items: &mut PrintItems) {
  match &statement.kind {
    StatementKind::Comment { token } => push_text(items, token.text),
    StatementKind::QualifiedRule { prelude, body } => {
      if !prelude.is_empty() {
        gen_tokens(prelude, items, true);
        items.push_space();
      }
      gen_block(body, items);
    }
    StatementKind::AtRule { name, prelude, body } => {
      items.push_string(name.text.to_string());
      if !prelude.is_empty() {
        items.push_space();
        gen_tokens(prelude, items, false);
      }
      match body {
        Some(body) => {
          items.push_space();
          gen_block(body, items);
        }
        None => items.push_string(";".to_string()),
      }
    }
    StatementKind::Declaration {
      name,
      value,
      verbatim_value,
    } => {
      gen_tokens(name, items, false);
      items.push_string(":".to_string());
      if !value.is_empty() {
        items.push_space();
        if *verbatim_value {
          gen_verbatim(value, items);
        } else {
          gen_tokens(value, items, false);
        }
      }
      items.push_string(";".to_string());
    }
    StatementKind::Raw { tokens, semicolon } => {
      gen_tokens(tokens, items, false);
      if *semicolon {
        items.push_string(";".to_string());
      }
    }
  }
}

fn gen_block(body: &[Statement], items: &mut PrintItems) {
  items.push_string("{".to_string());
  if body.is_empty() {
    items.push_signal(Signal::NewLine);
  } else {
    items.push_signal(Signal::StartIndent);
    items.push_signal(Signal::NewLine);
    gen_statements(body, items);
    items.push_signal(Signal::FinishIndent);
  }
  items.push_string("}".to_string());
}

/// Prints a token sequence, only ever normalizing whitespace. Existing
/// whitespace between tokens collapses to a single space and whitespace is
/// never added where the author had none, so token text is never reinterpreted.
/// The exceptions are commas (never a space before) and the inside edges of
/// parens and brackets (no space).
fn gen_tokens(tokens: &[Token], items: &mut PrintItems, selector: bool) {
  let mut depth = 0u32;
  let mut pending_space = false;
  let mut suppress_space = false;
  for token in tokens {
    match token.kind {
      TokenKind::Whitespace { .. } => {
        if !suppress_space {
          pending_space = true;
        }
      }
      TokenKind::Comma => {
        // no space before a comma; whether a space follows is decided by
        // the source, since a space can be meaningful, for example inside
        // Tailwind arbitrary values like `ease-[cubic-bezier(0.4,0,0.1,1)]`
        pending_space = false;
        items.push_string(",".to_string());
        if selector && depth == 0 {
          items.push_signal(Signal::NewLine);
          suppress_space = true;
        } else {
          suppress_space = false;
        }
      }
      TokenKind::CloseParen | TokenKind::CloseBracket => {
        depth = depth.saturating_sub(1);
        items.push_string(token.text.to_string());
        pending_space = false;
        suppress_space = false;
      }
      _ => {
        if matches!(
          token.kind,
          TokenKind::OpenParen | TokenKind::OpenBracket | TokenKind::Function
        ) {
          depth += 1;
        }
        if pending_space {
          items.push_space();
        }
        push_text(items, token.text);
        pending_space = false;
        // no space directly after an opening paren or bracket
        suppress_space = matches!(
          token.kind,
          TokenKind::OpenParen | TokenKind::OpenBracket | TokenKind::Function
        );
      }
    }
  }
}

/// Prints tokens exactly as they appeared in the source.
fn gen_verbatim(tokens: &[Token], items: &mut PrintItems) {
  let mut text = String::new();
  for token in tokens {
    text.push_str(token.text);
  }
  push_text(items, &text);
}

/// Pushes text that may contain newlines. Lines after the first are printed
/// verbatim without applying the current indentation level.
fn push_text(items: &mut PrintItems, text: &str) {
  if !text.contains('\n') {
    items.push_string(text.to_string());
    return;
  }
  let mut lines = text.split('\n');
  if let Some(first) = lines.next() {
    let first = first.trim_end_matches('\r');
    if !first.is_empty() {
      items.push_string(first.to_string());
    }
  }
  items.push_signal(Signal::StartIgnoringIndent);
  for line in lines {
    items.push_signal(Signal::NewLine);
    let line = line.trim_end_matches('\r');
    if !line.is_empty() {
      items.push_string(line.to_string());
    }
  }
  items.push_signal(Signal::FinishIgnoringIndent);
}
