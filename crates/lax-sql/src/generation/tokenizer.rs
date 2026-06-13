#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenKind {
  Whitespace {
    newlines: u32,
  },
  /// `--` to the end of the line.
  LineComment,
  /// `/* ... */`, nested per the SQL standard.
  BlockComment,
  /// An identifier or keyword.
  Word,
  /// A word immediately followed by an open paren. The token text includes
  /// the paren. Function names are never treated as keywords.
  Function,
  /// `"..."` or backtick quoted identifier, kept as one opaque token.
  QuotedIdent,
  /// `[...]`, a T-SQL identifier or an array subscript, kept opaque.
  Bracketed,
  /// `'...'` or dollar quoted `$tag$...$tag$` string, kept opaque.
  Str,
  Number,
  Comma,
  Semicolon,
  OpenParen,
  CloseParen,
  Delim,
}

#[derive(Debug, Clone, Copy)]
pub struct Token<'a> {
  pub kind: TokenKind,
  pub text: &'a str,
}

pub fn tokenize(text: &str) -> Vec<Token<'_>> {
  let bytes = text.as_bytes();
  let mut tokens: Vec<Token> = Vec::new();
  let mut i = 0;
  while i < bytes.len() {
    let start = i;
    let b = bytes[i];
    let kind = if is_whitespace(b) {
      let mut newlines = 0;
      while i < bytes.len() && is_whitespace(bytes[i]) {
        if bytes[i] == b'\n' {
          newlines += 1;
        }
        i += 1;
      }
      TokenKind::Whitespace { newlines }
    } else if (b == b'-' && peek(bytes, i + 1) == Some(b'-')) || (b == b'#' && is_hash_comment(bytes, i)) {
      while i < bytes.len() && bytes[i] != b'\n' {
        i += 1;
      }
      TokenKind::LineComment
    } else if b == b'/' && peek(bytes, i + 1) == Some(b'*') {
      i = scan_block_comment(bytes, i + 2);
      TokenKind::BlockComment
    } else if b == b'\'' {
      // a string directly prefixed with E uses backslash escapes
      let backslash_escapes = tokens
        .last()
        .is_some_and(|t| t.kind == TokenKind::Word && t.text.eq_ignore_ascii_case("e"));
      i = scan_string(bytes, i + 1, backslash_escapes);
      TokenKind::Str
    } else if b == b'"' || b == b'`' {
      i = scan_quoted_ident(bytes, i + 1, b);
      TokenKind::QuotedIdent
    } else if b == b'[' {
      while i < bytes.len() && bytes[i] != b']' {
        i += 1;
      }
      if i < bytes.len() {
        i += 1;
      }
      TokenKind::Bracketed
    } else if b == b'$' && dollar_quote_end(bytes, i).is_some() {
      let tag_end = dollar_quote_end(bytes, i).unwrap();
      i = scan_dollar_string(text, i, tag_end);
      TokenKind::Str
    } else if b.is_ascii_digit() || (b == b'.' && peek_digit(bytes, i + 1)) {
      i = scan_number(bytes, i + 1);
      TokenKind::Number
    } else if is_word_start(b) {
      i = scan_word(bytes, i);
      if peek(bytes, i) == Some(b'(') {
        i += 1;
        TokenKind::Function
      } else {
        TokenKind::Word
      }
    } else {
      i += utf8_len(b);
      match b {
        b',' => TokenKind::Comma,
        b';' => TokenKind::Semicolon,
        b'(' => TokenKind::OpenParen,
        b')' => TokenKind::CloseParen,
        _ => TokenKind::Delim,
      }
    };
    tokens.push(Token {
      kind,
      text: &text[start..i],
    });
  }
  tokens
}

/// `#` starts a MySQL style line comment when it is followed by whitespace or
/// ends the input. T-SQL temporary tables like `#temp`, identifiers, and tight
/// operators like the PostgreSQL `#>` stay ordinary tokens.
///
/// This rule is intentionally position independent: a `#` led token means the
/// same thing whether it is at the start of a line or mid line. Depending on
/// line position would make formatting non idempotent, since reformatting can
/// move a `#temp` reference to the start of a line. The cost is that a MySQL
/// `#comment` with no following space is read as tokens rather than a comment,
/// which is an irreducible dialect ambiguity.
fn is_hash_comment(bytes: &[u8], i: usize) -> bool {
  match peek(bytes, i + 1) {
    None => true,
    Some(next) => is_whitespace(next),
  }
}

fn is_whitespace(b: u8) -> bool {
  matches!(b, b' ' | b'\t' | b'\r' | b'\n' | b'\x0c')
}

fn is_word_start(b: u8) -> bool {
  b.is_ascii_alphabetic() || b == b'_' || b >= 0x80
}

fn is_word_char(b: u8) -> bool {
  b.is_ascii_alphanumeric() || b == b'_' || b == b'$' || b >= 0x80
}

fn peek(bytes: &[u8], i: usize) -> Option<u8> {
  bytes.get(i).copied()
}

fn peek_digit(bytes: &[u8], i: usize) -> bool {
  bytes.get(i).is_some_and(|b| b.is_ascii_digit())
}

fn utf8_len(b: u8) -> usize {
  if b < 0x80 {
    1
  } else if b >= 0xF0 {
    4
  } else if b >= 0xE0 {
    3
  } else if b >= 0xC0 {
    2
  } else {
    1
  }
}

fn scan_word(bytes: &[u8], mut i: usize) -> usize {
  while i < bytes.len() && is_word_char(bytes[i]) {
    i += 1;
  }
  i
}

fn scan_number(bytes: &[u8], mut i: usize) -> usize {
  while i < bytes.len() {
    let b = bytes[i];
    if b.is_ascii_digit() {
      i += 1;
    } else if b == b'.' && peek_digit(bytes, i + 1) {
      i += 2;
    } else {
      break;
    }
  }
  i
}

/// Scans a single quoted string with standard SQL semantics: a quote is
/// escaped by doubling it and a backslash is a literal character. This is
/// what the standard, PostgreSQL, Oracle, SQLite, and T-SQL do; MySQL
/// backslash escaped quotes are the one dialect form that can mis-scan, and
/// MySQL also supports the portable doubled form.
fn scan_string(bytes: &[u8], mut i: usize, backslash_escapes: bool) -> usize {
  while i < bytes.len() {
    if backslash_escapes && bytes[i] == b'\\' {
      i += 1;
      if i < bytes.len() {
        i += utf8_len(bytes[i]);
      }
    } else if bytes[i] == b'\'' {
      if peek(bytes, i + 1) == Some(b'\'') {
        i += 2;
      } else {
        return i + 1;
      }
    } else {
      i += 1;
    }
  }
  i
}

/// Scans a double quote or backtick quoted region. Doubling escapes the
/// quote per the standard, and a backslash skips the next character because
/// Hive, BigQuery, and MySQL use double quoted strings with backslash
/// escapes; a standard quoted identifier ending in a literal backslash is
/// pathological by comparison.
fn scan_quoted_ident(bytes: &[u8], mut i: usize, quote: u8) -> usize {
  while i < bytes.len() {
    if bytes[i] == b'\\' {
      i += 1;
      if i < bytes.len() {
        i += utf8_len(bytes[i]);
      }
    } else if bytes[i] == quote {
      if peek(bytes, i + 1) == Some(quote) {
        i += 2;
      } else {
        return i + 1;
      }
    } else {
      i += 1;
    }
  }
  i
}

/// Block comments nest per the SQL standard.
fn scan_block_comment(bytes: &[u8], mut i: usize) -> usize {
  let mut depth = 1u32;
  while i < bytes.len() {
    if bytes[i] == b'/' && peek(bytes, i + 1) == Some(b'*') {
      depth += 1;
      i += 2;
    } else if bytes[i] == b'*' && peek(bytes, i + 1) == Some(b'/') {
      depth -= 1;
      i += 2;
      if depth == 0 {
        break;
      }
    } else {
      i += 1;
    }
  }
  i
}

/// Returns the index just past the closing `$` of a dollar quote tag like
/// `$tag$` or `$$` when the bytes at `i` start one.
fn dollar_quote_end(bytes: &[u8], i: usize) -> Option<usize> {
  let mut j = i + 1;
  while j < bytes.len() && (bytes[j].is_ascii_alphanumeric() || bytes[j] == b'_') {
    j += 1;
  }
  if peek(bytes, j) == Some(b'$') {
    Some(j + 1)
  } else {
    None
  }
}

fn scan_dollar_string(text: &str, start: usize, tag_end: usize) -> usize {
  let tag = &text[start..tag_end];
  match text[tag_end..].find(tag) {
    Some(pos) => tag_end + pos + tag.len(),
    None => text.len(),
  }
}
