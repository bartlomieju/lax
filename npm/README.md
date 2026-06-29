# npm packages

The dprint Wasm plugins are published to npm so users can install and pin them
with their package manager (and track updates with Renovate/Dependabot):

| npm package  | crate        | dprint config path                          |
| ------------ | ------------ | ------------------------------------------- |
| `lax-css`    | `lax-css`    | `./node_modules/lax-css/plugin.wasm`        |
| `lax-markup` | `lax-markup` | `./node_modules/lax-markup/plugin.wasm`     |
| `lax-sql`    | `lax-sql`    | `./node_modules/lax-sql/plugin.wasm`        |

Each package directory holds the static metadata (`package.json`, `index.js`,
`index.d.ts`, `README.md`). The `plugin.wasm` is built and dropped in by CI; the
version in `package.json` is overwritten from the crate version at publish time.

## How publishing works

npm publishing is one stage of the unified `.github/workflows/release.yml`
pipeline (see [RELEASING.md](../RELEASING.md)). After a crate version is bumped,
the workflow builds the Wasm plugin, stamps the crate version into
`package.json`, and runs `npm publish` via **npm trusted publishing (OIDC)** —
there is no `NPM_TOKEN` secret. Provenance is generated automatically (public
repo + public package + OIDC), which is why each `package.json` `repository.url`
points at `bartlomieju/lax`. Versions already on npm are skipped.

## One-time bootstrap (per package)

A trusted publisher can only be configured **after** a package's first version
exists on npm, and the settings page lives at a per-package URL. So each package
is bootstrapped once:

1. Build the Wasm and assemble the package locally:
   ```sh
   cargo build --release --features wasm --target wasm32-unknown-unknown -p lax-css
   cp target/wasm32-unknown-unknown/release/lax_css.wasm npm/lax-css/plugin.wasm
   ```
2. Publish the first version manually (requires `npm login` + 2FA):
   ```sh
   npm publish --access public ./npm/lax-css
   ```
3. Configure the trusted publisher at
   `https://www.npmjs.com/package/lax-css/access` → **Trusted Publisher** →
   GitHub Actions:
   - Organization or user: `bartlomieju`
   - Repository: `lax`
   - Workflow filename: `release.yml`
   - Environment: *(leave blank)*

Repeat for `lax-markup` and `lax-sql`. After that, npm publishes happen as part
of the release workflow with no tokens.
