# lax

A family of formatters that never reinterpret your code: they accept
whatever is in the wild, adjust only whitespace, and make a best effort at
stable, pretty output. Anything unknown, broken, or ambiguous passes
through verbatim and stays stable across passes.

| Crate                            | Formats                                  |
| -------------------------------- | ---------------------------------------- |
| [lax-core](crates/lax-core)      | shared printing machinery                |
| [lax-css](crates/lax-css)        | CSS, SCSS, Less                          |
| [lax-sql](crates/lax-sql)        | SQL, dialect agnostic by construction    |
| [lax-markup](crates/lax-markup)  | HTML, XML, SVG, Vue, Svelte, Astro, ...  |

Each formatter is usable as a Rust library or as a dprint plugin, and each
is validated against a vendored corpus (prettier, biome, malva, sqlfluff,
and markup_fmt test inputs) with three machine checked invariants:
formatting never errors, formatting is idempotent, and nothing but
whitespace ever changes.

These crates power `deno fmt`.

## Development

```bash
cargo test --workspace
```
