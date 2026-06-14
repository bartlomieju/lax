# lax

**Formatters that never get in your way.** lax accepts whatever is in the
wild, touches only whitespace, and leaves everything else exactly as you
wrote it. Unknown syntax, vendor extensions, dialects, half-broken files,
tomorrow's CSS that shipped today — all of it formats cleanly and stays
stable, because lax never tries to understand your code well enough to
mangle it.

Most formatters parse your file into a model of what they think it means,
then re-emit that model. Anything outside the model is an error or a
rewrite. lax does the opposite: it tokenizes losslessly, finds structure
only where structure is unambiguous, and adjusts the whitespace around it.
That one decision is why lax is both **faster** and **safer** than the
libraries it replaces.

| Crate                           | Formats                                 | Replaces      |
| ------------------------------- | --------------------------------------- | ------------- |
| [lax-core](crates/lax-core)     | shared printing machinery               | —             |
| [lax-css](crates/lax-css)       | CSS, SCSS, Less                         | malva         |
| [lax-sql](crates/lax-sql)       | SQL, dialect agnostic by construction   | sqlformat-rs  |
| [lax-markup](crates/lax-markup) | HTML, XML, SVG, Vue, Svelte, Astro, ... | markup_fmt    |

Each formatter is usable as a Rust library or as a dprint plugin. Together
they power `deno fmt`.

## Why lax

- **It won't reject your file.** No "unexpected token," no bailing out on a
  CSS nesting feature or a Snowflake-only SQL keyword. If lax can read it, it
  can format it — and if it can't make sense of a region, that region passes
  through untouched instead of erroring.
- **It won't change what your code *means*.** The only thing that ever moves
  is whitespace. No requoting, no case changes, no reordering, no invented or
  dropped tokens. Diffs stay small; semantics stay identical.
- **It's stable.** Format twice, get the same bytes. Every release is checked
  for idempotency and content-preservation across a large corpus.
- **It's fast.** A lossless tokenizer and a single-pass printer do far less
  work than a full parse-and-rebuild. See below.

## Performance

Measured per file (the real `deno fmt` workload) across the vendored test
corpora — thousands of real-world inputs from the prettier, biome, malva,
sqlfluff, and markup_fmt suites. Throughput in MB/s, higher is better.

| Language          | lax       | incumbent              | speedup  |
| ----------------- | --------- | ---------------------- | -------- |
| CSS / SCSS / Less | ~39 MB/s  | malva ~19 MB/s         | **~2x**  |
| SQL               | ~38 MB/s  | sqlformat-rs ~9 MB/s   | **~4.5x**|
| HTML / XML / SVG  | ~66 MB/s  | —                      | —        |

And the robustness story, which is the whole point:

> Of **1065** CSS corpus inputs, malva rejects **205** with a parse error.
> lax-css formats every one of them.

Numbers are from `cargo run --release --manifest-path benchmarks/Cargo.toml`
on an Apple M-series laptop; absolute throughput varies by machine, the
ratios are stable. The benchmark crate is kept out of the workspace so its
comparison dependencies never touch the main build or CI.

## Usage

As a Rust library:

```rust
use std::path::Path;
use dprint_core::configuration::GlobalConfiguration;
use lax_css::configuration::resolve_config;

let config = resolve_config(Default::default(), &GlobalConfiguration::default()).config;
let formatted = lax_css::format_text(Path::new("styles.css"), source, &config)?;
```

Each crate also ships a dprint plugin (`wasm32-unknown-unknown`), so it drops
into any dprint-based toolchain.

## Development

```bash
cargo test --workspace          # unit + corpus invariant tests
cargo clippy --workspace        # lints
cargo run --release --manifest-path benchmarks/Cargo.toml   # benchmarks
```

The corpus tests assert all three invariants — never errors, idempotent,
whitespace-only — at multiple widths and configurations, so a regression in
any of them fails CI.
