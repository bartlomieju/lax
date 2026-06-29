# lax-css

Lax CSS, SCSS, and Less formatter that never reinterprets your code, distributed
as a [dprint](https://dprint.dev) Wasm plugin.

This package ships the plugin's `plugin.wasm` so you can manage it with your
package manager (and tools like Renovate) instead of a pinned URL.

## Usage

Install the package:

```sh
npm install --save-dev lax-css
```

Reference the bundled Wasm file from your `dprint.json`:

```jsonc
{
  "plugins": [
    "./node_modules/lax-css/plugin.wasm"
  ]
}
```

Then run dprint as usual:

```sh
dprint fmt
```

## Source

Built from [`crates/lax-css`](https://github.com/bartlomieju/lax) in the
`bartlomieju/lax` monorepo. The published package includes
[npm provenance](https://docs.npmjs.com/generating-provenance-statements)
attesting it was built and published from that repository.

## License

MIT
