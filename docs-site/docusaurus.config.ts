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

// The version served at /docs by default (the LTS line). This stays pinned
// even when a newer major is released and listed in the dropdown.
const DEFAULT_VERSION = '1.x';

// The first entry in versions.json is the "latest" — append "(latest)" to its
// label so it stays distinguishable from the default version in the dropdown,
// e.g. latest → "2.0.0 (latest)", default → "1.1.0".
const latestVersion = versionsJson[0];
const versionLabelsWithLatest = {
  ...versionLabels,
  ...(latestVersion && versionLabels[latestVersion]
    ? {
        [latestVersion]: {
          label: `${versionLabels[latestVersion].label} (latest)`,
          banner: 'none' as const,
        },
      }
    : {}),
};

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
  // Umami Cloud website ID (cloud.umami.is → site settings). Empty = analytics
  // disabled; the tracker is only injected for production builds.
  umamiWebsiteId: 'a003cde7-1a07-4667-8369-6f8fc019eff6',
};
// ─────────────────────────────────────────────────────────────────────────────

// ─── ANALYTICS ───────────────────────────────────────────────────────────────
const umamiHeadTags =
  SITE_CONFIG.umamiWebsiteId && process.env.NODE_ENV === 'production'
    ? [
        {
          tagName: 'script',
          attributes: {
            defer: 'true',
            src: 'https://cloud.umami.is/script.js',
            'data-website-id': SITE_CONFIG.umamiWebsiteId,
            // Only record traffic hitting the real deployment host.
            'data-domains': `${SITE_CONFIG.url.replace(/^https?:\/\//, '')}`,
          },
        },
      ]
    : [];
// ─────────────────────────────────────────────────────────────────────────────

const config: Config = {
  title: SITE_CONFIG.title,
  tagline: SITE_CONFIG.tagline,
  favicon: 'img/favicon.ico',

  headTags: umamiHeadTags,

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
          // Versioning — "1.x" is the default (served at /docs). The latest
          // major (e.g. "2.x") is listed in the dropdown but is not the
          // default. The in-progress docs/ folder ("current") is hidden.
          lastVersion: DEFAULT_VERSION,
          includeCurrentVersion: false,
          versions: versionLabelsWithLatest,
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
      content: `🚀 <strong>v1.0.1 is out!</strong> &nbsp; <a href="${SITE_CONFIG.baseUrl}docs/migration/overview" data-umami-event="announcement-cta">See migration guide →</a> &nbsp;·&nbsp; 📚 <strong>Docs in beta</strong> — <a href="${SITE_CONFIG.githubUrl}/issues/new?template=documentation.md&labels=documentation">Report an issue</a>`,
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
