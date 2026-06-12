use dprint_core::formatting::PrintItems;
use dprint_core::formatting::Signal;
use lax_core::FlowClass;
use lax_core::FlowPrinter;
use lax_core::contains_directive;
use lax_core::push_comment;
use lax_core::push_text;

use std::cell::RefCell;

use super::parser::Node;
use super::tokenizer::is_raw_element;
use crate::configuration::Configuration;
use crate::format_text::ExternalFormatter;

/// Elements that flow with surrounding text. Whitespace around them renders,
/// so content containing them is never restructured. Everything not on this
/// list, including unknown elements and components, is treated as block.
const INLINE_ELEMENTS: &[&str] = &[
  "a", "abbr", "b", "bdi", "bdo", "br", "button", "cite", "code", "data", "dfn", "em", "i", "img", "input", "kbd",
  "label", "mark", "meter", "noscript", "object", "output", "progress", "q", "ruby", "s", "samp", "select", "slot",
  "small", "span", "strong", "sub", "sup", "time", "u", "var", "wbr",
];

fn is_inline(name: &str) -> bool {
  INLINE_ELEMENTS.iter().any(|e| e.eq_ignore_ascii_case(name))
}

struct Context<'a> {
  source: &'a str,
  ignore_directive: &'a str,
  line_width: u32,
  external: Option<&'a ExternalFormatter<'a>>,
  external_error: &'a RefCell<Option<anyhow::Error>>,
}

pub fn generate(
  nodes: &[Node],
  source: &str,
  config: &Configuration,
  external: Option<&ExternalFormatter>,
  external_error: &RefCell<Option<anyhow::Error>>,
) -> PrintItems {
  let mut items = PrintItems::new();
  let ctx = Context {
    source,
    ignore_directive: &config.ignore_node_comment_text,
    line_width: config.line_width,
    external,
    external_error,
  };
  if can_restructure(nodes, false, true) {
    gen_structural_children(nodes, &mut items, &ctx);
    items.push_signal(Signal::NewLine);
  } else {
    // a document with top level text flows as written
    push_text(&mut items, source.trim_end());
    items.push_signal(Signal::NewLine);
  }
  items
}

fn is_block_node(node: &Node) -> bool {
  match node {
    Node::Element { name, .. } => !is_inline(name),
    Node::Comment { .. } | Node::Verbatim { .. } => true,
    _ => false,
  }
}

/// True when the children can be put one per line without changing what the
/// markup renders as.
///
/// Restructuring writes a newline into every gap: after the open tag,
/// between children, and before the close tag. Whitespace that contains a
/// newline renders as a single space no matter how it is indented, so a gap
/// where the author already had a line break is always safe to renormalize.
/// A gap with no line break is only safe when both of its sides are block
/// level, where whitespace does not render at all. Text pins everything.
///
/// At the document root there are no enclosing tags, so nothing is ever
/// written into the edge gaps and they are always safe.
fn can_restructure(children: &[Node], parent_inline: bool, root: bool) -> bool {
  if children
    .iter()
    .any(|c| matches!(c, Node::Text { .. } | Node::RawText { .. }))
  {
    return false;
  }
  let mut prev_side_block = !parent_inline;
  let mut gap_has_newline = root;
  for child in children {
    if let Node::Whitespace { newlines, .. } = child {
      if *newlines > 0 {
        gap_has_newline = true;
      }
      continue;
    }
    let gap_safe = gap_has_newline || (prev_side_block && is_block_node(child));
    if !gap_safe {
      return false;
    }
    gap_has_newline = false;
    prev_side_block = is_block_node(child);
  }
  root || gap_has_newline || (prev_side_block && !parent_inline)
}

fn gen_structural_children(nodes: &[Node], items: &mut PrintItems, ctx: &Context) {
  let mut first = true;
  let mut pending_blank = false;
  let mut ignore_next = false;
  for node in nodes {
    if let Node::Whitespace { newlines, .. } = node {
      if *newlines >= 2 {
        pending_blank = true;
      }
      continue;
    }
    if !first {
      items.push_signal(Signal::NewLine);
    }
    if pending_blank && !first {
      items.push_signal(Signal::NewLine);
    }
    pending_blank = false;
    first = false;
    let is_comment = matches!(node, Node::Comment { .. });
    if ignore_next && !is_comment {
      let (start, end) = node.span();
      push_text(items, ctx.source[start..end].trim_end());
      ignore_next = false;
      continue;
    }
    if let Node::Comment { text, .. } = node
      && contains_directive(text, ctx.ignore_directive)
    {
      ignore_next = true;
    }
    gen_node(node, items, ctx);
  }
}

fn gen_node(node: &Node, items: &mut PrintItems, ctx: &Context) {
  match node {
    Node::Comment { text, .. } => push_comment(items, ctx.source, text),
    Node::Verbatim { span } | Node::Text { span } | Node::RawText { span } => {
      push_text(items, &ctx.source[span.0..span.1]);
    }
    Node::Whitespace { .. } => {}
    Node::Element {
      name,
      attrs,
      self_closing,
      complete,
      children,
      closed,
      span,
    } => {
      gen_open_tag(name, attrs, *self_closing, *complete, items, ctx);
      if !*complete
        || *self_closing
        || super::parser::VOID_ELEMENTS
          .iter()
          .any(|v| v.eq_ignore_ascii_case(name))
      {
        return;
      }
      let parent_inline = is_inline(name);
      if is_raw_element(name) {
        if let Some(formatted) = format_embedded(name, attrs, children, ctx) {
          if formatted.trim().is_empty() {
            if *closed {
              items.push_string(format!("</{}>", name));
            }
            return;
          }
          // script and style contents sit at the same level as their tag,
          // matching the markup_fmt and dprint default that the Vue and
          // Svelte ecosystems expect
          for line in formatted.trim_end().split('\n') {
            items.push_signal(Signal::NewLine);
            lax_core::push_text_line(items, line.trim_end_matches('\r'));
          }
          if *closed {
            items.push_signal(Signal::NewLine);
            items.push_string(format!("</{}>", name));
          }
          return;
        }
        // raw contents are preserved byte for byte, together with the close
        // tag, so that no whitespace is ever inserted before it; whitespace
        // before `</pre>` would render
        if let Some(first) = children.first() {
          let start = first.span().0;
          let end = if *closed {
            span.1
          } else {
            children.last().unwrap().span().1
          };
          push_text(items, &ctx.source[start..end]);
        } else if *closed {
          items.push_string(format!("</{}>", name));
        }
        return;
      }
      if children.iter().all(|c| matches!(c, Node::Whitespace { .. })) {
        if parent_inline && !children.is_empty() {
          // whitespace inside an inline element renders
          let start = children.first().unwrap().span().0;
          let end = children.last().unwrap().span().1;
          push_text(items, &ctx.source[start..end]);
        }
        // otherwise nothing but whitespace collapses
      } else if can_restructure(children, parent_inline, false) {
        items.push_signal(Signal::StartIndent);
        items.push_signal(Signal::NewLine);
        gen_structural_children(children, items, ctx);
        items.push_signal(Signal::FinishIndent);
        // the newline puts the close tag on its own line; with no close
        // tag in the source there is nothing to put there
        if *closed {
          items.push_signal(Signal::NewLine);
        }
      } else {
        // mixed content is whitespace sensitive and stays as written
        let start = children.first().map(|c| c.span().0).unwrap();
        let end = children.last().map(|c| c.span().1).unwrap();
        push_text(items, &ctx.source[start..end]);
      }
      if *closed {
        items.push_string(format!("</{}>", name));
      }
    }
  }
}

fn gen_open_tag(
  name: &str,
  attrs: &[super::tokenizer::Attr],
  self_closing: bool,
  complete: bool,
  items: &mut PrintItems,
  ctx: &Context,
) {
  items.push_string(format!("<{}", name));
  if !attrs.is_empty() {
    let mut flow = FlowPrinter::new(items, false);
    for attr in attrs {
      flow.token(
        items,
        FlowClass::Whitespace {
          newlines: attr.newlines_before,
        },
        |_| {},
      );
      let text = attr.text;
      flow.token(items, FlowClass::Other, |items| push_text(items, text));
    }
    flow.finish(items);
  }
  let _ = ctx;
  if !complete {
    // the file ended inside this tag; nothing is manufactured
    return;
  }
  if self_closing {
    items.push_string(" />".to_string());
  } else {
    items.push_string(">".to_string());
  }
}

/// Runs the external formatter over a script or style body. Returns None
/// when there is no external formatter, the element is not embeddable, the
/// formatter declined, or it failed, in which case the error is recorded
/// and the contents stay verbatim for this pass.
fn format_embedded(name: &str, attrs: &[super::tokenizer::Attr], children: &[Node], ctx: &Context) -> Option<String> {
  let external = ctx.external?;
  let kind = if name.eq_ignore_ascii_case("style") {
    "css"
  } else if name.eq_ignore_ascii_case("script") {
    "js"
  } else {
    return None;
  };
  let lang = attr_value(attrs, "lang")
    .or_else(|| attr_value(attrs, "type"))
    .unwrap_or(kind);
  let (start, end) = match (children.first(), children.last()) {
    (Some(first), Some(last)) => (first.span().0, last.span().1),
    _ => return None,
  };
  let content = &ctx.source[start..end];
  match external(lang, content, ctx.line_width) {
    Ok(result) => result,
    Err(error) => {
      ctx.external_error.borrow_mut().get_or_insert(error);
      None
    }
  }
}

fn attr_value<'a>(attrs: &[super::tokenizer::Attr<'a>], name: &str) -> Option<&'a str> {
  for attr in attrs {
    let text = attr.text;
    let Some(eq) = text.find('=') else { continue };
    if !text[..eq].trim().eq_ignore_ascii_case(name) {
      continue;
    }
    let value = text[eq + 1..].trim();
    let value = value
      .strip_prefix('"')
      .and_then(|v| v.strip_suffix('"'))
      .or_else(|| value.strip_prefix('\'').and_then(|v| v.strip_suffix('\'')))
      .unwrap_or(value);
    return Some(value);
  }
  None
}
