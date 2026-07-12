import { expect, test } from "@playwright/test";

test.describe("typed UI extension runtime", () => {
  test("keeps the shell, record page, and sibling extensions alive after isolated failures", async ({
    page,
  }) => {
    await page.goto("/records/phase7i-demo");

    await expect(page.locator("h1")).toHaveText("Record page");
    await expect(page.getByTestId("record-core-content")).toBeVisible();
    await expect(page.getByTestId("healthy-main-extension")).toBeVisible();
    await expect(page.getByTestId("healthy-sidebar-extension")).toBeVisible();

    const fallbacks = page.getByTestId("ui-extension-fallback");
    await expect(fallbacks).toHaveCount(2);
    await expect(page.getByTestId("ui-extension-failure-evidence")).toContainText(
      "2 isolated extension failures recorded.",
    );

    const renderFailure = page.locator(
      '[data-extension-coordinate="crm.activities:deal.render-failure-proof@record.detail.sidebar"]',
    );
    const loadFailure = page.locator(
      '[data-extension-coordinate="crm.sales-activities-link:deal.load-failure-proof@record.detail.sidebar"]',
    );

    await expect(renderFailure).toContainText("Extension unavailable");
    await expect(loadFailure).toContainText("Extension unavailable");

    await renderFailure.getByTestId("ui-extension-retry").click();
    await expect(renderFailure).toContainText("Extension unavailable");
    await expect(page.getByTestId("ui-extension-failure-evidence")).toContainText(
      "UI_EXTENSION_RENDER_FAILED · crm.activities:deal.render-failure-proof@record.detail.sidebar · attempt 2",
    );

    await expect(page.getByTestId("record-core-content")).toBeVisible();
    await expect(page.getByTestId("healthy-main-extension")).toBeVisible();
    await expect(page.getByTestId("healthy-sidebar-extension")).toBeVisible();
    await expect(page.locator(".crm-shell")).toBeVisible();
  });
});
