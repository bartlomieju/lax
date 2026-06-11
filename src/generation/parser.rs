use super::tokenizer::Token;
use super::tokenizer::TokenKind;

#[derive(Debug)]
pub struct Statement<'a> {
  pub blank_line_before: bool,
  pub trailing_comment: Option<Token<'a>>,
  pub kind: StatementKind<'a>,
}

#[derive(Debug)]
pub enum StatementKind<'a> {
  QualifiedRule {
    prelude: Vec<Token<'a>>,
    body: Vec<Statement<'a>>,
  },
  AtRule {
    name: Token<'a>,
    prelude: Vec<Token<'a>>,
    body: Option<Vec<Statement<'a>>>,
  },
  Declaration {
    name: Vec<Token<'a>>,
    value: Vec<Token<'a>>,
    /// Custom property values are preserved verbatim.
    verbatim_value: bool,
  },
  Comment {
    token: Token<'a>,
  },
  /// Anything that is not recognized is passed through as raw tokens.
  Raw {
    tokens: Vec<Token<'a>>,
    semicolon: bool,
  },
}

pub fn parse<'a>(tokens: &[Token<'a>]) -> Vec<Statement<'a>> {
  let mut parser = Parser { tokens, pos: 0 };
  parser.parse_statements(true)
}

enum Terminator {
  Semicolon(usize),
  OpenBrace(usize),
  CloseBrace(usize),
  Eof(usize),
}

struct Parser<'b, 'a> {
  tokens: &'b [Token<'a>],
  pos: usize,
}

impl<'b, 'a> Parser<'b, 'a> {
  fn peek(&self) -> Option<&Token<'a>> {
    self.tokens.get(self.pos)
  }

  fn parse_statements(&mut self, top_level: bool) -> Vec<Statement<'a>> {
    let mut statements: Vec<Statement<'a>> = Vec::new();
    let mut pending_newlines = 0u32;
    loop {
      while let Some(token) = self.peek() {
        if let TokenKind::Whitespace { newlines } = token.kind {
          pending_newlines += newlines;
          self.pos += 1;
        } else {
          break;
        }
      }
      let Some(token) = self.peek().copied() else {
        break;
      };
      let kind = match token.kind {
        TokenKind::CloseBrace => {
          self.pos += 1;
          if !top_level {
            return statements;
          }
          // a stray close brace at the top level is kept as raw text
          StatementKind::Raw {
            tokens: vec![token],
            semicolon: false,
          }
        }
        TokenKind::LineComment | TokenKind::BlockComment => {
          self.pos += 1;
          StatementKind::Comment { token }
        }
        // an at keyword directly followed by a colon is a Less variable
        // declaration, not an at-rule
        TokenKind::AtKeyword if !matches!(self.tokens.get(self.pos + 1).map(|t| t.kind), Some(TokenKind::Colon)) => {
          self.pos += 1;
          self.parse_at_rule(token)
        }
        _ => match self.parse_statement_kind() {
          Some(kind) => kind,
          None => {
            pending_newlines = 0;
            continue;
          }
        },
      };
      let blank_line_before = pending_newlines >= 2 && !statements.is_empty();
      pending_newlines = 0;
      let trailing_comment = self.try_take_trailing_comment();
      statements.push(Statement {
        blank_line_before,
        trailing_comment,
        kind,
      });
    }
    statements
  }

  fn try_take_trailing_comment(&mut self) -> Option<Token<'a>> {
    let mut index = self.pos;
    if let Some(token) = self.tokens.get(index)
      && matches!(token.kind, TokenKind::Whitespace { newlines: 0 })
    {
      index += 1;
    }
    match self.tokens.get(index) {
      Some(token) if matches!(token.kind, TokenKind::LineComment | TokenKind::BlockComment) => {
        self.pos = index + 1;
        Some(*token)
      }
      _ => None,
    }
  }

  fn parse_at_rule(&mut self, name: Token<'a>) -> StatementKind<'a> {
    let prelude_start = self.pos;
    let mut depth = 0u32;
    while let Some(token) = self.peek() {
      match token.kind {
        TokenKind::OpenParen | TokenKind::OpenBracket | TokenKind::Function => {
          depth += 1;
        }
        TokenKind::CloseParen | TokenKind::CloseBracket => {
          depth = depth.saturating_sub(1);
        }
        TokenKind::Semicolon if depth == 0 => {
          let prelude = trim_ws(&self.tokens[prelude_start..self.pos]);
          self.pos += 1;
          return StatementKind::AtRule {
            name,
            prelude,
            body: None,
          };
        }
        TokenKind::OpenBrace => {
          if depth == 0 {
            let prelude = trim_ws(&self.tokens[prelude_start..self.pos]);
            self.pos += 1;
            let body = self.parse_statements(false);
            return StatementKind::AtRule {
              name,
              prelude,
              body: Some(body),
            };
          }
          depth += 1;
        }
        TokenKind::CloseBrace => {
          if depth == 0 {
            // leave the brace for the enclosing block to consume
            let prelude = trim_ws(&self.tokens[prelude_start..self.pos]);
            return StatementKind::AtRule {
              name,
              prelude,
              body: None,
            };
          }
          depth -= 1;
        }
        _ => {}
      }
      self.pos += 1;
    }
    let prelude = trim_ws(&self.tokens[prelude_start..self.pos]);
    StatementKind::AtRule {
      name,
      prelude,
      body: None,
    }
  }

  /// Parses a qualified rule, declaration, or raw statement by scanning
  /// ahead for the first top level open brace, semicolon, or close brace.
  fn parse_statement_kind(&mut self) -> Option<StatementKind<'a>> {
    let start = self.pos;
    let mut depth = 0u32;
    let mut colon_index: Option<usize> = None;
    let mut index = self.pos;
    let terminator = loop {
      let Some(token) = self.tokens.get(index) else {
        break Terminator::Eof(index);
      };
      match token.kind {
        TokenKind::OpenParen | TokenKind::OpenBracket | TokenKind::Function => {
          depth += 1;
        }
        TokenKind::CloseParen | TokenKind::CloseBracket => {
          depth = depth.saturating_sub(1);
        }
        TokenKind::OpenBrace => {
          if depth == 0 {
            break Terminator::OpenBrace(index);
          }
          depth += 1;
        }
        TokenKind::CloseBrace => {
          if depth == 0 {
            break Terminator::CloseBrace(index);
          }
          depth -= 1;
        }
        TokenKind::Semicolon if depth == 0 => {
          break Terminator::Semicolon(index);
        }
        TokenKind::Colon if depth == 0 && colon_index.is_none() => {
          colon_index = Some(index);
        }
        _ => {}
      }
      index += 1;
    };
    match terminator {
      Terminator::OpenBrace(end) => {
        let prelude = trim_ws(&self.tokens[start..end]);
        self.pos = end + 1;
        let body = self.parse_statements(false);
        Some(StatementKind::QualifiedRule { prelude, body })
      }
      Terminator::Semicolon(end) | Terminator::CloseBrace(end) | Terminator::Eof(end) => {
        let had_semicolon = matches!(terminator, Terminator::Semicolon(_));
        self.pos = if had_semicolon { end + 1 } else { end };
        let segment = &self.tokens[start..end];
        if trim_ws(segment).is_empty() {
          return None;
        }
        match colon_index {
          Some(colon) if is_declaration_name(&trim_ws(&self.tokens[start..colon])) => {
            let name = trim_ws(&self.tokens[start..colon]);
            let value = trim_ws(&self.tokens[colon + 1..end]);
            let verbatim_value = name.len() == 1 && name[0].kind == TokenKind::Ident && name[0].text.starts_with("--");
            Some(StatementKind::Declaration {
              name,
              value,
              verbatim_value,
            })
          }
          _ => Some(StatementKind::Raw {
            tokens: trim_ws(segment),
            semicolon: had_semicolon,
          }),
        }
      }
    }
  }
}

/// A declaration name must look like a property or variable. Anything
/// selector-ish, like `&:extend(.foo)` in Less, falls through to a raw
/// statement instead.
fn is_declaration_name(tokens: &[Token<'_>]) -> bool {
  !tokens.is_empty()
    && tokens.iter().all(|token| match token.kind {
      TokenKind::Ident | TokenKind::AtKeyword | TokenKind::Interpolation => true,
      TokenKind::Delim => matches!(token.text, "$" | "*" | "_"),
      _ => false,
    })
}

fn trim_ws<'a>(tokens: &[Token<'a>]) -> Vec<Token<'a>> {
  let start = tokens
    .iter()
    .position(|t| !matches!(t.kind, TokenKind::Whitespace { .. }));
  let Some(start) = start else {
    return Vec::new();
  };
  let end = tokens
    .iter()
    .rposition(|t| !matches!(t.kind, TokenKind::Whitespace { .. }))
    .unwrap();
  tokens[start..=end].to_vec()
}
