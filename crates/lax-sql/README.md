# lax-sql

Lax SQL formatter, usable as a Rust library or as a
[dprint](https://dprint.dev) plugin.

## Philosophy

This formatter is deliberately lax: it never interprets your SQL beyond
splitting it into statements and clauses, and it never rewrites a token.
Statements are split at top level clause keywords (`select`, `from`,
`where`, joins, ...) and each clause goes on its own line; the layout
within a clause follows the configured `clauseStyle` (see below). The
output is canonical: the same query formats the same way regardless of how
it was typed, so author line breaks are not consulted.

Because nothing is interpreted, the formatter is dialect agnostic by
construction: PostgreSQL dollar quoting and `E'...'` escape strings, MySQL
backticks, T-SQL bracket identifiers and `#temp` tables, and placeholder
styles (`?`, `$1`, `:name`, `@var`) are all opaque tokens that pass through
untouched. The corpus test runs the formatter over the sqlfluff dialect
fixtures, about 2150 files across 30 dialects, under every clause style.

Three dialect ambiguities are resolved in favor of being position
independent and standard, so that formatting stays idempotent: a backslash
inside a regular single quoted string is a literal character (use `''`
doubling or `E'...'` strings); BigQuery triple quoted strings are not
recognized because they collide with standard quote doubling; and `#`
starts a comment only when followed by whitespace, so a MySQL `#comment`
with no space is read as tokens while a T-SQL `#temp` reference is an
identifier no matter where it lands on a line.

- A token is never rewritten; strings, quoted identifiers, numbers, and
  comments pass through verbatim, except that multi line comment interiors
  are realigned with their statement.
- The same query always formats identically regardless of input layout.

## Clause style

`clauseStyle` controls how a clause body is laid out:

| Value               | Behavior                                              |
| ------------------- | ----------------------------------------------------- |
| `"fill"` (default)  | The clause body flows after the keyword and wraps at the line width, packing items until they no longer fit. Compact. |
| `"expanded"`        | The clause keyword sits alone and the body is indented below it, with one comma separated item per line. The classic SQL look. |

Commas inside parens, such as function arguments, fill within the group in
both styles; only top level commas drive the one per line layout in
`expanded`.

## Keyword casing

The one opt-in exception to "never touch tokens" is `keywordCase`:

| Value                  | Behavior                                   |
| ---------------------- | ------------------------------------------ |
| `"preserve"` (default) | Keywords are kept exactly as written.      |
| `"upper"`              | Known SQL keywords are uppercased.         |
| `"lower"`              | Known SQL keywords are lowercased.         |

Only words on a curated keyword list are transformed. Quoted identifiers and
function names are different token kinds and can never be affected; unquoted
identifiers that collide with a keyword are case insensitive in SQL engines,
so the transform is semantics preserving.

## Configuration

| Key           | Default      | Description                  |
| ------------- | ------------ | ---------------------------- |
| `lineWidth`   | `120`        | Target maximum line width.   |
| `indentWidth` | `2`          | Number of spaces per indent. |
| `useTabs`     | `false`      | Use tabs instead of spaces.  |
| `newLineKind` | `lf`         | Kind of newline to use.      |
| `keywordCase` | `"preserve"` | See above.                   |
| `clauseStyle` | `"fill"`     | See above.                   |

`// dprint-ignore` and `// dprint-ignore-file` comment directives are
supported and configurable via `ignoreNodeCommentText` and
`ignoreFileCommentText`.

## Development

```bash
cargo test
```

Spec tests live in `tests/specs`. Each file contains one or more cases with
the input followed by an `[expect]` block.
