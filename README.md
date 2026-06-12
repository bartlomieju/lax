# lax-core

Shared printing machinery for the lax formatter family:
[lax-css](https://github.com/bartlomieju/lax-css),
[lax-sql](https://github.com/bartlomieju/lax-sql), and lax-markup.

The lax formatters never interpret the code being formatted, only adjust
whitespace. This crate holds the parts that are identical across languages:

- `FlowPrinter`: turns a classified token stream into dprint print items
  with author newline preservation, width aware wrapping at author spaces,
  and per depth indentation of multi line groups.
- text and comment emission, including relative realignment of multi line
  comment interiors and tab safe output.
- ignore directive matching and the first comment cluster ignore file rule.
