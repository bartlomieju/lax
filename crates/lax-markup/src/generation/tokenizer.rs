/// One attribute inside a tag, kept as a verbatim slice including its value
/// and quotes. `name={expr}` style values scan balanced braces so template
/// expressions stay opaque.
#[derive(Debug, Clone, Copy)]
pub struct Attr<'a> {
  pub text: &'a str,
  /// Newlines in the whitespace before this attribute.
  pub newlines_before: u32,
}

#[derive(Debug)]
pub enum EventKind<'a> {
  /// A text run containing at least one non whitespace character. Balanced
  /// `{...}` regions are scanned atomically so `<` inside template
  /// expressions does not start a tag.
  Text,
  /// A whitespace only text run.
  Whitespace {
    newlines: u32,
  },
  /// `<!-- ... -->`
  Comment {
    text: &'a str,
  },
  /// `<!DOCTYPE ...>`, `<!...>`, or `<?...?>`
  Doctype,
  /// `<![CDATA[ ... ]]>`
  CData,
  OpenTag {
    name: &'a str,
    attrs: Vec<Attr<'a>>,
    self_closing: bool,
    /// False when the file ends inside the tag, in which case no `>` is
    /// manufactured.
    complete: bool,
    /// Newlines the author left before the closing `>` or `/>`.
    newlines_before_close: u32,
  },
  CloseTag {
    name: &'a str,
  },
  /// The verbatim contents of a raw text element (script, style, pre,
  /// textarea), up to but not including its close tag.
  RawText,
}

#[derive(Debug)]
pub struct Event<'a> {
  pub kind: EventKind<'a>,
  pub span: (usize, usize),
}

/// Elements whose contents are never parsed for tags. `pre` keeps its
/// contents byte for byte anyway, so any markup inside it stays verbatim.
pub fn is_raw_element(name: &str) -> bool {
  ["script", "style", "pre", "textarea"]
    .iter()
    .any(|r| r.eq_ignore_ascii_case(name))
}

pub fn tokenize(text: &str) -> Vec<Event<'_>> {
  let bytes = text.as_bytes();
  let mut events = Vec::new();
  let mut i = 0;
  while i < bytes.len() {
    let start = i;
    if bytes[i] == b'<' {
      if text[i..].starts_with("<!--") {
        let end = text[i + 4..].find("-->").map(|p| i + 4 + p + 3).unwrap_or(text.len());
        events.push(Event {
          kind: EventKind::Comment {
            text: &text[start..end],
          },
          span: (start, end),
        });
        i = end;
        continue;
      }
      if text[i..].starts_with("<![CDATA[") {
        let end = text[i + 9..].find("]]>").map(|p| i + 9 + p + 3).unwrap_or(text.len());
        events.push(Event {
          kind: EventKind::CData,
          span: (start, end),
        });
        i = end;
        continue;
      }
      if let Some(next) = bytes.get(i + 1).copied() {
        if next == b'!' || next == b'?' {
          let end = scan_to_tag_end(bytes, i + 1);
          events.push(Event {
            kind: EventKind::Doctype,
            span: (start, end),
          });
          i = end;
          continue;
        }
        if next == b'/' {
          let name_start = i + 2;
          let name_end = scan_name(bytes, name_start);
          let end = scan_to_tag_end(bytes, name_end);
          events.push(Event {
            kind: EventKind::CloseTag {
              name: &text[name_start..name_end],
            },
            span: (start, end),
          });
          i = end;
          continue;
        }
        if next.is_ascii_alphabetic() {
          let (event, end) = scan_open_tag(text, i);
          let raw = matches!(&event.kind, EventKind::OpenTag { name, self_closing, .. }
            if !self_closing && is_raw_element(name));
          let raw_name = match &event.kind {
            EventKind::OpenTag { name, .. } => *name,
            _ => unreachable!(),
          };
          events.push(event);
          i = end;
          if raw {
            let close = format!("</{}", raw_name.to_ascii_lowercase());
            let content_end = text[i..]
              .to_ascii_lowercase()
              .find(&close)
              .map(|p| i + p)
              .unwrap_or(text.len());
            if content_end > i {
              events.push(Event {
                kind: EventKind::RawText,
                span: (i, content_end),
              });
            }
            i = content_end;
          }
          continue;
        }
      }
      // a lone `<` that does not start anything is text
    }
    // text run up to the next tag start
    let end = scan_text(text, i);
    let run = &text[start..end.max(start + 1)];
    let end = end.max(start + 1);
    if run.chars().all(char::is_whitespace) {
      events.push(Event {
        kind: EventKind::Whitespace {
          newlines: run.matches('\n').count() as u32,
        },
        span: (start, end),
      });
    } else {
      events.push(Event {
        kind: EventKind::Text,
        span: (start, end),
      });
    }
    i = end;
  }
  events
}

fn scan_name(bytes: &[u8], mut i: usize) -> usize {
  while i < bytes.len() && (bytes[i].is_ascii_alphanumeric() || matches!(bytes[i], b'-' | b'_' | b':' | b'.')) {
    i += 1;
  }
  i
}

/// Scans past `>` at the top level of a tag, skipping quoted regions.
fn scan_to_tag_end(bytes: &[u8], mut i: usize) -> usize {
  while i < bytes.len() {
    match bytes[i] {
      b'>' => return i + 1,
      b'"' | b'\'' => {
        let quote = bytes[i];
        i += 1;
        while i < bytes.len() && bytes[i] != quote {
          i += 1;
        }
        if i < bytes.len() {
          i += 1;
        }
      }
      _ => i += 1,
    }
  }
  i
}

/// Scans a text run, ending before the next `<` that starts a tag, comment,
/// or declaration. Balanced `{...}` regions are skipped atomically.
fn scan_text(text: &str, mut i: usize) -> usize {
  let bytes = text.as_bytes();
  while i < bytes.len() {
    match bytes[i] {
      b'<' => {
        if let Some(next) = bytes.get(i + 1).copied()
          && (next.is_ascii_alphabetic() || matches!(next, b'/' | b'!' | b'?'))
        {
          return i;
        }
        i += 1;
      }
      b'{' => {
        i = scan_balanced_braces(bytes, i);
      }
      _ => i += 1,
    }
  }
  i
}

/// Skips a balanced `{...}` region, taking quotes into account. When the
/// braces never balance the `{` is treated as a plain character.
fn scan_balanced_braces(bytes: &[u8], start: usize) -> usize {
  let mut i = start + 1;
  let mut depth = 1u32;
  while i < bytes.len() {
    match bytes[i] {
      b'{' => depth += 1,
      b'}' => {
        depth -= 1;
        if depth == 0 {
          return i + 1;
        }
      }
      b'"' | b'\'' | b'`' => {
        let quote = bytes[i];
        i += 1;
        while i < bytes.len() && bytes[i] != quote {
          if bytes[i] == b'\\' {
            i += 1;
          }
          i += 1;
        }
      }
      _ => {}
    }
    i += 1;
  }
  // unbalanced; treat the brace as a plain character
  start + 1
}

fn scan_open_tag(text: &str, start: usize) -> (Event<'_>, usize) {
  let bytes = text.as_bytes();
  let name_start = start + 1;
  let name_end = scan_name(bytes, name_start);
  let name = &text[name_start..name_end];
  let mut attrs = Vec::new();
  let mut i = name_end;
  let mut self_closing = false;
  let mut complete = false;
  let mut newlines_before_close = 0;
  loop {
    // whitespace before the next attribute or tag end
    let ws_start = i;
    while i < bytes.len() && bytes[i].is_ascii_whitespace() {
      i += 1;
    }
    let newlines_before = text[ws_start..i].matches('\n').count() as u32;
    if i >= bytes.len() {
      break;
    }
    match bytes[i] {
      b'>' => {
        i += 1;
        complete = true;
        newlines_before_close = newlines_before;
        break;
      }
      b'/' if bytes.get(i + 1) == Some(&b'>') => {
        self_closing = true;
        i += 2;
        complete = true;
        newlines_before_close = newlines_before;
        break;
      }
      _ => {
        let attr_start = i;
        // attribute name; template expressions can appear as the whole
        // name or inside it, like data-{{key}}, and a slash is part of
        // the name unless it ends the tag, so that comments inside a tag
        // cannot stall the scanner
        if bytes[i] == b'{' {
          i = scan_balanced_braces(bytes, i);
        } else {
          while i < bytes.len() && !bytes[i].is_ascii_whitespace() && !matches!(bytes[i], b'=' | b'>') {
            if bytes[i] == b'/' && bytes.get(i + 1) == Some(&b'>') {
              break;
            }
            if bytes[i] == b'{' {
              i = scan_balanced_braces(bytes, i);
            } else {
              i += 1;
            }
          }
        }
        // optional value
        let mut j = i;
        while j < bytes.len() && bytes[j].is_ascii_whitespace() {
          j += 1;
        }
        if bytes.get(j) == Some(&b'=') {
          j += 1;
          while j < bytes.len() && bytes[j].is_ascii_whitespace() {
            j += 1;
          }
          match bytes.get(j) {
            Some(b'"') | Some(b'\'') => {
              let quote = bytes[j];
              j += 1;
              while j < bytes.len() && bytes[j] != quote {
                j += 1;
              }
              i = (j + 1).min(bytes.len());
            }
            Some(b'{') => {
              i = scan_balanced_braces(bytes, j);
            }
            _ => {
              while j < bytes.len() && !bytes[j].is_ascii_whitespace() && bytes[j] != b'>' {
                j += 1;
              }
              i = j;
            }
          }
        }
        if i == attr_start {
          // never loop without progress, no matter how broken the tag is
          i += 1;
          continue;
        }
        attrs.push(Attr {
          text: &text[attr_start..i],
          newlines_before,
        });
      }
    }
  }
  (
    Event {
      kind: EventKind::OpenTag {
        name,
        attrs,
        self_closing,
        complete,
        newlines_before_close,
      },
      span: (start, i),
    },
    i,
  )
}
