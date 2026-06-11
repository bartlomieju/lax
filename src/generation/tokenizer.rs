#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenKind {
  Whitespace {
    newlines: u32,
  },
  LineComment,
  BlockComment,
  Ident,
  /// An identifier immediately followed by an open paren. The token text
  /// includes the paren.
  Function,
  AtKeyword,
  Hash,
  Str,
  /// An unquoted url token, including the function name and both parens.
  Url,
  Number,
  /// SCSS `#{...}`, Less `@{...}`, or JS template `${...}` interpolation
  /// kept as one opaque token.
  Interpolation,
  Colon,
  Semicolon,
  Comma,
  OpenBrace,
  CloseBrace,
  OpenParen,
  CloseParen,
  OpenBracket,
  CloseBracket,
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
    } else if b == b'/' && peek(bytes, i + 1) == Some(b'*') {
      i = scan_block_comment(bytes, i + 2);
      TokenKind::BlockComment
    } else if b == b'/' && peek(bytes, i + 1) == Some(b'/') {
      i = scan_line_comment(bytes, i + 2);
      TokenKind::LineComment
    } else if b == b'"' || b == b'\'' {
      i = scan_string(bytes, i + 1, b);
      TokenKind::Str
    } else if b.is_ascii_digit()
      || ((b == b'.' || b == b'+') && peek_digit(bytes, i + 1))
      || (b == b'-' && is_number_start(bytes, i + 1))
    {
      i = scan_number(bytes, i + 1);
      TokenKind::Number
    } else if (b == b'#' || b == b'@' || b == b'$') && peek(bytes, i + 1) == Some(b'{') {
      i = scan_interpolation(bytes, i + 2);
      TokenKind::Interpolation
    } else if b == b'#' && peek_name_char(bytes, i + 1) {
      i = scan_name(bytes, i + 1);
      TokenKind::Hash
    } else if b == b'@' && peek_name_char(bytes, i + 1) {
      i = scan_name(bytes, i + 1);
      TokenKind::AtKeyword
    } else if is_ident_start(bytes, i) {
      i = scan_name(bytes, i);
      if peek(bytes, i) == Some(b'(') {
        if text[start..i].eq_ignore_ascii_case("url") && !url_args_are_quoted(bytes, i + 1) {
          i = scan_url(bytes, i + 1);
          TokenKind::Url
        } else {
          i += 1;
          TokenKind::Function
        }
      } else {
        TokenKind::Ident
      }
    } else {
      i += utf8_len(b);
      match b {
        b'{' => TokenKind::OpenBrace,
        b'}' => TokenKind::CloseBrace,
        b'(' => TokenKind::OpenParen,
        b')' => TokenKind::CloseParen,
        b'[' => TokenKind::OpenBracket,
        b']' => TokenKind::CloseBracket,
        b':' => TokenKind::Colon,
        b';' => TokenKind::Semicolon,
        b',' => TokenKind::Comma,
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

fn is_name_char(b: u8) -> bool {
  b.is_ascii_alphanumeric() || b == b'-' || b == b'_' || b >= 0x80
}

fn is_name_start(b: u8) -> bool {
  b.is_ascii_alphabetic() || b == b'_' || b >= 0x80
}

fn is_ident_start(bytes: &[u8], i: usize) -> bool {
  let b = bytes[i];
  if is_name_start(b) || b == b'\\' {
    return true;
  }
  if b == b'-' {
    return peek_name_char(bytes, i + 1) || peek(bytes, i + 1) == Some(b'\\');
  }
  false
}

fn is_number_start(bytes: &[u8], i: usize) -> bool {
  peek_digit(bytes, i) || (peek(bytes, i) == Some(b'.') && peek_digit(bytes, i + 1))
}

fn peek(bytes: &[u8], i: usize) -> Option<u8> {
  bytes.get(i).copied()
}

fn peek_digit(bytes: &[u8], i: usize) -> bool {
  bytes.get(i).is_some_and(|b| b.is_ascii_digit())
}

fn peek_name_char(bytes: &[u8], i: usize) -> bool {
  bytes.get(i).is_some_and(|b| is_name_char(*b))
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

fn scan_name(bytes: &[u8], mut i: usize) -> usize {
  while i < bytes.len() {
    let b = bytes[i];
    if is_name_char(b) {
      i += 1;
    } else if b == b'\\' && i + 1 < bytes.len() {
      i += 1 + utf8_len(bytes[i + 1]);
    } else {
      break;
    }
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

fn scan_string(bytes: &[u8], mut i: usize, quote: u8) -> usize {
  while i < bytes.len() {
    let b = bytes[i];
    if b == b'\\' {
      i += 1;
      if i < bytes.len() {
        i += utf8_len(bytes[i]);
      }
    } else if b == quote {
      i += 1;
      break;
    } else if b == b'\n' {
      // unterminated string, end the token at the newline
      break;
    } else {
      i += 1;
    }
  }
  i
}

fn scan_block_comment(bytes: &[u8], mut i: usize) -> usize {
  while i < bytes.len() {
    if bytes[i] == b'*' && peek(bytes, i + 1) == Some(b'/') {
      return i + 2;
    }
    i += 1;
  }
  i
}

fn scan_line_comment(bytes: &[u8], mut i: usize) -> usize {
  while i < bytes.len() && bytes[i] != b'\n' {
    i += 1;
  }
  i
}

fn scan_interpolation(bytes: &[u8], mut i: usize) -> usize {
  let mut depth = 1u32;
  while i < bytes.len() {
    match bytes[i] {
      b'{' => {
        depth += 1;
        i += 1;
      }
      b'}' => {
        depth -= 1;
        i += 1;
        if depth == 0 {
          break;
        }
      }
      b'"' | b'\'' => {
        let quote = bytes[i];
        i = scan_string(bytes, i + 1, quote);
      }
      _ => i += 1,
    }
  }
  i
}

fn url_args_are_quoted(bytes: &[u8], mut i: usize) -> bool {
  while i < bytes.len() && matches!(bytes[i], b' ' | b'\t') {
    i += 1;
  }
  matches!(peek(bytes, i), Some(b'"') | Some(b'\''))
}

fn scan_url(bytes: &[u8], mut i: usize) -> usize {
  while i < bytes.len() {
    match bytes[i] {
      b')' => return i + 1,
      b'\\' => {
        i += 1;
        if i < bytes.len() {
          i += utf8_len(bytes[i]);
        }
      }
      _ => i += 1,
    }
  }
  i
}
