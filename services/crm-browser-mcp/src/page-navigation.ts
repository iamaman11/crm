import { isCrmRequest, safePagePath } from "./security.ts";

// A minimal, testable Playwright Page contract. The Durable Object remains
// the sole owner of the real browser, context, Page and CRM credential policy.
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
  setTrustedCrmNavigation: (enabled: boolean) => void,
  timeoutMs = 25_000,
) {
  // A controller-directed main-frame navigation may be authorized for the
  // exact CRM origin only. Release that permission after errors and redirects.
  setTrustedCrmNavigation(isCrmRequest(destination));
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
    setTrustedCrmNavigation(false);
  }
}
