import { themes as prismThemes } from 'prism-react-renderer';
import type { Config } from '@docusaurus/types';
import type * as Preset from '@docusaurus/preset-classic';
import versionLabelsJson from './version-labels.json';
import versionsJson from './versions.json';

// Narrow the JSON-inferred types to what the docs preset expects, keeping
// only labels for versions that currently exist in versions.json. During
// `docs:version` a rolling snapshot is deleted (and removed from
// versions.json) before being recreated — referencing the missing version
// here would fail Docusaurus's config validation.
const existingVersions = new Set<string>(versionsJson);
const versionLabels = Object.fromEntries(
  Object.entries(versionLabelsJson as Record<
    string,
    { label: string; banner: 'none' }
  >).filter(([version]) => existingVersions.has(version)),
) as Record<string, { label: string; banner: 'none' }>;

// The rolling "current" docs display the real version number of the most
// recent release (versions.json is ordered newest-first), suffixed with
// "(latest)" so it stays distinguishable from the frozen snapshot entry,
// e.g. current → "1.0.1 (latest)", snapshot → "1.0.1".
const latestReleasedLabel = versionsJson[0]
  ? versionLabels[versionsJson[0]]?.label
  : undefined;

// ─── WHITE-LABEL CONFIGURATION ───────────────────────────────────────────────
// Change these values when forking for a new project
const SITE_CONFIG = {
  title: 'Rust MCP SDK',
  tagline: 'A high-performance, asynchronous Rust toolkit for building MCP servers and clients.',
  url: 'https://rust-mcp-stack.github.io',
  baseUrl: '/rust-mcp-sdk/',
  organizationName: 'rust-mcp-stack',
  projectName: 'rust-mcp-sdk',
  githubUrl: 'https://github.com/rust-mcp-stack/rust-mcp-sdk',
  editUrlBase: 'https://github.com/rust-mcp-stack/rust-mcp-sdk/tree/main/docs-site/',
};
// ─────────────────────────────────────────────────────────────────────────────

const config: Config = {
  title: SITE_CONFIG.title,
  tagline: SITE_CONFIG.tagline,
  favicon: 'img/favicon.ico',

  future: {
    v4: true,
  },

  url: SITE_CONFIG.url,
  baseUrl: SITE_CONFIG.baseUrl,

  organizationName: SITE_CONFIG.organizationName,
  projectName: SITE_CONFIG.projectName,

  onBrokenLinks: 'throw',

  i18n: {
    defaultLocale: 'en',
    locales: ['en'],
  },

  // ─── ANNOUNCEMENT BAR ───────────────────────────────────────────────────────
  // Uncomment the announcementBar block below to show a top banner.
  // customFields: {},
  // ────────────────────────────────────────────────────────────────────────────

  presets: [
    [
      'classic',
      {
        docs: {
          sidebarPath: './sidebars.ts',
          editUrl: SITE_CONFIG.editUrlBase,
          // Versioning — "current" (docs/) is the latest, served at /docs.
          // Rolling major snapshots live under versioned_docs/version-{major}.x/
          lastVersion: 'current',
          versions: {
            current: {
              label: latestReleasedLabel
                ? `${latestReleasedLabel} (latest)`
                : 'Latest',
              banner: 'none',
            },
            ...versionLabels,
          },
          showLastUpdateAuthor: false,
          showLastUpdateTime: false,
          // Table of contents
          remarkPlugins: [],
          rehypePlugins: [],
        },
        blog: false, // disable blog — enable if needed
        theme: {
          customCss: './src/css/custom.css',
        },
      } satisfies Preset.Options,
    ],
  ],

  plugins: [
    // ─── LOCAL SEARCH ─────────────────────────────────────────────────────────
    // Switch to Algolia DocSearch for production — see WHITELABEL.md
    [
      require.resolve('@easyops-cn/docusaurus-search-local'),
      {
        hashed: true,
        language: ['en'],
        highlightSearchTermsOnTargetPage: true,
        explicitSearchResultPath: true,
        searchBarPosition: 'right',
        docsRouteBasePath: '/docs',
        indexBlog: false,
      },
    ],
  ],

  themeConfig: {
    // ─── ANNOUNCEMENT BAR ───────────────────────────────────────────────────
    announcementBar: {
      id: 'announcement-v1',
      content: `🚀 <strong>v1.0.1 is out!</strong> &nbsp; <a href="${SITE_CONFIG.baseUrl}docs/migration/overview">See migration guide →</a> &nbsp;·&nbsp; 📚 <strong>Docs in beta</strong> — <a href="${SITE_CONFIG.githubUrl}/issues/new?template=documentation.md&labels=documentation">Report an issue</a>`,
      backgroundColor: 'var(--ifm-color-primary)',
      textColor: '#ffffff',
      isCloseable: true,
    },
    // ────────────────────────────────────────────────────────────────────────

    image: 'img/social-card.png',

    colorMode: {
      defaultMode: 'light',
      disableSwitch: false,
      respectPrefersColorScheme: false,
    },

    docs: {
      sidebar: {
        hideable: true,
        autoCollapseCategories: true,
      },
    },

    navbar: {
      hideOnScroll: false,
      logo: {
        alt: `${SITE_CONFIG.title} Logo`,
        src: 'img/rust-mcp-icon.png',
        href: '/',
        target: '_self',
      },
      items: [
        // ── Left side ──
        {
          type: 'docSidebar',
          sidebarId: 'mainSidebar',
          position: 'left',
          label: 'Docs',
        },
        // ── Version dropdown ──
        {
          type: 'docsVersionDropdown',
          position: 'left',
          dropdownActiveClassDisabled: true,
        },
        // ── Right side ──
        {
          href: `${SITE_CONFIG.githubUrl}/issues/new?template=documentation.md&labels=documentation`,
          position: 'right',
          className: 'navbar-beta-badge',
          label: '📚 Docs in beta\n· Report an issue',
        },
        {
          href: SITE_CONFIG.githubUrl,
          position: 'right',
          className: 'navbar-github-link',
          'aria-label': 'GitHub repository',
        },
      ],
    },

    footer: {
      style: 'light',
      links: [
        {
          label: 'Docs',
          to: '/docs/getting-started/welcome',
        },
        {
          label: 'GitHub',
          href: SITE_CONFIG.githubUrl,
        },
        {
          label: 'Changelog',
          to: '/docs/migration/changelog',
        },
      ],
      copyright: `Copyright © ${new Date().getFullYear()} ${SITE_CONFIG.title}`,
    },

    prism: {
      theme: prismThemes.oneLight,
      darkTheme: prismThemes.oneDark,
      additionalLanguages: [
        'bash',
        'json',
        'typescript',
        'yaml',
        'markdown',
        'rust',
        'toml',
      ],
      magicComments: [
            // Keep the default highlighting behavior.
            {
              className: 'theme-code-block-highlighted-line',
              line: 'highlight-next-line',
              block: {
                start: 'highlight-start',
                end: 'highlight-end',
              },
            },

            // Green: added line
            {
              className: 'code-block-added-line',
              line: 'add-next-line',
              block: {
                start: 'add-start',
                end: 'add-end',
              },
            },

            // Red: removed line
            {
              className: 'code-block-removed-line',
              line: 'delete-next-line',
              block: {
                start: 'delete-start',
                end: 'delete-end',
              },
            },
          ],
    },

    tableOfContents: {
      minHeadingLevel: 2,
      maxHeadingLevel: 3,
    },
  } satisfies Preset.ThemeConfig,
};

export default config;
