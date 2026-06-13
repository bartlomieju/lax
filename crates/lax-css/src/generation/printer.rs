use dprint_core::formatting::PrintItems;
use dprint_core::formatting::Signal;
use lax_core::FlowClass;
use lax_core::FlowPrinter;
use lax_core::contains_directive;
use lax_core::push_comment;
use lax_core::push_text;

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
      && contains_directive(token.text, ctx.ignore_directive)
    {
      ignore_next = true;
    }
    gen_statement(statement, items, ctx);
    if let Some(comment) = statement.trailing_comment {
      items.push_space();
      push_comment(items, ctx.source, comment.text);
    }
    items.push_signal(Signal::NewLine);
  }
}

fn gen_statement(statement: &Statement, items: &mut PrintItems, ctx: &Context) {
  match &statement.kind {
    StatementKind::Comment { token } => push_comment(items, ctx.source, token.text),
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
    // an empty rule collapses to `{}` rather than spanning two lines
    if block.closed {
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
  // whether any visible token has been emitted yet on this selector, used so
  // a selector that starts with a combinator (nesting `> *`) gets no leading
  // space
  let mut emitted_any = false;
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
        // a child or sibling combinator at the top level of a selector gets a
        // single space on each side, however the author spaced it. Like the
        // space after a declaration colon, this is whitespace normalization,
        // not token reinterpretation. A `>`, `+`, or `~` nested in parens or
        // brackets (nth-child(2n+1), [a~=b], :not(a>b)) has depth above zero
        // and is left untouched.
        let is_combinator = depth == 0 && token.kind == TokenKind::Delim && matches!(token.text, ">" | "+" | "~");
        if is_open {
          depth += 1;
        }
        // a combinator gets a leading space only when it follows something;
        // a selector that starts with one (nesting `> *`) takes no leading space
        if pending_space || (is_combinator && emitted_any) {
          items.push_space();
          pending_space = false;
        }
        push_text(items, token.text);
        emitted_any = true;
        if is_combinator {
          items.push_space();
          swallow_ws = true;
        } else if token.kind == TokenKind::LineComment {
          // nothing may share a line with a line comment, or it would be
          // absorbed into the comment when the output is parsed again
          items.push_signal(Signal::NewLine);
          swallow_ws = true;
        } else {
          swallow_ws = is_open;
        }
      }
    }
  }
}

/// Prints a declaration value, at-rule prelude, or raw statement through
/// the shared flow printer, which preserves author newlines, wraps at
/// author spaces, and indents multi line paren groups per depth.
fn gen_value(tokens: &[Token], items: &mut PrintItems, starts_on_new_line: bool) {
  let mut flow = FlowPrinter::new(items, starts_on_new_line);
  for token in tokens {
    let class = match token.kind {
      TokenKind::Whitespace { newlines } => FlowClass::Whitespace { newlines },
      TokenKind::Comma => FlowClass::Comma,
      TokenKind::OpenParen | TokenKind::OpenBracket | TokenKind::Function => FlowClass::Open,
      TokenKind::CloseParen | TokenKind::CloseBracket => FlowClass::Close,
      TokenKind::LineComment => FlowClass::LineComment,
      _ => FlowClass::Other,
    };
    flow.token(items, class, |items| push_text(items, token.text));
  }
  flow.finish(items);
}

fn ends_with_line_comment(tokens: &[Token]) -> bool {
  tokens.last().is_some_and(|token| token.kind == TokenKind::LineComment)
}

/// Prints a declaration list on a single line, for inline `style=""`
/// attributes where newlines are not allowed. Whitespace around colons and
/// between declarations is normalized to a single space, but nothing is
/// wrapped, indented, or split. Like the rest of the printer it never
/// reinterprets or drops tokens; a `;` the author omitted is not invented.
pub fn generate_inline(statements: &[Statement], source: &str) -> String {
  let mut parts: Vec<String> = Vec::new();
  for statement in statements {
    match &statement.kind {
      StatementKind::Declaration {
        name,
        value,
        terminated,
        ..
      } => {
        let mut part = collapse_tokens(name);
        part.push(':');
        let value = collapse_tokens(value);
        if !value.is_empty() {
          part.push(' ');
          part.push_str(&value);
        }
        if *terminated {
          part.push(';');
        }
        if !part.is_empty() {
          parts.push(part);
        }
      }
      StatementKind::Raw { tokens, semicolon } => {
        let mut part = collapse_tokens(tokens);
        if *semicolon {
          part.push(';');
        }
        if !part.is_empty() {
          parts.push(part);
        }
      }
      // at-rules, qualified rules and comments are unusual inside an inline
      // style attribute; pass them through verbatim with whitespace collapsed
      // so unexpected input still round-trips without being mangled.
      _ => {
        let part = collapse_whitespace(&source[statement.span.0..statement.span.1]);
        if !part.is_empty() {
          parts.push(part);
        }
      }
    }
  }
  parts.join(" ")
}

/// Joins token text, replacing each run of whitespace with a single space and
/// trimming the ends, without inserting spaces where the source had none.
fn collapse_tokens(tokens: &[Token]) -> String {
  let mut out = String::new();
  let mut pending_space = false;
  for token in tokens {
    match token.kind {
      TokenKind::Whitespace { .. } => {
        if !out.is_empty() {
          pending_space = true;
        }
      }
      _ => {
        if pending_space {
          out.push(' ');
          pending_space = false;
        }
        out.push_str(token.text);
      }
    }
  }
  out
}

/// Collapses every run of whitespace in a string to a single space and trims.
fn collapse_whitespace(text: &str) -> String {
  text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Prints tokens exactly as they appeared in the source.
fn gen_verbatim(tokens: &[Token], items: &mut PrintItems) {
  let mut text = String::new();
  for token in tokens {
    text.push_str(token.text);
  }
  push_text(items, &text);
}
