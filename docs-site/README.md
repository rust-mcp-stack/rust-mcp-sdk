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
| `docs/` | **Latest** docs, served at `/docs` — the source of truth | ✅ yes |
| `versioned_docs/version-{major}.x/` | Frozen snapshot per major line (e.g. `/docs/1.x/`) | ⚠️ auto-generated; hand-editable only once superseded (see "Maintaining a superseded major") |
| `versions.json` / `version-labels.json` | Which snapshots exist / what the dropdown displays | ❌ auto-managed (labels may be hand-bumped for superseded majors) |
| `sidebars.ts` | Navigation for latest docs **and** the template for snapshot sidebars | ✅ yes |
| `.docs-major` | The major that `docs/` currently describes (e.g. `1`) — gates auto-versioning | ✅ manual (flip when a new major's docs go live) |

Snapshot URLs are stable (`/docs/1.x/…` never changes). Only the displayed label updates per release (`1.0.1` → `1.1.0` → …).

## Deployment vs. versioning

These are independent:

| Event | What happens |
|-------|--------------|
| Merge any `docs-site/**` change to `main` | Deploys immediately — no release needed |
| Publish a **stable** release of the *current* major (minor/patch, e.g. `v2.0.1`) | Refreshes that major's snapshot from `docs/`, updates the dropdown label, then deploys |
| Publish a **stable** release of a *new* major (e.g. `v2.0.0`) while `.docs-major` still points at the old major | Skipped — docs are left untouched. Snapshot it manually when ready (see below) |
| Publish a **pre-release** | Docs are untouched (versioning is skipped on purpose) |
| Run the `Version Docs` workflow manually (`workflow_dispatch`) | Snapshots `docs/` into the given major's folder and sets its label |

## Common scenarios to keep in mind

**Fixing a typo or updating content**
Edit the file under `docs/`, open a PR, merge. Live after deploy.

**Adding a new page**
Create `docs/<section>/<page>.mdx` **and** register it in `sidebars.ts` — unregistered pages build but never appear in navigation. This applies to every future version snapshot too.

**Releasing a patch/minor of the current major (e.g. `v2.0.1`, `v2.1.0`)**
Automatic: the release re-snapshots `docs/` into `versioned_docs/version-{major}.x/` and bumps the dropdown label. Nothing to do.

**Releasing a new major whose docs aren't ready yet (e.g. `v2.0.0`)**
Publish it as a **stable** release — the workflow sees that the release major (`2`) doesn't match `.docs-major` (`1`) and skips the snapshot, so the docs are left untouched. `/docs/1.x/` stays complete for existing users. When the v2 docs are ready:

1. Rewrite `docs/` to describe v2 (drafts deploy live to `/docs` as you work).
2. Set `.docs-major` to `2`.
3. Trigger the `Version Docs` workflow manually (`workflow_dispatch`) with version `2.0.0` — or just wait for the next `v2.x` release, which snapshots automatically.

**Releasing a new major whose docs ARE ready**
Set `.docs-major` to the new major, then publish the stable release. The workflow snapshots `docs/` into `version-{major}.x/` and sets the label automatically — no manual step.

**Maintaining a superseded major (e.g. v1 after v2 ships)**
A superseded major's snapshot is frozen — the `.docs-major` gate means later releases can never overwrite it. To update it:

- Edit files under `versioned_docs/version-{major}.x/` **directly** (safe once superseded — nothing re-snapshots it).
- If you add a new page, also add it to `versioned_sidebars/version-{major}.x-sidebars.json`.
- Optionally bump its dropdown label in `version-labels.json` by hand.

**Broken links fail CI**
The site builds with `onBrokenLinks: 'throw'`. If CI fails after a docs change, check for a bad link first. Relative links between pages are preferred over absolute paths.

### Manually refreshing a snapshot

Two equivalent ways:

**A. Via GitHub Actions (preferred)**
Run the `Version Docs` workflow (`workflow_dispatch`) with the target version (e.g. `2.0.0`). It snapshots `docs/` into `versioned_docs/version-{major}.x/`, updates `versions.json` / `version-labels.json`, and pushes (which triggers the deploy). Also flip `.docs-major` to the new major so future releases of that major auto-version.

**B. Locally**
```bash
cd docs-site
rm -rf versioned_docs/version-1.x versioned_sidebars/version-1.x-sidebars.json
node -e "const fs=require('fs');const v=JSON.parse(fs.readFileSync('versions.json'));const i=v.indexOf('1.x');if(i>=0)v.splice(i,1);fs.writeFileSync('versions.json',JSON.stringify(v,null,2)+'\n');"
pnpm docusaurus docs:version 1.x
# then set the display label in version-labels.json
```

Committing these changes under `docs-site/**` triggers the deploy automatically.
