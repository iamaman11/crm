import { isConfiguredSiteRequest, safePagePath } from "./security.ts";

// A minimal, testable Playwright Page contract. The Durable Object remains
// the sole owner of the real browser, context, Page and optional site credentials.
export interface NavigablePage {
  url(): string;
  title(): Promise<string>;
  goto(
    destination: string,
    options: { waitUntil: "domcontentloaded"; timeout: number },
  ): Promise<{ status(): number } | null>;
}

export async function navigatePage(
  page: NavigablePage,
  destination: string,
  setTrustedSiteNavigation: (enabled: boolean) => void,
  timeoutMs = 25_000,
  privilegedOrigin: string | null = null,
) {
  // Trust is ephemeral and only exists for explicitly configured service auth.
  setTrustedSiteNavigation(isConfiguredSiteRequest(destination, privilegedOrigin));
  try {
    const response = await page.goto(destination, {
      waitUntil: "domcontentloaded",
      timeout: timeoutMs,
    });
    return {
      http_status: response?.status() ?? null,
      path: safePagePath(page.url()),
      title: await page.title(),
    };
  } finally {
    setTrustedSiteNavigation(false);
  }
}
