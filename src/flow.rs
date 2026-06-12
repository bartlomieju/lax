use dprint_core::formatting::PrintItems;
use dprint_core::formatting::Signal;

#[derive(PartialEq, Clone, Copy)]
enum Pending {
  None,
  Space,
  Newline,
}

/// How a token participates in the whitespace flow of a value, clause, or
/// attribute list. The printer never needs to know what a token means, only
/// which of these classes it falls into.
#[derive(PartialEq, Clone, Copy)]
pub enum FlowClass {
  Whitespace {
    newlines: u32,
  },
  /// An opening paren, bracket, or similar group opener.
  Open,
  /// The matching group closer.
  Close,
  /// A separator that never takes a space before it.
  Comma,
  /// A comment that runs to the end of the line. Nothing may share a line
  /// with it, or it would be absorbed into the comment on reparse.
  LineComment,
  Other,
}

/// Prints a token sequence following the lax whitespace policy:
///
/// - a single author space becomes a possible line break point when the
///   line exceeds the configured width, since changing a space to a newline
///   is a whitespace only change
/// - an author newline is preserved as a newline, so hand formatted code
///   keeps its shape
/// - a line break is never introduced where the author had no whitespace
///
/// Continuation lines are indented one level. A group the author opened
/// with a newline indents its contents one level per nesting depth and puts
/// the closer back at the start level.
///
/// The caller classifies each token and provides a closure that emits it;
/// the flow printer decides what whitespace to print around it.
pub struct FlowPrinter {
  extra_indent: usize,
  pending: Pending,
  after_open: bool,
  first_emitted: bool,
  // one entry per open group; true when the group is multi line
  groups: Vec<bool>,
}

impl FlowPrinter {
  pub fn new(items: &mut PrintItems, starts_on_new_line: bool) -> Self {
    // the continuation indent starts after the first item is written, so
    // that a token sequence that starts a line is not itself indented
    let mut printer = FlowPrinter {
      extra_indent: 0,
      pending: Pending::None,
      after_open: false,
      first_emitted: starts_on_new_line,
      groups: Vec::new(),
    };
    if starts_on_new_line {
      items.push_signal(Signal::StartIndent);
      items.push_signal(Signal::NewLine);
      printer.extra_indent = 1;
    }
    printer
  }

  fn marked(&self) -> usize {
    self.groups.iter().filter(|m| **m).count()
  }

  fn set_extra_indent(&mut self, items: &mut PrintItems, desired: usize) {
    while self.extra_indent < desired {
      items.push_signal(Signal::StartIndent);
      self.extra_indent += 1;
    }
    while self.extra_indent > desired {
      items.push_signal(Signal::FinishIndent);
      self.extra_indent -= 1;
    }
  }

  fn flush_pending(&mut self, items: &mut PrintItems) {
    match self.pending {
      Pending::Space => items.push_signal(Signal::SpaceOrNewLine),
      Pending::Newline => {
        let marked = self.marked();
        self.set_extra_indent(items, marked.max(1));
        items.push_signal(Signal::NewLine);
      }
      Pending::None => {}
    }
    self.pending = Pending::None;
  }

  pub fn token(&mut self, items: &mut PrintItems, class: FlowClass, emit: impl FnOnce(&mut PrintItems)) {
    // when the first token is preceded by whitespace, the continuation
    // indent must be in place before that whitespace is flushed, or a width
    // induced break before the first token lands one level shallower than
    // the same break does on the next pass, when it is an author newline
    if !self.first_emitted && self.pending != Pending::None && !matches!(class, FlowClass::Whitespace { .. }) {
      items.push_signal(Signal::StartIndent);
      self.extra_indent += 1;
      self.first_emitted = true;
    }
    let mut emitted = false;
    match class {
      FlowClass::Whitespace { newlines } => {
        if self.after_open {
          if newlines > 0
            && let Some(top) = self.groups.last_mut()
          {
            *top = true;
            let marked = self.marked();
            self.set_extra_indent(items, marked.max(1));
            items.push_signal(Signal::NewLine);
          }
          // a space directly after a group opener is dropped
          self.after_open = false;
        } else if newlines > 0 {
          self.pending = Pending::Newline;
        } else if self.pending == Pending::None {
          self.pending = Pending::Space;
        }
      }
      FlowClass::Comma => {
        self.pending = Pending::None;
        emit(items);
        self.after_open = false;
        emitted = true;
      }
      FlowClass::Close => {
        let was_multi_line = self.groups.pop().unwrap_or(false);
        if was_multi_line {
          let marked = self.marked();
          self.set_extra_indent(items, marked);
          items.push_signal(Signal::NewLine);
          self.pending = Pending::None;
          emit(items);
          // the dedent applies to the closer's line only; restore the
          // continuation baseline so later breaks land at a stable level
          let marked = self.marked();
          self.set_extra_indent(items, marked.max(1));
        } else {
          // a space before a closer is dropped, but an author newline is
          // kept; it may also be load bearing when a line comment precedes
          // the closer
          if self.pending == Pending::Newline {
            let marked = self.marked();
            self.set_extra_indent(items, marked.max(1));
            items.push_signal(Signal::NewLine);
          }
          self.pending = Pending::None;
          emit(items);
        }
        self.after_open = false;
        emitted = true;
      }
      FlowClass::Open | FlowClass::LineComment | FlowClass::Other => {
        self.flush_pending(items);
        emit(items);
        if class == FlowClass::LineComment {
          self.pending = Pending::Newline;
        }
        if class == FlowClass::Open {
          self.groups.push(false);
          self.after_open = true;
        } else {
          self.after_open = false;
        }
        emitted = true;
      }
    }
    if emitted && !self.first_emitted {
      items.push_signal(Signal::StartIndent);
      self.extra_indent += 1;
      self.first_emitted = true;
    }
  }

  pub fn finish(mut self, items: &mut PrintItems) {
    self.set_extra_indent(items, 0);
  }
}
