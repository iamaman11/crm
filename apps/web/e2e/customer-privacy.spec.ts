import { expect, test } from "@playwright/test";

const PARTY_ID = "privacy-product-plane-party";
const TOKEN = "phase6l-process-bearer-token-0123456789abcdef0123456789abcdef";

test.describe("Customer Privacy product plane", () => {
  test("supports keyboard-only list and detail review against the real backend", async ({
    page,
  }) => {
    await page.goto("/customer/privacy");

    await expect(
      page.getByRole("heading", { name: "Customer Privacy cases" }),
    ).toBeVisible();
    const partyInput = page.getByLabel("Canonical Party reference");
    await partyInput.fill(PARTY_ID);
    await partyInput.press("Enter");

    await expect(page.getByRole("status")).toContainText(
      "1 privacy case loaded.",
    );
    await expect(
      page.getByRole("heading", { name: "Privacy cases" }),
    ).toBeFocused();

    const caseButton = page.getByRole("button", {
      name: /Erasure — Subject verified — Case /,
    });
    await expect(caseButton).toBeVisible();
    await caseButton.focus();
    await page.keyboard.press("Enter");

    await expect(page.getByRole("status")).toContainText(
      "Privacy case details loaded.",
    );
    await expect(
      page.getByRole("heading", { name: "Selected privacy case" }),
    ).toBeFocused();
    await expect(page.getByText("Subject verified", { exact: true })).toBeVisible();
    await expect(page.getByText("privacy-policy/1", { exact: true })).toBeVisible();
    await expect(page.getByText("Verified document", { exact: true })).toHaveCount(0);
    await expect(page.getByText("actor-a", { exact: true })).toHaveCount(0);
  });

  test("removes protected navigation when the session expires", async ({ page }) => {
    await page.goto("/customer/privacy");
    await expect(
      page.getByRole("link", { name: "Customer Privacy" }),
    ).toBeVisible();

    await page.evaluate(() => {
      window.sessionStore.clearProtectedState("expired");
    });

    await expect(
      page.getByRole("heading", { name: "Authentication required" }),
    ).toBeVisible();
    await expect(
      page.getByRole("link", { name: "Customer Privacy" }),
    ).toHaveCount(0);
  });

  test("conceals tenant-a cases from an authenticated tenant-b session", async ({
    page,
  }) => {
    await page.goto("/customer/privacy");
    await page.evaluate(
      ({ bearerToken }) => {
        window.sessionStore.setState({
          status: "authenticated",
          bearerToken,
          tenantId: "tenant-b",
          actorLabel: "Tenant B actor",
        });
      },
      { bearerToken: TOKEN },
    );

    const partyInput = page.getByLabel("Canonical Party reference");
    await partyInput.fill(PARTY_ID);
    await partyInput.press("Enter");

    await expect(
      page.getByRole("heading", { name: "Request unavailable" }),
    ).toBeFocused();
    await expect(page.getByText("The requested privacy case is not available to this session.")).toBeVisible();
    await expect(
      page.getByRole("button", { name: /Erasure — Subject verified — Case / }),
    ).toHaveCount(0);
  });
});
