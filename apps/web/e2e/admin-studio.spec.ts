import { expect, test } from "@playwright/test";

test.describe("Admin Studio governed metadata lifecycle", () => {
  test("publishes, reviews, confirms a breaking activation, and rolls back", async ({ page }) => {
    await page.goto("/admin/metadata");

    await expect(page.locator("h1")).toHaveText("Admin Studio");
    await expect(page.locator("#metadata-publish")).toBeVisible();

    await page.locator("#metadata-publish").click();
    const candidateRevision = page.getByTestId("metadata-candidate-revision");
    await expect(candidateRevision).toBeVisible();
    const firstRevision = (await candidateRevision.textContent())?.trim();
    expect(firstRevision).toBeTruthy();

    await page.locator("#metadata-review-impact").click();
    await expect(page.getByTestId("metadata-impact")).toBeVisible();
    await expect(page.locator("#metadata-activate")).toBeEnabled();
    await page.locator("#metadata-activate").click();
    await expect(page.getByTestId("metadata-active-revision")).toHaveText(firstRevision!);

    await page.locator("#metadata-object-id").fill("crm.custom.asset_v2");
    await page.locator("#metadata-label").fill("Asset v2");
    await page.locator("#metadata-plural-label").fill("Assets v2");
    await page.locator("#metadata-publish").click();
    await expect(candidateRevision).toBeVisible();
    const secondRevision = (await candidateRevision.textContent())?.trim();
    expect(secondRevision).toBeTruthy();
    expect(secondRevision).not.toBe(firstRevision);

    await page.locator("#metadata-review-impact").click();
    const impact = page.getByTestId("metadata-impact");
    await expect(impact).toBeVisible();
    await expect(impact).toContainText("Breaking changes");

    const confirmation = page.locator("#metadata-confirm-breaking");
    const activate = page.locator("#metadata-activate");
    await expect(confirmation).toBeVisible();
    await expect(activate).toBeDisabled();
    await confirmation.check();
    await expect(activate).toBeEnabled();

    await activate.click();
    await expect(page.getByTestId("metadata-active-revision")).toHaveText(secondRevision!);

    const rollback = page.locator("#metadata-rollback");
    await expect(rollback).toBeEnabled();
    await rollback.click();
    await expect(page.getByTestId("metadata-active-revision")).toHaveText(firstRevision!);
  });
});
