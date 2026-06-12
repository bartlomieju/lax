use dprint_core::formatting::PrintItems;
use dprint_core::formatting::Signal;
use lax_core::FlowClass;
use lax_core::FlowPrinter;
use lax_core::contains_directive;
use lax_core::push_comment;
use lax_core::push_text;

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

/// Prints one clause of a statement through the shared flow printer.
fn gen_clause(tokens: &[Token], items: &mut PrintItems, ctx: &Context) {
  let mut flow = FlowPrinter::new(items, false);
  for token in tokens {
    let class = match token.kind {
      TokenKind::Whitespace { newlines } => FlowClass::Whitespace { newlines },
      TokenKind::Comma => FlowClass::Comma,
      TokenKind::OpenParen | TokenKind::Function => FlowClass::Open,
      TokenKind::CloseParen => FlowClass::Close,
      TokenKind::LineComment => FlowClass::LineComment,
      _ => FlowClass::Other,
    };
    flow.token(items, class, |items| {
      if token.kind == TokenKind::Word {
        items.push_string(apply_keyword_case(token.text, ctx.keyword_case));
      } else {
        push_text(items, token.text);
      }
    });
  }
  flow.finish(items);
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

fn ends_with_line_comment(tokens: &[Token]) -> bool {
  tokens.last().is_some_and(|token| token.kind == TokenKind::LineComment)
}
