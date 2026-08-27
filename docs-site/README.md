# Documentation Site

The [rust-mcp-sdk documentation](https://rust-mcp-stack.github.io/rust-mcp-sdk/), built with [Docusaurus](https://docusaurus.io/) and managed with [pnpm](https://pnpm.io/).

## Quick start

```bash
pnpm install        # first time / after package.json changes
pnpm start          # dev server with hot reload
pnpm build          # production build → build/
pnpm typecheck      # TypeScript check
```

Requires Node >= 20.

## How docs are organized

| Path | What it is | Editable? |
|------|-----------|-----------|
| `docs/` | In-progress docs ("current") — hidden from the site, used as the source for future snapshots | ✅ yes |
| `versioned_docs/version-{major}.x/` | Frozen snapshot per major line (e.g. `/docs/1.x/`, `/docs/2.x/`) | ✅ manual |
| `versions.json` / `version-labels.json` | Which snapshots exist / what the dropdown displays | ✅ manual (see "Version model") |
| `sidebars.ts` | Navigation for latest docs **and** the template for snapshot sidebars | ✅ yes |
| `.docs-major` | The major that `docs/` currently describes (e.g. `2`) — informational only | ✅ manual |

Snapshot URLs are stable (`/docs/1.x/…` never changes). Only the displayed label is bumped by hand (`1.1.0` → `2.0.0` → …).

## Version model

The site deliberately decouples **default** from **latest**:

- `1.x` is the **default** — served at `/docs`, set by `lastVersion` in
  `docusaurus.config.ts`. It stays pinned even after a newer major ships.
- The first entry of `versions.json` is the **latest** and is shown in the
  dropdown with a `(latest)` suffix (e.g. `2.0.0 (latest)`), but it is **not**
  the default landing page.
- The in-progress `docs/` folder ("current") is hidden from the site via
  `includeCurrentVersion: false`.

This is why auto-versioning on release is disabled: versions are cut by hand so
a release can never overwrite the frozen `1.x` LTS snapshot or the `2.x` stub.
To re-enable release auto-versioning later, un-comment the `release:` trigger in
`.github/workflows/docs-version.yml`.

## Deployment vs. versioning

These are independent:

| Event | What happens |
|-------|--------------|
| Merge any `docs-site/**` change to `main` | Deploys immediately — no release needed |
| Publish a release (any kind) | Docs are **not** touched — auto-versioning is disabled; versions are managed by hand |
| Run the `Version Docs` workflow manually (`workflow_dispatch`) | Snapshots `docs/` into the given major's folder and sets its label |

## Common scenarios to keep in mind

**Fixing a typo or updating content**
Edit the file under `docs/`, open a PR, merge. Live after deploy.

**Adding a new page**
Create `docs/<section>/<page>.mdx` **and** register it in `sidebars.ts` — unregistered pages build but never appear in navigation. This applies to every future version snapshot too.

**Releasing a patch/minor (e.g. `v2.0.1`, `v2.1.0`)**
Manual: run the `Version Docs` workflow (`workflow_dispatch`) with the version
number. It re-snapshots `docs/` into `version-{major}.x/` and bumps the dropdown
label. Nothing happens automatically on release.

**Releasing a new major whose docs aren't ready yet (e.g. `v2.0.0`)**
Publish the release normally — docs are left untouched (auto-versioning is
disabled). When the docs are ready:
1. Rewrite `docs/` to describe the new major.
2. Trigger the `Version Docs` workflow manually with the version number.

**Releasing a new major whose docs ARE ready**
Trigger the `Version Docs` workflow manually with the version number (e.g.
`2.0.0`). It snapshots `docs/` into `version-{major}.x/` and sets the label.
Then edit `docusaurus.config.ts` if the new major should become the default
(`DEFAULT_VERSION`).

**Maintaining a superseded major (e.g. v1 after v2 ships)**
A superseded major's snapshot is frozen — with auto-versioning disabled, nothing
re-snapshots it. To update it:

- Edit files under `versioned_docs/version-{major}.x/` **directly** (safe once superseded — nothing re-snapshots it).
- If you add a new page, also add it to `versioned_sidebars/version-{major}.x-sidebars.json`.
- Optionally bump its dropdown label in `version-labels.json` by hand.

**Broken links fail CI**
The site builds with `onBrokenLinks: 'throw'`. If CI fails after a docs change, check for a bad link first. Relative links between pages are preferred over absolute paths.

### Manually refreshing a snapshot

Two equivalent ways:

**A. Via GitHub Actions (preferred)**
Run the `Version Docs` workflow (`workflow_dispatch`) with the target version (e.g. `2.0.0`). It snapshots `docs/` into `versioned_docs/version-{major}.x/`, updates `versions.json` / `version-labels.json`, and pushes (which triggers the deploy).

**B. Locally**
```bash
cd docs-site
rm -rf versioned_docs/version-1.x versioned_sidebars/version-1.x-sidebars.json
node -e "const fs=require('fs');const v=JSON.parse(fs.readFileSync('versions.json'));const i=v.indexOf('1.x');if(i>=0)v.splice(i,1);fs.writeFileSync('versions.json',JSON.stringify(v,null,2)+'\n');"
pnpm docusaurus docs:version 1.x
# then set the display label in version-labels.json
```

Committing these changes under `docs-site/**` triggers the deploy automatically.
