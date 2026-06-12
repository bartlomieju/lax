use super::tokenizer::Token;
use super::tokenizer::TokenKind;

#[derive(Debug)]
pub struct Statement<'a> {
  pub blank_line_before: bool,
  pub trailing_comment: Option<Token<'a>>,
  pub kind: StatementKind<'a>,
  /// Byte range of this statement in the source, including an attached
  /// trailing comment. Used to print a statement verbatim when it is
  /// preceded by an ignore comment.
  pub span: (usize, usize),
}

/// A block body together with whether the source actually closed it. An
/// unclosed block at the end of the file is printed without a closing brace
/// so that truncated input stays stable across formatting passes.
#[derive(Debug)]
pub struct Block<'a> {
  pub body: Vec<Statement<'a>>,
  pub closed: bool,
}

#[derive(Debug)]
pub enum StatementKind<'a> {
  QualifiedRule {
    prelude: Vec<Token<'a>>,
    block: Block<'a>,
  },
  AtRule {
    name: Token<'a>,
    prelude: Vec<Token<'a>>,
    block: Option<Block<'a>>,
    /// False when the at-rule was cut off by the end of the file, in which
    /// case no semicolon is added.
    terminated: bool,
  },
  Declaration {
    name: Vec<Token<'a>>,
    value: Vec<Token<'a>>,
    /// Custom property values are preserved verbatim.
    verbatim_value: bool,
    /// The author put the value on its own line after the colon.
    value_on_new_line: bool,
    /// False when the declaration was cut off by the end of the file, in
    /// which case no semicolon is added.
    terminated: bool,
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

pub fn parse<'a>(tokens: &[Token<'a>], source: &'a str) -> Vec<Statement<'a>> {
  let mut parser = Parser { tokens, pos: 0, source };
  parser.parse_statements(true).body
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
  source: &'a str,
}

impl<'b, 'a> Parser<'b, 'a> {
  fn peek(&self) -> Option<&Token<'a>> {
    self.tokens.get(self.pos)
  }

  fn parse_statements(&mut self, top_level: bool) -> Block<'a> {
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
            return Block {
              body: statements,
              closed: true,
            };
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
      let span_start = self.byte_offset(token.text);
      let last_token = &self.tokens[self.pos - 1];
      let span_end = self.byte_offset(last_token.text) + last_token.text.len();
      statements.push(Statement {
        blank_line_before,
        trailing_comment,
        kind,
        span: (span_start, span_end),
      });
    }
    Block {
      body: statements,
      closed: top_level,
    }
  }

  fn byte_offset(&self, text: &str) -> usize {
    text.as_ptr() as usize - self.source.as_ptr() as usize
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
            block: None,
            terminated: true,
          };
        }
        TokenKind::OpenBrace => {
          if depth == 0 {
            let prelude = trim_ws(&self.tokens[prelude_start..self.pos]);
            self.pos += 1;
            let block = self.parse_statements(false);
            return StatementKind::AtRule {
              name,
              prelude,
              block: Some(block),
              terminated: true,
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
              block: None,
              terminated: true,
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
      block: None,
      terminated: false,
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
        let block = self.parse_statements(false);
        Some(StatementKind::QualifiedRule { prelude, block })
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
            let value_on_new_line = !value.is_empty()
              && matches!(
                self.tokens.get(colon + 1),
                Some(Token { kind: TokenKind::Whitespace { newlines }, .. }) if *newlines > 0
              );
            Some(StatementKind::Declaration {
              name,
              value,
              verbatim_value,
              value_on_new_line,
              terminated: !matches!(terminator, Terminator::Eof(_)),
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
