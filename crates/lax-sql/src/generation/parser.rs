use super::keywords::is_clause_starter;
use super::keywords::is_join_prefix;
use super::keywords::is_starter_suppressor;
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

#[derive(Debug)]
pub enum StatementKind<'a> {
  Sql {
    /// A statement split at top level clause keywords. Each clause starts
    /// with its keyword token and prints on its own line.
    clauses: Vec<Vec<Token<'a>>>,
    semicolon: bool,
  },
  Comment {
    token: Token<'a>,
  },
}

pub fn parse<'a>(tokens: &[Token<'a>], source: &'a str) -> Vec<Statement<'a>> {
  let mut parser = Parser { tokens, pos: 0, source };
  parser.parse_statements()
}

struct Parser<'b, 'a> {
  tokens: &'b [Token<'a>],
  pos: usize,
  source: &'a str,
}

impl<'b, 'a> Parser<'b, 'a> {
  fn byte_offset(&self, text: &str) -> usize {
    text.as_ptr() as usize - self.source.as_ptr() as usize
  }

  fn parse_statements(&mut self) -> Vec<Statement<'a>> {
    let mut statements: Vec<Statement<'a>> = Vec::new();
    let mut pending_newlines = 0u32;
    loop {
      while let Some(token) = self.tokens.get(self.pos) {
        if let TokenKind::Whitespace { newlines } = token.kind {
          pending_newlines += newlines;
          self.pos += 1;
        } else {
          break;
        }
      }
      let Some(token) = self.tokens.get(self.pos).copied() else {
        break;
      };
      let kind = match token.kind {
        TokenKind::LineComment | TokenKind::BlockComment => {
          self.pos += 1;
          StatementKind::Comment { token }
        }
        _ => self.parse_sql_statement(),
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

  /// Consumes tokens up to and including a top level semicolon, splitting
  /// the statement at top level clause keywords.
  fn parse_sql_statement(&mut self) -> StatementKind<'a> {
    let mut clauses: Vec<Vec<Token<'a>>> = Vec::new();
    let mut current: Vec<Token<'a>> = Vec::new();
    let mut depth = 0u32;
    let mut semicolon = false;
    let mut first_word: Option<&'a str> = None;
    let mut prev_significant: Option<Token<'a>> = None;
    while let Some(token) = self.tokens.get(self.pos).copied() {
      match token.kind {
        TokenKind::OpenParen | TokenKind::Function => depth += 1,
        TokenKind::CloseParen => depth = depth.saturating_sub(1),
        TokenKind::Semicolon if depth == 0 => {
          self.pos += 1;
          semicolon = true;
          break;
        }
        // a clause that is only join prefixes so far, like `left outer`,
        // must not be split again at the following `join`
        TokenKind::Word
          if depth == 0
            && !current.is_empty()
            && self.starts_clause(&token, first_word, prev_significant)
            && !only_join_prefixes(&current) =>
        {
          clauses.push(trim_ws(&current));
          current.clear();
        }
        _ => {}
      }
      if first_word.is_none() && token.kind == TokenKind::Word {
        first_word = Some(token.text);
      }
      if !matches!(token.kind, TokenKind::Whitespace { .. }) {
        prev_significant = Some(token);
      }
      current.push(token);
      self.pos += 1;
    }
    let current = trim_ws(&current);
    if !current.is_empty() {
      clauses.push(current);
    }
    StatementKind::Sql { clauses, semicolon }
  }

  /// A clause starter begins a new line. A join prefix like `left` only
  /// starts a clause when the upcoming words lead into a `join`, so that a
  /// column named `left` does not force a line break. A starter directly
  /// after `on`, `do`, or a dot is part of another construct, like
  /// `ON DELETE SET NULL` or a qualified name, and never splits.
  fn starts_clause(&self, token: &Token<'a>, first_word: Option<&str>, prev_significant: Option<Token<'a>>) -> bool {
    if let Some(prev) = prev_significant {
      let suppressed = match prev.kind {
        TokenKind::Word => is_starter_suppressor(prev.text),
        TokenKind::Delim => prev.text == ".",
        _ => false,
      };
      if suppressed {
        return false;
      }
    }
    if token.text.eq_ignore_ascii_case("set") {
      // `set` is a clause only in an update statement; in
      // `ALTER COLUMN x SET NOT NULL` and friends it must stay inline
      return first_word.is_some_and(|w| w.eq_ignore_ascii_case("update"));
    }
    if is_clause_starter(token.text) {
      return true;
    }
    if !is_join_prefix(token.text) {
      return false;
    }
    let mut index = self.pos + 1;
    while let Some(next) = self.tokens.get(index) {
      match next.kind {
        TokenKind::Whitespace { .. } => index += 1,
        TokenKind::Word if next.text.eq_ignore_ascii_case("join") => {
          return true;
        }
        TokenKind::Word if is_join_prefix(next.text) => index += 1,
        _ => return false,
      }
    }
    false
  }
}

fn only_join_prefixes(tokens: &[Token<'_>]) -> bool {
  let trimmed = trim_ws(tokens);
  !trimmed.is_empty()
    && trimmed.iter().all(|token| match token.kind {
      TokenKind::Word => is_join_prefix(token.text),
      TokenKind::Whitespace { .. } => true,
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
