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
use crate::configuration::ClauseStyle;
use crate::configuration::Configuration;
use crate::configuration::KeywordCase;

struct Context<'a> {
  source: &'a str,
  ignore_directive: &'a str,
  keyword_case: KeywordCase,
  clause_style: ClauseStyle,
}

pub fn generate(statements: &[Statement], source: &str, config: &Configuration) -> PrintItems {
  let mut items = PrintItems::new();
  let ctx = Context {
    source,
    ignore_directive: &config.ignore_node_comment_text,
    keyword_case: config.keyword_case,
    clause_style: config.clause_style,
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

/// Prints one clause. Author newlines are not consulted: lax-sql is
/// canonical, so the same query always formats the same way regardless of how
/// it was typed. The layout comes from the configured clause style.
fn gen_clause(tokens: &[Token], items: &mut PrintItems, ctx: &Context) {
  match ctx.clause_style {
    ClauseStyle::Fill => gen_clause_fill(tokens, items, ctx),
    ClauseStyle::Expanded => gen_clause_expanded(tokens, items, ctx),
  }
}

fn emit_token(token: &Token, items: &mut PrintItems, ctx: &Context) {
  if token.kind == TokenKind::Word {
    items.push_string(apply_keyword_case(token.text, ctx.keyword_case));
  } else {
    push_text(items, token.text);
  }
}

/// Fill style: the whole clause flows on one line and wraps at the configured
/// width, packing items until they no longer fit.
fn gen_clause_fill(tokens: &[Token], items: &mut PrintItems, ctx: &Context) {
  let mut flow = FlowPrinter::new(items, false);
  for token in tokens {
    let class = match token.kind {
      TokenKind::Whitespace { .. } => FlowClass::Whitespace { newlines: 0 },
      TokenKind::Comma => FlowClass::Comma,
      TokenKind::OpenParen | TokenKind::Function => FlowClass::Open,
      TokenKind::CloseParen => FlowClass::Close,
      TokenKind::LineComment => FlowClass::LineComment,
      _ => FlowClass::Other,
    };
    flow.token(items, class, |items| emit_token(token, items, ctx));
  }
  flow.finish(items);
}

/// Expanded style: the leading run of clause keywords stays on the clause
/// line, the body is indented on the following line, and each top level comma
/// separated item goes on its own line. Commas inside parens, like function
/// arguments, are left to fill within the group.
fn gen_clause_expanded(tokens: &[Token], items: &mut PrintItems, ctx: &Context) {
  // consume the leading run of clause keywords, like SELECT, INSERT INTO,
  // or GROUP BY, skipping whitespace; the rest is the body
  let mut head: Vec<&Token> = Vec::new();
  let mut idx = 0;
  loop {
    while idx < tokens.len() && matches!(tokens[idx].kind, TokenKind::Whitespace { .. }) {
      idx += 1;
    }
    match tokens.get(idx) {
      Some(t) if t.kind == TokenKind::Word && is_keyword(t.text) => {
        head.push(t);
        idx += 1;
      }
      _ => break,
    }
  }
  // with no leading keyword, fall back to fill so we never emit a dangling
  // indented body under nothing
  if head.is_empty() {
    gen_clause_fill(tokens, items, ctx);
    return;
  }
  for (i, token) in head.iter().enumerate() {
    if i > 0 {
      items.push_space();
    }
    emit_token(token, items, ctx);
  }
  // trim whitespace at the body edges so the indented line has no stray space
  let body = trim_ws_slice(&tokens[idx..]);
  if body.is_empty() {
    return;
  }
  // the body starts indented on its own line
  let mut flow = FlowPrinter::new(items, true);
  let mut depth = 0u32;
  for token in body {
    let class = match token.kind {
      TokenKind::Whitespace { .. } => FlowClass::Whitespace { newlines: 0 },
      TokenKind::Comma if depth == 0 => FlowClass::CommaBreak,
      TokenKind::Comma => FlowClass::Comma,
      TokenKind::OpenParen | TokenKind::Function => {
        depth += 1;
        FlowClass::Open
      }
      TokenKind::CloseParen => {
        depth = depth.saturating_sub(1);
        FlowClass::Close
      }
      TokenKind::LineComment => FlowClass::LineComment,
      _ => FlowClass::Other,
    };
    flow.token(items, class, |items| emit_token(token, items, ctx));
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

/// Returns the slice with leading and trailing whitespace tokens removed.
fn trim_ws_slice<'a>(tokens: &'a [Token<'a>]) -> &'a [Token<'a>] {
  let start = tokens
    .iter()
    .position(|t| !matches!(t.kind, TokenKind::Whitespace { .. }));
  let Some(start) = start else {
    return &[];
  };
  let end = tokens
    .iter()
    .rposition(|t| !matches!(t.kind, TokenKind::Whitespace { .. }))
    .unwrap();
  &tokens[start..=end]
}
