use dprint_core::formatting::PrintItems;
use dprint_core::formatting::Signal;

use super::keywords::is_keyword;
use super::parser::Statement;
use super::parser::StatementKind;
use super::tokenizer::Token;
use super::tokenizer::TokenKind;
use crate::configuration::Configuration;
use crate::configuration::KeywordCase;

struct Context<'a> {
  source: &'a str,
  ignore_directive: &'a str,
  keyword_case: KeywordCase,
}

pub fn generate(statements: &[Statement], source: &str, config: &Configuration) -> PrintItems {
  let mut items = PrintItems::new();
  let ctx = Context {
    source,
    ignore_directive: &config.ignore_node_comment_text,
    keyword_case: config.keyword_case,
  };
  gen_statements(statements, &mut items, &ctx);
  items
}

fn gen_statements(statements: &[Statement], items: &mut PrintItems, ctx: &Context) {
  let mut ignore_next = false;
  for (i, statement) in statements.iter().enumerate() {
    if i > 0 && statement.blank_line_before {
      items.push_signal(Signal::NewLine);
    }
    let is_comment = matches!(statement.kind, StatementKind::Comment { .. });
    if ignore_next && !is_comment {
      // print the statement exactly as it was written
      push_text(items, ctx.source[statement.span.0..statement.span.1].trim_end());
      items.push_signal(Signal::NewLine);
      ignore_next = false;
      continue;
    }
    if let StatementKind::Comment { token } = &statement.kind
      && token.text.contains(ctx.ignore_directive)
    {
      ignore_next = true;
    }
    gen_statement(statement, items, ctx);
    if let Some(comment) = statement.trailing_comment {
      items.push_space();
      push_comment(items, ctx, &comment);
    }
    items.push_signal(Signal::NewLine);
  }
}

fn gen_statement(statement: &Statement, items: &mut PrintItems, ctx: &Context) {
  match &statement.kind {
    StatementKind::Comment { token } => push_comment(items, ctx, token),
    StatementKind::Sql { clauses, semicolon } => {
      for (i, clause) in clauses.iter().enumerate() {
        if i > 0 {
          items.push_signal(Signal::NewLine);
        }
        gen_clause(clause, items, ctx);
      }
      if *semicolon {
        if clauses.last().is_some_and(|c| ends_with_line_comment(c)) {
          items.push_signal(Signal::NewLine);
        }
        items.push_string(";".to_string());
      }
    }
  }
}

#[derive(PartialEq, Clone, Copy)]
enum Pending {
  None,
  Space,
  Newline,
}

/// Prints one clause of a statement.
///
/// Whitespace handling follows the lax policy:
/// - a single author space becomes a possible line break point when the line
///   exceeds the configured width
/// - an author newline is preserved as a newline, so hand formatted
///   statements keep their shape
/// - a line break is never introduced where the author had no whitespace
///
/// Continuation lines are indented one level. A paren group the author
/// opened with a newline indents its contents one level per nesting depth
/// and puts the closing paren back at the start level.
fn gen_clause(tokens: &[Token], items: &mut PrintItems, ctx: &Context) {
  let mut extra_indent = 0usize;
  let mut pending = Pending::None;
  let mut after_open = false;
  let mut first_emitted = false;
  // one entry per open paren; true when the group is multi line
  let mut groups: Vec<bool> = Vec::new();
  for token in tokens {
    let mut emitted = false;
    match token.kind {
      TokenKind::Whitespace { newlines } => {
        if after_open {
          if newlines > 0
            && let Some(top) = groups.last_mut()
          {
            *top = true;
            let marked = groups.iter().filter(|m| **m).count();
            set_extra_indent(items, &mut extra_indent, marked.max(1));
            items.push_signal(Signal::NewLine);
          }
          // a space directly after an opening paren is dropped
          after_open = false;
        } else if newlines > 0 {
          pending = Pending::Newline;
        } else if pending == Pending::None {
          pending = Pending::Space;
        }
      }
      TokenKind::Comma => {
        pending = Pending::None;
        items.push_string(",".to_string());
        after_open = false;
        emitted = true;
      }
      TokenKind::CloseParen => {
        let was_multi_line = groups.pop().unwrap_or(false);
        if was_multi_line {
          let marked = groups.iter().filter(|m| **m).count();
          set_extra_indent(items, &mut extra_indent, marked);
          items.push_signal(Signal::NewLine);
          pending = Pending::None;
          items.push_string(token.text.to_string());
          // the dedent applies to the closing paren line only
          set_extra_indent(items, &mut extra_indent, marked.max(1));
        } else {
          // a space before a closing paren is dropped, but an author
          // newline is kept; it may also be load bearing when a line
          // comment precedes the paren
          if pending == Pending::Newline {
            let marked = groups.iter().filter(|m| **m).count();
            set_extra_indent(items, &mut extra_indent, marked.max(1));
            items.push_signal(Signal::NewLine);
          }
          pending = Pending::None;
          items.push_string(token.text.to_string());
        }
        after_open = false;
        emitted = true;
      }
      _ => {
        match pending {
          Pending::Space => items.push_signal(Signal::SpaceOrNewLine),
          Pending::Newline => {
            let marked = groups.iter().filter(|m| **m).count();
            set_extra_indent(items, &mut extra_indent, marked.max(1));
            items.push_signal(Signal::NewLine);
          }
          Pending::None => {}
        }
        pending = Pending::None;
        if token.kind == TokenKind::Word {
          items.push_string(apply_keyword_case(token.text, ctx.keyword_case));
        } else {
          push_text(items, token.text);
        }
        // nothing may share a line with a line comment, or it would be
        // absorbed into the comment when the output is parsed again
        if token.kind == TokenKind::LineComment {
          pending = Pending::Newline;
        }
        let is_open = matches!(token.kind, TokenKind::OpenParen | TokenKind::Function);
        if is_open {
          groups.push(false);
        }
        after_open = is_open;
        emitted = true;
      }
    }
    if emitted && !first_emitted {
      items.push_signal(Signal::StartIndent);
      extra_indent += 1;
      first_emitted = true;
    }
  }
  set_extra_indent(items, &mut extra_indent, 0);
}

/// Applies the configured case to a word if and only if it is a known SQL
/// keyword. Quoted identifiers and function names are different token kinds
/// and can never be touched.
fn apply_keyword_case(word: &str, case: KeywordCase) -> String {
  match case {
    KeywordCase::Preserve => word.to_string(),
    KeywordCase::Upper if is_keyword(word) => word.to_ascii_uppercase(),
    KeywordCase::Lower if is_keyword(word) => word.to_ascii_lowercase(),
    _ => word.to_string(),
  }
}

fn set_extra_indent(items: &mut PrintItems, current: &mut usize, desired: usize) {
  while *current < desired {
    items.push_signal(Signal::StartIndent);
    *current += 1;
  }
  while *current > desired {
    items.push_signal(Signal::FinishIndent);
    *current -= 1;
  }
}

fn ends_with_line_comment(tokens: &[Token]) -> bool {
  tokens.last().is_some_and(|token| token.kind == TokenKind::LineComment)
}

/// Prints a comment, realigning the interior of a multi line comment
/// relative to the comment's new position.
fn push_comment(items: &mut PrintItems, ctx: &Context, token: &Token) {
  let text = token.text;
  if !text.contains('\n') {
    push_text(items, text);
    return;
  }
  let offset = text.as_ptr() as usize - ctx.source.as_ptr() as usize;
  let line_start = ctx.source[..offset].rfind('\n').map(|i| i + 1).unwrap_or(0);
  let original_column = ctx.source[line_start..offset].chars().count();
  let mut lines: Vec<&str> = text.split('\n').collect();
  while lines.len() > 1 && lines.last().is_some_and(|l| l.trim().is_empty()) {
    lines.pop();
  }
  let mut lines = lines.into_iter();
  if let Some(first) = lines.next() {
    push_text_line(items, first.trim_end());
  }
  for line in lines {
    items.push_signal(Signal::NewLine);
    let line = line.trim_end();
    let mut remaining = original_column;
    let line = line.trim_start_matches(|c: char| {
      if remaining > 0 && (c == ' ' || c == '\t') {
        remaining -= 1;
        true
      } else {
        false
      }
    });
    push_text_line(items, line);
  }
}

/// Pushes text that may contain newlines or tabs. Lines after the first are
/// printed verbatim without applying the current indentation level, and tabs
/// are sent as tab print items because the printer rejects raw tabs.
fn push_text(items: &mut PrintItems, text: &str) {
  if !text.contains('\n') {
    push_text_line(items, text);
    return;
  }
  let mut lines = text.split('\n');
  if let Some(first) = lines.next() {
    push_text_line(items, first.trim_end_matches('\r'));
  }
  items.push_signal(Signal::StartIgnoringIndent);
  for line in lines {
    items.push_signal(Signal::NewLine);
    push_text_line(items, line.trim_end_matches('\r'));
  }
  items.push_signal(Signal::FinishIgnoringIndent);
}

fn push_text_line(items: &mut PrintItems, line: &str) {
  let mut first = true;
  for part in line.split('\t') {
    if !first {
      items.push_signal(Signal::Tab);
    }
    first = false;
    if !part.is_empty() {
      items.push_string(part.to_string());
    }
  }
}
