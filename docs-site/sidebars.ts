import type { SidebarsConfig } from '@docusaurus/plugin-content-docs';

const sidebars: SidebarsConfig = {
  mainSidebar: [
    // ─── GET STARTED ───────────────────────────────────────────────────────
    {
      type: 'category',
      label: '📦 Get Started',
      collapsible: false,
      className: 'sidebar-section-title',
      items: [
        'getting-started/welcome',
        'getting-started/installation',
        'getting-started/quickstart',
        'getting-started/add-a-resource',
        'getting-started/add-a-resource-template',
        'getting-started/add-a-prompt',
        'getting-started/build-a-client',
      ],
    },
    // ─── TUTORIALS ─────────────────────────────────────────────────────────
    {
      type: 'category',
      label: '🎓 Tutorials',
      collapsible: true,
      collapsed: false,
      items: [
        'tutorials/build-your-first-mcp-server',
        'tutorials/build-your-first-mcp-client',
        'tutorials/deploy-your-server-over-http',
        'tutorials/add-oauth-to-your-server',
        'tutorials/embed-mcp-byo',
        'tutorials/long-running-tasks',
      ],
    },
    // ─── CORE CONCEPTS ─────────────────────────────────────────────────────
    {
      type: 'category',
      label: '🧠 Core Concepts',
      collapsible: true,
      collapsed: true,
      items: [
        'core-concepts/how-it-works',
        'core-concepts/server-essentials',
        'core-concepts/client-essentials',
        'core-concepts/transports',
        'core-concepts/handler-traits',
      ],
    },
    // ─── TRANSPORTS ─────────────────────────────────────────────────────────
    {
      type: 'category',
      label: '🚚 Transports',
      collapsible: true,
      collapsed: true,
      items: [
        'transports/overview',
        'transports/stdio',
        'transports/streamable-http',
        'transports/sse',
        'transports/client-transports',
        'transports/resumability',
        'transports/task-store',
        'transports/cancellation',
        'transports/sessions',
      ],
    },
    // ─── SERVERS ───────────────────────────────────────────────────────────
    {
      type: 'category',
      label: '🖥️ Servers',
      collapsible: true,
      collapsed: true,
      items: [
        'servers/overview',
        'servers/server-handler',
        'servers/server-handler-core',
        'servers/tools',
        'servers/resources',
        'servers/prompts',
        'servers/elicitation',
        'servers/sampling',
        'servers/tasks',
        'servers/logging',
        'servers/progress',
        'servers/roots',
        'servers/completions',
        'servers/notifications',
        'servers/message-observer',
        'servers/sessions',
      ],
    },
    // ─── CLIENTS ───────────────────────────────────────────────────────────
    {
      type: 'category',
      label: '📱 Clients',
      collapsible: true,
      collapsed: true,
      items: [
        'clients/overview',
        'clients/client-handler',
        'clients/client-handler-core',
        'clients/tools',
        'clients/resources',
        'clients/prompts',
        'clients/elicitation',
        'clients/sampling',
        'clients/tasks',
        'clients/roots',
        'clients/logging',
        'clients/progress',
        'clients/notifications',
      ],
    },
    // ─── HTTP BACKENDS ──────────────────────────────────────────────────────
    {
      type: 'category',
      label: '🌐 HTTP Backends',
      collapsible: true,
      collapsed: true,
      items: [
        'http-backends/overview',
        'http-backends/axum',
        'http-backends/actix',
        'http-backends/streamable-http',
        'http-backends/sse',
        'http-backends/byo-server',
        'http-backends/middleware',
        'http-backends/dns-rebinding',
        'http-backends/health-checks',
        'http-backends/tls',
      ],
    },
    // ─── AUTHENTICATION ────────────────────────────────────────────────────
    {
      type: 'category',
      label: '🔐 Authentication',
      collapsible: true,
      collapsed: true,
      items: [
        'auth/choosing',
        'auth/overview',
        {
          type: 'category',
          label: 'Server Authentication',
          items: [
            'auth/server-auth/remote-auth-provider',
            'auth/server-auth/token-verification',
            'auth/server-auth/oauth-proxy',
          ],
        },
        {
          type: 'category',
          label: 'Client Authentication',
          items: [
            'auth/client-auth/oauth-flow',
            'auth/client-auth/pkce',
            'auth/client-auth/dcr',
            'auth/client-auth/token-store',
            'auth/client-auth/client-config',
          ],
        },
        {
          type: 'category',
          label: 'Identity Providers',
          items: [
            'auth/providers/keycloak',
            'auth/providers/workos',
            'auth/providers/scalekit',
            'auth/providers/rust-mcp-extra',
          ],
        },
      ],
    },
    // ─── MACROS ────────────────────────────────────────────────────────────
    {
      type: 'category',
      label: '🛠️ Macros',
      collapsible: true,
      collapsed: true,
      items: [
        'macros/overview',
        'macros/mcp-tool',
        'macros/tool-box',
        'macros/mcp-elicit',
        'macros/mcp-resource',
        'macros/mcp-resource-template',
        'macros/mcp-icon',
        'macros/json-schema',
      ],
    },
    // ─── DEPLOYMENT ────────────────────────────────────────────────────────
    {
      type: 'category',
      label: '🚀 Deployment',
      collapsible: true,
      collapsed: true,
      items: [
        'deployment/running',
        'deployment/cargo-features',
        'deployment/tls',
        'deployment/production-checklist',
        'deployment/mcp-inspector',
      ],
    },
    // ─── EXAMPLES ──────────────────────────────────────────────────────────
    {
      type: 'category',
      label: '💡 Examples',
      collapsible: true,
      collapsed: true,
      items: [
        'examples/overview',
        {
          type: 'category',
          label: 'Getting Started',
          items: [
            'examples/servers/quick-start-server-stdio',
            'examples/servers/quick-start-streamable-http',
            'examples/clients/quick-start-client-stdio',
          ],
        },
        {
          type: 'category',
          label: 'Stdio',
          items: [
            'examples/servers/hello-world-server-stdio',
            'examples/servers/hello-world-server-stdio-core',
            'examples/clients/simple-mcp-client-stdio',
            'examples/clients/simple-mcp-client-stdio-core',
          ],
        },
        {
          type: 'category',
          label: 'Streamable HTTP',
          items: [
            'examples/servers/hello-world-server-streamable-http',
            'examples/servers/hello-world-server-streamable-http-core',
            'examples/servers/streamable-http-healthcheck',
            'examples/clients/simple-mcp-client-streamable-http',
            'examples/clients/simple-mcp-client-streamable-http-core',
          ],
        },
        {
          type: 'category',
          label: 'SSE',
          items: [
            'examples/clients/simple-mcp-client-sse',
            'examples/clients/simple-mcp-client-sse-core',
          ],
        },
        {
          type: 'category',
          label: 'Auth',
          items: [
            'examples/auth/server-oauth-remote',
            'examples/auth/client-oauth',
            'examples/auth/keycloak-auth',
            'examples/auth/workos-auth',
            'examples/auth/scalekit-auth',
          ],
        },
        {
          type: 'category',
          label: 'BYO-Server',
          items: [
            'examples/byo-server/byo-server-axum',
            'examples/byo-server/byo-server-actix',
          ],
        },
      ],
    },
    // ─── MIGRATION ─────────────────────────────────────────────────────────
    {
      type: 'category',
      label: '🔄 Migration',
      collapsible: true,
      collapsed: true,
      items: [
        'migration/overview',
        'migration/v0.10-to-v1.0',
        'migration/v0.9-to-v0.10',
        'migration/whats-new',
        'migration/changelog',
      ],
    },
    // ─── API REFERENCE ─────────────────────────────────────────────────────
    {
      type: 'doc',
      id: 'api-reference/overview',
      label: '📖 API Reference',
    },
    // ─── CONTRIBUTING ──────────────────────────────────────────────────────
    {
      type: 'doc',
      id: 'contributing',
      label: '🤝 Contributing',
    },
  ],
};

export default sidebars;
