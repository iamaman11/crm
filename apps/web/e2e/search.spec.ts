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

  test("handles unauthenticated/expired session locally", async ({ page }) => {
    await page.goto("/search");

    // Verify search page is loaded
    await expect(page.locator("h1")).toHaveText("Global search");

    // Expire the session dynamically
    await page.evaluate(() => {
      (window as any).sessionStore.clearProtectedState("expired");
    });

    // Verify the UI changes to "No authenticated session"
    const feedback = page.locator(".crm-feedback");
    await expect(feedback).toBeVisible();
    await expect(feedback.locator("h2")).toHaveText("No authenticated session");
  });

  test("handles invalid token from backend query", async ({ page }) => {
    await page.goto("/search");

    // Change session to have an invalid token
    await page.evaluate(() => {
      (window as any).sessionStore.setState({
        status: "authenticated",
        bearerToken: "invalid-token-12345-invalid-token-12345-invalid-token-12345",
        tenantId: "tenant-a",
      });
    });

    const input = page.locator("#search-input");
    await input.fill("Phase");

    const submitBtn = page.locator("#search-submit");
    await submitBtn.click();

    // Verify server error message
    const feedback = page.locator(".crm-feedback");
    await expect(feedback).toBeVisible();
    await expect(feedback.locator("h2")).toHaveText("Search failed");
  });

  test("handles unauthorized/invalid tenant from backend query", async ({ page }) => {
    await page.goto("/search");

    // Change tenant to unauthorized tenant-b
    await page.evaluate(() => {
      (window as any).sessionStore.setState({
        status: "authenticated",
        bearerToken: "phase6l-process-bearer-token-0123456789abcdef0123456789abcdef",
        tenantId: "tenant-b",
      });
    });

    const input = page.locator("#search-input");
    await input.fill("Phase");

    const submitBtn = page.locator("#search-submit");
    await submitBtn.click();

    // Verify permission denied message
    const feedback = page.locator(".crm-feedback");
    await expect(feedback).toBeVisible();
    await expect(feedback.locator("h2")).toHaveText("Search failed");
    await expect(feedback).toContainText("You are not permitted to access the requested tenant");
  });
});
