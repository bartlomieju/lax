//! Shared printing machinery for the lax formatter family.
//!
//! The lax formatters (lax-css, lax-sql, lax-markup) share one philosophy:
//! never interpret the code being formatted, only adjust whitespace. This
//! crate holds the pieces that are identical across languages: the flow
//! printer that turns a token stream into print items with author newline
//! preservation and width aware wrapping, text and comment emission, and
//! ignore directive handling.

mod flow;
mod text;

pub use flow::FlowClass;
pub use flow::FlowPrinter;
pub use text::HeaderToken;
pub use text::contains_directive;
pub use text::has_ignore_file_comment;
pub use text::push_comment;
pub use text::push_text;
pub use text::push_text_line;
