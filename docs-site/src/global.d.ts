// Umami analytics tracker (https://umami.is/docs/tracker) injected via
// headTags in docusaurus.config.ts for production builds only.
interface UmamiTracker {
  track(event?: string, data?: Record<string, unknown>): void;
}

declare global {
  interface Window {
    umami?: UmamiTracker;
  }
}

export {};
