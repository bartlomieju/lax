use dprint_core::formatting::PrintItems;
use dprint_core::formatting::Signal;

/// Pushes text that may contain newlines or tabs. Lines after the first are
/// printed verbatim without applying the current indentation level, and tabs
/// are sent as tab print items because the printer rejects raw tabs.
pub fn push_text(items: &mut PrintItems, text: &str) {
  if !text.contains('\n') {
    push_text_line(items, text);
    return;
  }
  let mut lines = text.split('\n');
  if let Some(first) = lines.next() {
    push_text_line(items, first.trim_end_matches('\r'));
  }
  items.push_signal(Signal::StartIgnoringIndent);
  for line in lines {
    items.push_signal(Signal::NewLine);
    push_text_line(items, line.trim_end_matches('\r'));
  }
  items.push_signal(Signal::FinishIgnoringIndent);
}

pub fn push_text_line(items: &mut PrintItems, line: &str) {
  let mut first = true;
  for part in line.split('\t') {
    if !first {
      items.push_signal(Signal::Tab);
    }
    first = false;
    if !part.is_empty() {
      items.push_string(part.to_string());
    }
  }
}

/// Prints a comment, realigning the interior of a multi line comment
/// relative to the comment's new position. Interior lines keep their
/// indentation relative to the line the comment started on, instead of
/// their absolute columns, so a comment stays stable when the statement
/// around it is reindented, for example by a host formatter indenting an
/// embedded block. `comment` must be a subslice of `source`.
pub fn push_comment(items: &mut PrintItems, source: &str, comment: &str) {
  if !comment.contains('\n') {
    push_text(items, comment);
    return;
  }
  let offset = comment.as_ptr() as usize - source.as_ptr() as usize;
  let line_start = source[..offset].rfind('\n').map(|i| i + 1).unwrap_or(0);
  let original_column = source[line_start..offset].chars().count();
  let mut lines: Vec<&str> = comment.split('\n').collect();
  while lines.len() > 1 && lines.last().is_some_and(|l| l.trim().is_empty()) {
    lines.pop();
  }
  let mut lines = lines.into_iter();
  if let Some(first) = lines.next() {
    push_text_line(items, first.trim_end());
  }
  for line in lines {
    items.push_signal(Signal::NewLine);
    let line = line.trim_end();
    let mut remaining = original_column;
    let line = line.trim_start_matches(|c: char| {
      if remaining > 0 && (c == ' ' || c == '\t') {
        remaining -= 1;
        true
      } else {
        false
      }
    });
    push_text_line(items, line);
  }
}

/// True when the comment contains the directive as a whole word, so that an
/// ignore file directive does not also match the plain ignore directive it
/// starts with.
pub fn contains_directive(text: &str, directive: &str) -> bool {
  text.match_indices(directive).any(|(index, _)| {
    !text[index + directive.len()..].starts_with(|c: char| c.is_ascii_alphanumeric() || c == '-' || c == '_')
  })
}

/// A language independent view of a token for the ignore file header check.
pub enum HeaderToken<'a> {
  Whitespace { newlines: u32 },
  Comment(&'a str),
  Other,
}

/// True when a comment in the first comment cluster of the file contains the
/// ignore file directive. The cluster starts on the first line and ends at
/// the first blank line or non comment construct.
pub fn has_ignore_file_comment<'a>(tokens: impl Iterator<Item = HeaderToken<'a>>, directive: &str) -> bool {
  for (index, token) in tokens.enumerate() {
    match token {
      HeaderToken::Whitespace { newlines } => {
        if newlines >= 2 || (index == 0 && newlines >= 1) {
          return false;
        }
      }
      HeaderToken::Comment(text) => {
        if contains_directive(text, directive) {
          return true;
        }
      }
      HeaderToken::Other => return false,
    }
  }
  false
}
