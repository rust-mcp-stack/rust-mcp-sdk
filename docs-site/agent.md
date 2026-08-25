# System Instructions for AI Agents

**CONTEXT FOR AI:** 
You are an expert AI developer interacting with a white-label Docusaurus v3 template. This project was carefully crafted to mimic the premium look and feel of the Material UI (MUI) documentation website. 

The user is providing this repository to you so that you can rebrand and customize it for their specific product.

**YOUR DIRECTIVE:**
Your primary goal is to seamlessly transform this generic template into a customized documentation site for the user's project, while maintaining the high-quality typography, spacing, and CSS infrastructure that has already been built.

## Project Architecture & AI Guidelines

### 1. Configuration (`docusaurus.config.ts`)
- All primary project metadata is isolated at the top of `docusaurus.config.ts` inside the `SITE_CONFIG` object.
- **AI Task:** When rebranding, modify ONLY the properties inside `SITE_CONFIG` (title, tagline, url, githubUrl, etc.). Do not touch the lower parts of the config unless the user specifically asks to add new plugins or change search providers.
- Local search is already installed and working. Do not attempt to configure Algolia unless explicitly requested.

### 2. Theming & CSS (`src/css/custom.css`)
- This template relies entirely on a heavily customized CSS variable system (`:root`).
- **AI Task:** To rebrand the site, DO NOT write inline styles or generic CSS rules. Instead, locate the `--ifm-color-primary` variables at the top of `src/css/custom.css` and update the base color and its shade variants.
- Ensure you also update the dark mode variants located under the `[data-theme='dark']` block.
- Typography is set to IBM Plex Sans/Mono via `@import` at the top of the CSS file. Change the Google Font import and the `--ifm-font-family-base` variable if a different font is requested.

### 3. Content Management (`docs/` and `sidebars.ts`)
- The `docs/` directory contains sample boilerplate pages.
- **AI Task:** Before writing new documentation, use terminal commands to completely delete the existing sample folders inside `docs/` if the user wants to start fresh.
- Always ensure `sidebars.ts` is updated to match any new file structures you create. The sidebar uses a nested structure mimicking MUI's sidebar.

### 4. Versioning (`versioned_docs/`)
- Docs live in `docs/` (the "Latest" version served at `/docs`). Each major version keeps a single rolling snapshot under `versioned_docs/version-{major}.x/`.
- **AI Task:** Write new documentation pages in the `docs/` directory (the latest working version).
- To snapshot/refresh a major version, use `pnpm docusaurus docs:version <major>.x` (automated on `rust-mcp-sdk` releases via `.github/workflows/docs-version.yml`).

### 5. Custom Components (`src/components/`, `src/theme/`)
- Docusaurus component swizzling has already been applied where necessary (e.g., if custom navbars or badges exist).
- **AI Task:** Avoid ejecting/swizzling core Docusaurus components unless absolutely necessary. Rely on CSS customization first.

---

## AI Execution Checklist
When the user asks you to "Rebrand this template for my project", execute the following sequence:

1. **Ask for Details:** If not provided, ask the user for their: Project Name, Tagline, GitHub Repo URL, Primary Brand Color (Hex), and whether they want a custom font.
2. **Update Config:** Modify `SITE_CONFIG` in `docusaurus.config.ts`.
3. **Update Theme:** Modify the CSS variables in `src/css/custom.css` to match their brand color. Calculate the lighter/darker shades mathematically if necessary.
4. **Update Logos:** Use an SVG generator or ask the user to provide new SVGs for `static/img/logo.svg`, `static/img/logo-dark.svg`, and `static/img/favicon.svg`.
5. **Clear Boilerplate:** Propose a command to delete the sample `docs/` files and set up their first real page.
