# lax-sql

Lax SQL formatter that never reinterprets your code, dialect agnostic by
construction, distributed as a [dprint](https://dprint.dev) Wasm plugin.

This package ships the plugin's `plugin.wasm` so you can manage it with your
package manager (and tools like Renovate) instead of a pinned URL.

## Usage

Install the package:

```sh
npm install --save-dev lax-sql
```

Reference the bundled Wasm file from your `dprint.json`:

```jsonc
{
  "plugins": [
    "./node_modules/lax-sql/plugin.wasm"
  ]
}
```

Then run dprint as usual:

```sh
dprint fmt
```

Matches `.sql` files.

## Source

Built from [`crates/lax-sql`](https://github.com/bartlomieju/lax) in the
`bartlomieju/lax` monorepo. The published package includes
[npm provenance](https://docs.npmjs.com/generating-provenance-statements)
attesting it was built and published from that repository.

## License

MIT
