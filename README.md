# lax-sql

Lax SQL formatter, usable as a Rust library or as a
[dprint](https://dprint.dev) plugin.

## Philosophy

This formatter is deliberately lax, following the same design as
[lax-css](https://github.com/bartlomieju/dprint-plugin-css): it never
interprets your SQL, only adjusts whitespace. Statements are split at top
level clause keywords (`select`, `from`, `where`, joins, ...), each clause
goes on its own line, and everything else is preserved as written.

Because nothing is interpreted, the formatter is dialect agnostic by
construction: PostgreSQL dollar quoting and `E'...'` escape strings, MySQL
backticks and `#` comments, T-SQL bracket identifiers and `#temp` tables,
and placeholder styles (`?`, `$1`, `:name`, `@var`) are all opaque tokens
that pass through untouched. The corpus test runs the formatter over the
sqlfluff dialect fixtures, about 2150 files across 30 dialects.

Two dialect ambiguities are resolved in favor of the standard: a backslash
inside a regular single quoted string is a literal character (use `''`
doubling or `E'...'` strings, which both also work in MySQL), and BigQuery
triple quoted strings are not recognized because they are ambiguous with
standard quote doubling.

- A line break is never introduced where the author had no whitespace.
- Author newlines are preserved, so hand formatted statements keep their
  shape. Multi line paren groups indent one level per nesting depth.
- Long clauses wrap at the configured line width, breaking only at existing
  author spaces.
- Strings, quoted identifiers, and comments are never modified, except that
  multi line comment interiors are realigned with their statement.

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

`// dprint-ignore` and `// dprint-ignore-file` comment directives are
supported and configurable via `ignoreNodeCommentText` and
`ignoreFileCommentText`.

## Development

```bash
cargo test
```

Spec tests live in `tests/specs`. Each file contains one or more cases with
the input followed by an `[expect]` block.
