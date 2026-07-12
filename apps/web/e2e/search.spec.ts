import { test, expect } from "@playwright/test";

test.describe("Global Search E2E Workflow", () => {
  test("performs global search successfully and renders hits", async ({ page }) => {
    // Navigate to the search page
    await page.goto("/search");

    // Check that we are on the global search page
    await expect(page.locator("h1")).toHaveText("Global search");

    // Fill search input
    const input = page.locator("#search-input");
    await input.fill("Phase");

    // Click submit/search button
    const submitBtn = page.locator("#search-submit");
    await submitBtn.click();

    // Verify search hit card appears
    const hitCard = page.locator("[data-testid='search-hit']").first();
    await expect(hitCard).toBeVisible();

    // Verify the search hit details
    await expect(hitCard.locator(".crm-hit-card-title")).toContainText("Phase");
  });

  test("handles empty/no results cleanly", async ({ page }) => {
    await page.goto("/search");

    const input = page.locator("#search-input");
    await input.fill("NonExistentRecordNameXYZ");

    const submitBtn = page.locator("#search-submit");
    await submitBtn.click();

    // Verify feedback panel for "No results found" is visible
    const feedback = page.locator(".crm-feedback");
    await expect(feedback).toBeVisible();
    await expect(feedback.locator("h2")).toHaveText("No results found");
  });
});
