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
| `versioned_docs/version-{major}.x/` | Frozen snapshot per major line (e.g. `/docs/1.x/`) | ❌ auto-generated |
| `versions.json` / `version-labels.json` | Which snapshots exist / what the dropdown displays | ❌ auto-managed |
| `sidebars.ts` | Navigation for latest docs **and** the template for snapshot sidebars | ✅ yes |

Snapshot URLs are stable (`/docs/1.x/…` never changes). Only the displayed label updates per release (`1.0.1` → `1.1.0` → …).

## Deployment vs. versioning

These are independent:

| Event | What happens |
|-------|--------------|
| Merge any `docs-site/**` change to `main` | Deploys immediately — no release needed |
| Publish a **stable** `rust-mcp-sdk-v*` release | Refreshes that major's snapshot from `docs/`, updates the dropdown label, then deploys |
| Publish a **pre-release** | Docs are untouched (versioning is skipped on purpose) |

## Common scenarios to keep in mind

**Fixing a typo or updating content**
Edit the file under `docs/`, open a PR, merge. Live after deploy.

**Adding a new page**
Create `docs/<section>/<page>.mdx` **and** register it in `sidebars.ts` — unregistered pages build but never appear in navigation. This applies to every future version snapshot too.

**Never edit `versioned_docs/version-*/` by hand**
Manual edits are overwritten on the next release. If a fix belongs in an old major, it goes into `docs/` first, then the snapshot is refreshed by a release (or manually — see below).

**Releasing a patch/minor (v1.x.y)**
Automatic: the workflow re-snapshots `docs/` into `versioned_docs/version-1.x/` and bumps the dropdown label. Nothing to do.

**Releasing a new major whose docs aren't ready yet**
Publish it as a **pre-release** (e.g. `v2.0.0` flagged pre-release, or `v2.0.0-rc.1`). Docs stay untouched. Draft the new docs directly in `docs/` — they deploy live as you write, while `/docs/1.x/` stays complete for existing users. When the docs are ready, cut a stable `v2.x.y` release and everything (snapshot + dropdown) updates automatically.

**Don't release old majors after a new one ships**
A later `v1.x` tag would re-snapshot the *current* `docs/` — which by then describes v2 — into the frozen `1.x` snapshot. Old-major snapshots are read-only history.

**Broken links fail CI**
The site builds with `onBrokenLinks: 'throw'`. If CI fails after a docs change, check for a bad link first. Relative links between pages are preferred over absolute paths.

### Manually refreshing a snapshot (rare)

Only needed if a snapshot must update without cutting a release:

```bash
cd docs-site
rm -rf versioned_docs/version-1.x versioned_sidebars/version-1.x-sidebars.json
node -e "const fs=require('fs');const v=JSON.parse(fs.readFileSync('versions.json'));const i=v.indexOf('1.x');if(i>=0)v.splice(i,1);fs.writeFileSync('versions.json',JSON.stringify(v,null,2)+'\n');"
pnpm docusaurus docs:version 1.x
# then set the display label in version-labels.json
```

Committing these changes under `docs-site/**` triggers the deploy automatically.
