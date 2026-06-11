# dprint-plugin-css

Lenient CSS, SCSS, and Less formatter for [dprint](https://dprint.dev).

## Philosophy

This formatter is deliberately lax. It is built on the generic, semantics-free
parsing model from CSS Syntax Level 3: the parser does not know what any
at-rule, property, or value means, and the printer only ever adjusts
whitespace, indentation, and line breaks.

Concretely, that means:

- Casing is never changed. `prefersDark: true` inside an unknown at-rule
  stays exactly as written.
- Values are never rewritten, reordered, or normalized.
- Unknown or future syntax formats fine, because nothing is special-cased.
  Tailwind directives, vendor hacks, and tomorrow's at-rules all pass through.
- Custom property values are preserved verbatim, including inner whitespace.

SCSS and Less are supported through the same generic model. `#{...}` and
`@{...}` interpolations are treated as opaque tokens, and `//` line comments
are recognized everywhere. The indented Sass syntax (`.sass` files) is not
supported.

## Status

Experimental.

Long values and at-rule preludes wrap at the configured `lineWidth`, but a
line break is only ever introduced where the author already had whitespace,
so constructs where a space is meaningful (Tailwind arbitrary values, unicode
ranges, unquoted urls) are never broken apart. Author newlines inside values
are preserved, so hand formatted multi line font stacks, `grid-template-areas`
blocks, and SCSS maps keep their shape. Selectors are never wrapped.

## Configuration

| Key           | Default | Description                       |
| ------------- | ------- | --------------------------------- |
| `lineWidth`   | `120`   | Target maximum line width.        |
| `indentWidth` | `2`     | Number of spaces per indent.      |
| `useTabs`     | `false` | Use tabs instead of spaces.       |
| `newLineKind` | `lf`    | Kind of newline to use.           |

## Development

```bash
cargo test
```

Spec tests live in `tests/specs`. Each file contains one or more cases with
the input followed by an `[expect]` block.
