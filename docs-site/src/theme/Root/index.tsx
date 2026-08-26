import React, { useEffect, useRef } from 'react';
import type { ReactNode } from 'react';
import { useLocation } from '@docusaurus/router';

// Debounce so search-as-you-type only records the settled query.
const SEARCH_DEBOUNCE_MS = 1000;

export default function Root({ children }: { children: ReactNode }): ReactNode {
  const location = useLocation();
  const lastTrackedRef = useRef('');

  // Search queries — two capture paths sharing one dedupe key space:
  //  1. URL param effect: covers direct landings on /search?q=... (bookmarks,
  //     result-page reloads) and any router-visible query updates.
  //  2. Delegated `input` listener: the local search plugin rewrites ?q= via
  //     history.replaceState while typing, which react-router does not observe;
  //     listening to the input element itself catches every keystroke stream.
  useEffect(() => {
    if (!location.pathname.endsWith('/search')) return;
    const query = new URLSearchParams(location.search).get('q')?.trim();
    if (!query) return;
    const key = `url:${query.toLowerCase()}`;
    if (lastTrackedRef.current === key) return;
    const timer = setTimeout(() => {
      lastTrackedRef.current = key;
      window.umami?.track('docs-search', { query });
    }, SEARCH_DEBOUNCE_MS);
    return () => clearTimeout(timer);
  }, [location.pathname, location.search]);

  useEffect(() => {
    let searchTimer: ReturnType<typeof setTimeout> | undefined;
    const onInput = (event: Event) => {
      const input = event.target as HTMLInputElement | null;
      if (!input || input.type !== 'search') return;
      if (searchTimer) clearTimeout(searchTimer);
      const query = input.value.trim();
      if (!query) return;
      searchTimer = setTimeout(() => {
        const key = `input:${query.toLowerCase()}`;
        if (lastTrackedRef.current === key) return;
        lastTrackedRef.current = key;
        window.umami?.track('docs-search', { query });
      }, SEARCH_DEBOUNCE_MS);
    };
    document.addEventListener('input', onInput, true);
    return () => {
      document.removeEventListener('input', onInput, true);
      if (searchTimer) clearTimeout(searchTimer);
    };
  }, []);

  // Click instrumentation: code-block copy buttons and outbound links. A single
  // delegated document-level listener survives Docusaurus's client-side
  // re-renders without per-element wiring.
  useEffect(() => {
    const onClick = (event: MouseEvent) => {
      const target = event.target as HTMLElement | null;
      if (!target) return;
      if (target.closest('.theme-code-block-copy-button')) {
        window.umami?.track('code-copy', { page: window.location.pathname });
        return;
      }
      const anchor = target.closest('a[href]');
      const href = anchor?.getAttribute('href');
      if (!href || !/^https?:\/\//i.test(href)) return;
      try {
        const url = new URL(href, window.location.href);
        if (url.origin === window.location.origin) return;
        window.umami?.track('outbound-link', {
          url: href,
          from: window.location.pathname,
        });
      } catch {
        // Malformed href — never worth breaking a navigation over.
      }
    };
    document.addEventListener('click', onClick);
    return () => document.removeEventListener('click', onClick);
  }, []);

  return <>{children}</>;
}
