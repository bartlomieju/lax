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
  let mut tokens = Vec::new();
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
    } else if b == b'-' && peek(bytes, i + 1) == Some(b'-') {
      while i < bytes.len() && bytes[i] != b'\n' {
        i += 1;
      }
      TokenKind::LineComment
    } else if b == b'/' && peek(bytes, i + 1) == Some(b'*') {
      i = scan_block_comment(bytes, i + 2);
      TokenKind::BlockComment
    } else if b == b'\'' {
      i = scan_string(bytes, i + 1);
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

/// Scans a single quoted string. Both doubled quote (`''`, the standard) and
/// backslash (`\'`, MySQL) escapes are recognized so that strings from any
/// dialect keep their boundaries.
fn scan_string(bytes: &[u8], mut i: usize) -> usize {
  while i < bytes.len() {
    match bytes[i] {
      b'\\' => {
        i += 1;
        if i < bytes.len() {
          i += utf8_len(bytes[i]);
        }
      }
      b'\'' => {
        if peek(bytes, i + 1) == Some(b'\'') {
          i += 2;
        } else {
          return i + 1;
        }
      }
      _ => i += 1,
    }
  }
  i
}

fn scan_quoted_ident(bytes: &[u8], mut i: usize, quote: u8) -> usize {
  while i < bytes.len() {
    if bytes[i] == quote {
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
