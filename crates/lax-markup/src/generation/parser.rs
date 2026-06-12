use super::tokenizer::Attr;
use super::tokenizer::Event;
use super::tokenizer::EventKind;

pub const VOID_ELEMENTS: &[&str] = &[
  "area", "base", "br", "col", "embed", "hr", "img", "input", "link", "meta", "source", "track", "wbr",
];

#[derive(Debug)]
pub enum Node<'a> {
  Element {
    name: &'a str,
    attrs: Vec<Attr<'a>>,
    self_closing: bool,
    /// False when the source ends inside the open tag.
    complete: bool,
    children: Vec<Node<'a>>,
    /// False when the close tag was missing from the source; nothing is
    /// manufactured so truncated input stays stable.
    closed: bool,
    span: (usize, usize),
  },
  Text {
    span: (usize, usize),
  },
  Whitespace {
    newlines: u32,
    span: (usize, usize),
  },
  RawText {
    span: (usize, usize),
  },
  Comment {
    text: &'a str,
    span: (usize, usize),
  },
  Verbatim {
    span: (usize, usize),
  },
}

impl<'a> Node<'a> {
  pub fn span(&self) -> (usize, usize) {
    match self {
      Node::Element { span, .. }
      | Node::Text { span }
      | Node::RawText { span }
      | Node::Comment { span, .. }
      | Node::Verbatim { span }
      | Node::Whitespace { span, .. } => *span,
    }
  }
}

fn is_void(name: &str) -> bool {
  VOID_ELEMENTS.iter().any(|v| v.eq_ignore_ascii_case(name))
}

pub fn parse<'a>(events: Vec<Event<'a>>) -> Vec<Node<'a>> {
  let mut parser = Parser {
    events: events.into_iter().peekable(),
  };
  parser.parse_children(None).0
}

struct Parser<'a> {
  events: std::iter::Peekable<std::vec::IntoIter<Event<'a>>>,
}

impl<'a> Parser<'a> {
  /// Parses siblings until a close tag for `parent` or the end of input.
  /// Returns the children and the end offset of the consumed close tag when
  /// one was found.
  fn parse_children(&mut self, parent: Option<&str>) -> (Vec<Node<'a>>, Option<usize>) {
    let mut children = Vec::new();
    while let Some(event) = self.events.peek() {
      if let EventKind::CloseTag { name } = &event.kind {
        if let Some(parent) = parent
          && name.eq_ignore_ascii_case(parent)
        {
          let end = event.span.1;
          self.events.next();
          return (children, Some(end));
        }
        // an unmatched close tag passes through verbatim
        let span = event.span;
        self.events.next();
        children.push(Node::Verbatim { span });
        continue;
      }
      let event = self.events.next().unwrap();
      match event.kind {
        EventKind::Text => children.push(Node::Text { span: event.span }),
        EventKind::Whitespace { newlines } => children.push(Node::Whitespace {
          newlines,
          span: event.span,
        }),
        EventKind::Comment { text } => children.push(Node::Comment { text, span: event.span }),
        EventKind::Doctype | EventKind::CData => children.push(Node::Verbatim { span: event.span }),
        EventKind::RawText => children.push(Node::RawText { span: event.span }),
        EventKind::CloseTag { .. } => unreachable!(),
        EventKind::OpenTag {
          name,
          attrs,
          self_closing,
          complete,
        } => {
          if self_closing || is_void(name) || !complete {
            children.push(Node::Element {
              name,
              attrs,
              self_closing,
              complete,
              children: Vec::new(),
              closed: true,
              span: event.span,
            });
          } else {
            let (inner, close_end) = self.parse_children(Some(name));
            let end = close_end
              .or_else(|| inner.last().map(|n| n.span().1))
              .unwrap_or(event.span.1);
            children.push(Node::Element {
              name,
              attrs,
              self_closing: false,
              complete: true,
              children: inner,
              closed: close_end.is_some(),
              span: (event.span.0, end),
            });
          }
        }
      }
    }
    (children, None)
  }
}
