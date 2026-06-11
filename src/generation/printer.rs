use dprint_core::formatting::PrintItems;
use dprint_core::formatting::Signal;

use super::parser::Block;
use super::parser::Statement;
use super::parser::StatementKind;
use super::tokenizer::Token;
use super::tokenizer::TokenKind;
use crate::configuration::Configuration;

pub struct Context<'a> {
  pub source: &'a str,
  pub ignore_directive: &'a str,
}

pub fn generate(statements: &[Statement], source: &str, config: &Configuration) -> PrintItems {
  let mut items = PrintItems::new();
  let ctx = Context {
    source,
    ignore_directive: &config.ignore_node_comment_text,
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
      push_text(items, comment.text);
    }
    items.push_signal(Signal::NewLine);
  }
}

fn gen_statement(statement: &Statement, items: &mut PrintItems, ctx: &Context) {
  match &statement.kind {
    StatementKind::Comment { token } => push_text(items, token.text),
    StatementKind::QualifiedRule { prelude, block } => {
      if !prelude.is_empty() {
        gen_selector(prelude, items);
        if ends_with_line_comment(prelude) {
          items.push_signal(Signal::NewLine);
        } else {
          items.push_space();
        }
      }
      gen_block(block, items, ctx);
    }
    StatementKind::AtRule {
      name,
      prelude,
      block,
      terminated,
    } => {
      items.push_string(name.text.to_string());
      if !prelude.is_empty() {
        items.push_space();
        gen_value(prelude, items, false);
      }
      let after_line_comment = ends_with_line_comment(prelude);
      match block {
        Some(block) => {
          if after_line_comment {
            items.push_signal(Signal::NewLine);
          } else {
            items.push_space();
          }
          gen_block(block, items, ctx);
        }
        None => {
          if *terminated {
            if after_line_comment {
              items.push_signal(Signal::NewLine);
            }
            items.push_string(";".to_string());
          }
        }
      }
    }
    StatementKind::Declaration {
      name,
      value,
      verbatim_value,
      value_on_new_line,
      terminated,
    } => {
      gen_selector(name, items);
      items.push_string(":".to_string());
      if !value.is_empty() {
        if *verbatim_value {
          if *value_on_new_line {
            items.push_signal(Signal::StartIndent);
            items.push_signal(Signal::NewLine);
            gen_verbatim(value, items);
            items.push_signal(Signal::FinishIndent);
          } else {
            items.push_space();
            gen_verbatim(value, items);
          }
        } else {
          if !*value_on_new_line {
            items.push_space();
          }
          gen_value(value, items, *value_on_new_line);
          if *terminated && ends_with_line_comment(value) {
            items.push_signal(Signal::NewLine);
          }
        }
      }
      if *terminated {
        items.push_string(";".to_string());
      }
    }
    StatementKind::Raw { tokens, semicolon } => {
      gen_value(tokens, items, false);
      if *semicolon {
        if ends_with_line_comment(tokens) {
          items.push_signal(Signal::NewLine);
        }
        items.push_string(";".to_string());
      }
    }
  }
}

fn gen_block(block: &Block, items: &mut PrintItems, ctx: &Context) {
  items.push_string("{".to_string());
  if block.body.is_empty() {
    if block.closed {
      items.push_signal(Signal::NewLine);
      items.push_string("}".to_string());
    }
  } else {
    items.push_signal(Signal::StartIndent);
    items.push_signal(Signal::NewLine);
    gen_statements(&block.body, items, ctx);
    items.push_signal(Signal::FinishIndent);
    if block.closed {
      items.push_string("}".to_string());
    }
  }
}

/// Prints a selector or declaration name. Top level commas put each selector
/// on its own line. No wrapping happens inside a selector; author whitespace
/// collapses to a single space.
fn gen_selector(tokens: &[Token], items: &mut PrintItems) {
  let mut depth = 0u32;
  let mut pending_space = false;
  let mut swallow_ws = false;
  for token in tokens {
    match token.kind {
      TokenKind::Whitespace { .. } => {
        if !swallow_ws {
          pending_space = true;
        }
      }
      TokenKind::Comma => {
        pending_space = false;
        items.push_string(",".to_string());
        if depth == 0 {
          items.push_signal(Signal::NewLine);
          swallow_ws = true;
        } else {
          swallow_ws = false;
        }
      }
      TokenKind::CloseParen | TokenKind::CloseBracket => {
        depth = depth.saturating_sub(1);
        pending_space = false;
        items.push_string(token.text.to_string());
        swallow_ws = false;
      }
      _ => {
        let is_open = matches!(
          token.kind,
          TokenKind::OpenParen | TokenKind::OpenBracket | TokenKind::Function
        );
        if is_open {
          depth += 1;
        }
        if pending_space {
          items.push_space();
          pending_space = false;
        }
        push_text(items, token.text);
        // nothing may share a line with a line comment, or it would be
        // absorbed into the comment when the output is parsed again
        if token.kind == TokenKind::LineComment {
          items.push_signal(Signal::NewLine);
          swallow_ws = true;
        } else {
          swallow_ws = is_open;
        }
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

/// Prints a declaration value, at-rule prelude, or raw statement.
///
/// Whitespace handling follows the lax policy:
/// - a single author space becomes a possible line break point when the line
///   exceeds the configured width, since changing a space to a newline is a
///   whitespace only change
/// - an author newline is preserved as a newline, so hand formatted values
///   like multi line font stacks or grid-template-areas keep their shape
/// - a line break is never introduced where the author had no whitespace
///
/// Continuation lines are indented one level. A paren or bracket group that
/// the author opened with a newline indents its contents one level per
/// nesting depth and puts the closing paren back at the start level.
fn gen_value(tokens: &[Token], items: &mut PrintItems, starts_on_new_line: bool) {
  // the continuation indent starts after the first item is written, so that
  // a value that starts a line, like a raw statement, is not itself indented
  let mut extra_indent = 0usize;
  if starts_on_new_line {
    items.push_signal(Signal::StartIndent);
    items.push_signal(Signal::NewLine);
    extra_indent = 1;
  }
  let mut pending = Pending::None;
  let mut after_open = false;
  let mut first_emitted = starts_on_new_line;
  // one entry per open paren or bracket; true when the group is multi line
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
      TokenKind::CloseParen | TokenKind::CloseBracket => {
        let was_multi_line = groups.pop().unwrap_or(false);
        if was_multi_line {
          let marked = groups.iter().filter(|m| **m).count();
          set_extra_indent(items, &mut extra_indent, marked);
          items.push_signal(Signal::NewLine);
          pending = Pending::None;
          items.push_string(token.text.to_string());
          // the dedent applies to the closing paren line only; restore the
          // continuation baseline so later breaks land at a stable level
          set_extra_indent(items, &mut extra_indent, marked.max(1));
        } else {
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
        push_text(items, token.text);
        // nothing may share a line with a line comment, or it would be
        // absorbed into the comment when the output is parsed again
        if token.kind == TokenKind::LineComment {
          pending = Pending::Newline;
        }
        let is_open = matches!(
          token.kind,
          TokenKind::OpenParen | TokenKind::OpenBracket | TokenKind::Function
        );
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

/// Prints tokens exactly as they appeared in the source.
fn gen_verbatim(tokens: &[Token], items: &mut PrintItems) {
  let mut text = String::new();
  for token in tokens {
    text.push_str(token.text);
  }
  push_text(items, &text);
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
