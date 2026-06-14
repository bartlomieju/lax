# Working in this repo (agent guide)

This file is for AI agents and new contributors. It captures the architecture,
conventions, and workflows that aren't obvious from the code alone. Read it
before making changes.

## What lax is

A family of **lax** formatters: they never reinterpret your code, they only
adjust whitespace. The input is tokenized losslessly, structure is recognized
only where it is unambiguous, and the printer emits the same tokens with
normalized spacing. Anything unknown, dialect-specific, broken, or ambiguous
**passes through verbatim** and stays stable across passes.

This is the entire design philosophy and it is load-bearing. When in doubt:

- **Never invent or drop a token.** A semicolon the author omitted is not
  added; a token the author wrote is never removed. (Whitespace is the only
  thing that moves. One documented exception: a declaration the formatter can
  *fully see* may get a terminating separator where the grammar guarantees one,
  but an opaque/placeholder region never does.)
- **Never reinterpret.** No requoting, no case changes, no reordering, no
  rewriting values. If a region can't be understood, it is emitted as-is.
- **Degrade gracefully.** Unparseable input is not an error; it is passed
  through. Formatting must never fail on valid-but-weird input.

Two invariants follow and are machine-checked (see Testing): formatting is
**idempotent** (format twice = format once) and **content-preserving**
(nothing but whitespace changes).

## Repo layout

A Cargo workspace (edition 2024). Members:

| Crate                      | Formats                                  | Replaces      |
| -------------------------- | ---------------------------------------- | ------------- |
| `crates/lax-core`          | shared printing machinery (no formatter) | —             |
| `crates/lax-css`           | CSS, SCSS, Less                          | malva         |
| `crates/lax-sql`           | SQL (dialect agnostic)                   | sqlformat-rs  |
| `crates/lax-markup`        | HTML, XML, SVG, Vue, Svelte, Astro, ...  | markup_fmt    |

`benchmarks/` is a **workspace-excluded** crate (see `exclude` in the root
`Cargo.toml`) so its comparison dependencies (malva, sqlformat) never touch the
main build or CI. Run it explicitly with `--manifest-path benchmarks/Cargo.toml`.

These crates are consumed by `deno fmt` (the `deno` repo depends on the
published versions). See "Integration with deno" below.

## Architecture of a formatter crate

Every formatter crate (`lax-css`, `lax-sql`, `lax-markup`) has the same shape:

```
src/
  lib.rs                  # public exports (format_text, configuration)
  format_text.rs          # entry point: tokenize -> parse -> generate
  configuration/mod.rs    # Configuration struct + resolve_config()
  generation/
    tokenizer.rs          # lossless tokenizer; keeps raw &str slices
    parser.rs             # generic structure recognizer (statements/nodes)
    printer.rs            # emits PrintItems via lax-core helpers
    (keywords.rs)         # lax-sql only
  wasm_plugin.rs          # dprint Wasm plugin (gated behind `wasm` feature)
```

The pipeline in `format_text.rs::format_text_inner`:

1. **tokenize** the source into tokens that hold raw `&str` slices. The
   tokenizer is *lossless* — every byte is accounted for, comments and
   interpolations (`#{...}`, `${...}`, `@{...}`) are kept as opaque tokens.
2. **parse** generically: scan for unambiguous structure (e.g. CSS `{`/`;`/`}`,
   markup tags) and classify into statements/nodes. Unrecognized spans become
   "raw"/"verbatim" and are emitted untouched.
3. **generate** a dprint-core `PrintItems` stream, then run it through
   `dprint_core::formatting::format` with the resolved width/indent/newline
   options. `format_text` returns `Ok(None)` when the output equals the input.

### lax-core (the shared engine)

`lax-core` has no formatter of its own; it provides the printing primitives the
crates share. Key exports (`crates/lax-core/src/lib.rs`):

- `FlowPrinter` + `FlowClass` (`flow.rs`) — the heart of "lax" output. The
  printer feeds tokens to a `FlowPrinter` tagged with a `FlowClass`
  (`Whitespace { newlines }`, `Open`, `Close`, `Comma`, `CommaBreak`,
  `LineComment`, `Other`). It **preserves author newlines**, turns author
  spaces into soft wrap points (wrap at width), and indents nested
  paren/bracket groups by depth. It never introduces a break where the author
  had no whitespace.
- `push_text`, `push_text_line`, `push_comment` (`text.rs`) — emit verbatim
  text / comments into the `PrintItems` stream (comment interiors realign
  relative to their new column).
- `contains_directive`, `has_ignore_file_comment`, `HeaderToken` (`text.rs`) —
  support for `deno-fmt-ignore` / `deno-fmt-ignore-file` style directives.

If you change `FlowClass` or `FlowPrinter`, you are changing all three
formatters at once; bump `lax-core` and the crates that depend on it.

### Configuration

Each crate's `configuration/mod.rs` defines a `Configuration` struct (serde,
`camelCase`) and `resolve_config(ConfigKeyMap, &GlobalConfiguration)`.

- `Configuration` does **not** derive `Default`. Build one with
  `resolve_config(Default::default(), &GlobalConfiguration::default())` (this is
  how tests and consumers do it), or a full struct literal.
- New config fields: add the field with `#[serde(default)]`, and resolve it in
  `resolve_config` via `get_value(&mut config, "camelCaseKey", default, ...)`.
  A struct literal anywhere (incl. deno's fmt.rs) must then set the new field.

## Build / test / lint / format

Match CI exactly (`.github/workflows/ci.yml`):

```bash
cargo fmt --check                                  # formatting (rustfmt)
cargo clippy --workspace --all-features -- -D warnings   # lints
cargo test --workspace                             # unit + corpus + spec tests
# wasm plugin builds:
cargo build --release --features wasm --target wasm32-unknown-unknown -p lax-css
# (and -p lax-sql, -p lax-markup)
```

- **rustfmt config is `max_width = 120, tab_spaces = 2, edition = 2024`** (the
  repo's own rustfmt.toml). Note this differs from deno's 80 — code copied
  between repos needs reformatting for the target.
- Run a single crate's specs: `cargo test -p lax-css --test test`.
- CI runs `clippy --workspace` **without** `--all-targets`, so it checks
  lib/bins but not test/example targets. A lint that only fires under
  `--all-targets` won't fail CI (but is still worth fixing).
- The `benchmarks/` crate is excluded from `--workspace`; it is never built by
  CI.

## Testing

Three layers, all run by `cargo test --workspace`:

### Spec tests (`tests/test.rs` + `tests/specs/`)

Golden-file tests via the `dprint-development` `run_specs` harness with
`format_twice: true` (so every case also asserts idempotency). Spec file format:

```
~~ lineWidth: 40 ~~                 # optional config header, `~~ key: value ~~`
== name of the case ==
<input>

[expect]
<expected output>

== next case ==
...
```

- The config header maps to `resolve_config` keys (e.g. `lineWidth`,
  `singleLine`).
- **`[# ... ]` comments are NOT supported by this harness and will crash it
  (SIGABRT).** Put explanatory notes in the `== case name ==` instead.
- Expected output is compared byte-for-byte, **including the trailing
  newline**. A formatter that produces non-newline-terminated output (e.g. a
  single-line mode) must still newline-terminate for the harness; consumers
  that need it inline trim the result.

### Corpus tests (`tests/corpus_test.rs` + `tests/corpus/`)

~1000+ real-world inputs vendored from the prettier, biome, malva, sqlfluff,
and markup_fmt suites (inputs only; see `NOTICE.md`). The test asserts the
three invariants — **never errors, idempotent, content-preserving** — at
multiple widths. This is the safety net for the lax philosophy; if you add a
feature, make sure it still holds. (Corpus tests call `format_text` without an
external formatter.)

### External-formatter tests (`tests/external_test.rs`, lax-markup only)

lax-markup can hand embedded regions (`<script>`/`<style>` bodies, Astro
frontmatter, `{{ }}` interpolations) to an external formatter callback. These
tests pass a mock callback and assert the wiring. There is no spec-config way to
inject an external formatter, so embedded-formatting behavior is tested here.

### Adding a regression test

Prefer a spec case in `tests/specs/<lang>/<file>.txt`. For deno bug repros,
`lax-css` keeps them in `tests/specs/css/deno_issues.txt`.

## dprint plugins / wasm

Each crate compiles to a `wasm32-unknown-unknown` dprint plugin behind the
`wasm` feature (`wasm_plugin.rs`, gated on `target_arch = "wasm32"`). The wasm
API surface uses dprint's `FormatError`, not `anyhow`.

**`dprint-core` is pinned to `0.67.4`** in the root `Cargo.toml`
`[workspace.dependencies]`. Do not bump it casually: it must match the version
deno and the dprint plugin ecosystem use; 0.68's types are incompatible.

## Benchmarks

`benchmarks/` (excluded crate) measures lax vs the libraries it replaces on the
vendored corpora, per file:

```bash
cargo run --release --manifest-path benchmarks/Cargo.toml
```

It prints throughput (MB/s) and a robustness count (how many corpus inputs the
incumbent rejects that lax formats). Numbers are summarized in the README.
`benchmarks/target` and `benchmarks/Cargo.lock` are gitignored.

## Publishing / versioning

Crates are published to crates.io independently. Order matters because of path
deps: publish `lax-core` before crates that depend on it.

```bash
cargo publish -p lax-css --dry-run   # verify first
cargo publish -p lax-css
```

Bump the version in the crate's `Cargo.toml`, then update the workspace
`Cargo.lock` (e.g. `cargo update -p lax-css --precise <ver>`), commit both, push,
and publish. Current published baselines at time of writing: lax-core 0.1.2,
lax-css 0.2.4, lax-sql 0.2.1, lax-markup 0.2.4.

## Integration with deno

`deno fmt` depends on the published crates and calls `lax_*::format_text` from
`cli/tools/fmt.rs`. Two integration points worth knowing:

- **Inline style attributes** use lax-css's `single_line` config: it formats a
  declaration list on one line (normalizing colon/separator whitespace, no line
  breaks, no invented tokens). `format_text` newline-terminates the result;
  the markup caller trims it.
- **`{{ }}` interpolations** in markup are handed to the external formatter as
  JavaScript expressions (`lang = "ts"`); the result has its trailing
  statement `;`/newline stripped and the braces normalized to `{{ expr }}`.
  Anything that doesn't parse as JS (mustache `{{#...}}`/`{{/...}}`/`{{.}}`,
  Nunjucks/Vento filters, triple-brace `{{{ }}}`, plain text) is kept verbatim
  and never fails the format. Vue/Svelte single-brace `{expr}` and binding
  attributes (`:class`, `v-for`) are intentionally **not** formatted.

When changing behavior here, the deno side needs: a version bump of the dep,
and regeneration of the affected fmt fixtures (run deno's fmt spec/integration
tests to get exact expected output — don't hand-write golden files).

## Quick gotchas

- Only whitespace moves. If a change rewrites a token, it's a bug, not a feature.
- `Configuration` has no `Default`; use `resolve_config(Default::default(), ...)`.
- Spec files: no `[# ]` comments; expectations include the trailing newline.
- rustfmt `max_width = 120` here (deno uses 80).
- `dprint-core` stays at `0.67.4`.
- `benchmarks/` is excluded from the workspace on purpose.
- Don't run multiple heavy `cargo` builds/tests in parallel — it's memory-hungry.
