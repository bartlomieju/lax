# lax-markup

Lax HTML, XML, SVG, and component (Vue, Svelte, Astro) formatter, usable as
a Rust library or as a [dprint](https://dprint.dev) plugin. Part of the lax
formatter family with [lax-css](https://github.com/bartlomieju/lax-css) and
[lax-sql](https://github.com/bartlomieju/lax-sql), built on
[lax-core](https://github.com/bartlomieju/lax-core).

## Philosophy

Markup whitespace renders, so a formatter that is careless with it changes
what the page looks like. lax-markup only restructures content where doing
so provably cannot change rendering:

- A gap between elements is renormalized only when the author already had a
  line break there (a newline renders as one space regardless of
  indentation), or when both sides of the gap are block level (whitespace
  there does not render at all).
- Content containing text or inline elements is preserved byte for byte.
- `pre`, `textarea`, `script`, and `style` contents are preserved byte for
  byte, including the position of the close tag.
- Attributes are kept verbatim, including quotes, order, and template
  expressions; author line breaks between attributes are preserved and long
  tags wrap at the configured width.
- Comment interiors realign with their element; everything else about them
  is untouched.

Template syntax needs no special support: Vue `{{ expressions }}`, Svelte
and Astro `{expressions}` and `{#blocks}`, and Jinja style `{% tags %}` live
in text and attribute values, which are verbatim by construction. Balanced
`{...}` regions are scanned atomically so comparisons like `{#if x < y}`
do not confuse tag detection. Vue and Svelte single file components are
plain markup to this formatter: `<template>`, `<script>`, and `<style>`
blocks format structurally, with script and style contents preserved.

Anything broken, truncated, or unknown passes through verbatim and stays
stable: unclosed tags are never closed, missing `>` is never added, and
unmatched close tags are kept where they were.

## Configuration

| Key           | Default | Description                  |
| ------------- | ------- | ---------------------------- |
| `lineWidth`   | `120`   | Target maximum line width.   |
| `indentWidth` | `2`     | Number of spaces per indent. |
| `useTabs`     | `false` | Use tabs instead of spaces.  |
| `newLineKind` | `lf`    | Kind of newline to use.      |

`<!-- dprint-ignore -->` and `<!-- dprint-ignore-file -->` comment
directives are supported and configurable via `ignoreNodeCommentText` and
`ignoreFileCommentText`.

## Development

```bash
cargo test
```

Spec tests live in `tests/specs`; the corpus test runs the formatter over
the prettier HTML and Vue fixtures asserting that formatting never errors,
is idempotent, and only ever changes whitespace.
