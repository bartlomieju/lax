# Releasing

All releases go through a single workflow: **Actions → Release → Run workflow**
(`.github/workflows/release.yml`). It is the only thing you trigger.

## What it does

Inputs:

- **crates** — `all`, or a comma list of `lax-core,lax-css,lax-markup,lax-sql`.
- **bump** — `patch` / `minor` / `major`, applied to every selected crate.

Given those, the workflow:

1. **Bumps** each selected crate's version (`cargo set-version`), commits the
   `Cargo.toml` / `Cargo.lock` changes to `main`, tags `‹crate›-v‹version›`, and
   cuts a GitHub release per tag on this repo.
2. **Publishes to crates.io** the selected crates, in dependency order
   (`lax-core` first), via crates.io trusted publishing (OIDC).
3. For each selected **plugin** crate (`lax-css` / `lax-markup` / `lax-sql`),
   builds the Wasm and publishes it to **npm** (OIDC trusted publishing) and as a
   **dprint registry** GitHub release on the per-plugin repo
   (`bartlomieju/lax-‹name›`).

The crate version is the single source of truth — the npm package version and
the dprint release tag both equal it. Every publish step skips a version that
already exists, so a failed **downstream** job can be re-run safely. Re-running
the **whole** workflow bumps again, so re-run the failed job, not the workflow.

Typical flow: merge your feature PRs, then run the workflow with the crates you
changed and the bump level. For example, to ship a fix to the SQL and markup
plugins: `crates = lax-markup,lax-sql`, `bump = patch`.

## One-time setup

No long-lived publish tokens are stored. Each registry uses OIDC trusted
publishing tied to this repo and the `release.yml` workflow, so configure it once
per package/crate:

1. **crates.io** — for each crate (`lax-core`, `lax-css`, `lax-markup`,
   `lax-sql`): crate Settings → Trusted Publishing → add GitHub repo
   `bartlomieju/lax`, workflow `release.yml`. (All four crates already exist on
   crates.io, so this is just a settings change.)
2. **npm** — the packages don't exist on npm yet, so bootstrap each once and then
   add its trusted publisher. See [npm/README.md](./npm/README.md).
3. **dprint registry** — add a repo secret `PLUGIN_RELEASE_TOKEN`: a fine-grained
   PAT with Contents: read/write on `bartlomieju/lax-css`, `lax-markup`, and
   `lax-sql`. The default `GITHUB_TOKEN` cannot write releases to other repos.

## First run

Trusted publishing can only be verified live. Do a low-risk first release — e.g.
`crates = lax-sql`, `bump = patch` — and confirm the bump commit, crates.io
publish, npm publish, and the two GitHub releases all land before relying on it.
