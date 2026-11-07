import { expect, test } from "@playwright/test";

// These tests are deliberately excluded from the ordinary DEV-mode E2E run.
// They require the built browser bundle, the real cookie login, crm-api and PostgreSQL.
test.describe("G2 production-built CRM workflow", () => {
  // Mutation tests share an ephemeral PostgreSQL fixture. Retrying a partial
  // workflow would create a second record instead of replaying the same case.
  test.describe.configure({ retries: 0 });
  test.skip(!process.env.G2_E2E, "Runs only in the real PostgreSQL pilot acceptance stage");

  const prefix = process.env.G2_RUN_ID ?? "g2-e2e";
  const organization = prefix + " Organization";
  const renamedOrganization = prefix + " Organization Updated";
  const person = prefix + " Person";
  const deal = prefix + " Opportunity";
  const updatedDeal = prefix + " Opportunity Updated";
  const taskName = prefix + " Follow up";
  const updatedTask = prefix + " Call customer";
  const email = prefix.toLowerCase().replace(/[^a-z0-9]/g, "") + "@example.test";
  const phase = process.env.G2_TEST_PHASE ?? "before";

  async function signIn(page: import("@playwright/test").Page, user: "alice" | "bob") {
    await page.goto("/customers");
    await expect(page.getByRole("heading", { name: "Authentication required" })).toBeVisible();
    await page.getByRole("link", { name: "Sign in securely" }).click();
    await expect(page.getByRole("heading", { name: "Sign in to CRM" })).toBeVisible();
    await page.getByLabel("Username").fill(user);
    await page.getByLabel("Password").fill(user === "alice"
      ? "01234567890123456789012345678901"
      : "abcdefghijklmnopqrstuvwxyzABCDEF");
    await page.getByRole("button", { name: "Sign in" }).click();
    await expect(page.getByText(user, { exact: false }).first()).toBeVisible();
    await page.goto("/customers");
    await expect(page.getByRole("heading", { name: "Customers", exact: true })).toBeVisible();
    await expect(page.getByText("Request unavailable")).toHaveCount(0);
  }

  test("customer → contact → deal → task persists, with no duplicate UI effects", async ({ page }) => {
    test.skip(phase !== "before");
    await signIn(page, "alice");

    // Native label and button selectors intentionally mirror the normal user flow.
    await page.getByLabel("Type").selectOption({ label: "Organization" });
    await page.getByLabel("Name", { exact: true }).fill(organization);
    await page.getByRole("button", { name: "Save customer" }).dblclick();
    await expect(page.getByRole("heading", { name: organization })).toBeVisible();
    await expect(page.getByRole("status").filter({ hasText: /Customer saved|Customer account/ })).toBeVisible();
    await expect(page.getByRole("button", { name: organization + " · Organization", exact: true })).toHaveCount(1);

    await page.getByLabel("Email or phone").fill(email);
    await page.getByRole("button", { name: "Save contact" }).click();
    await expect(page.getByRole("status").filter({ hasText: "Contact saved." })).toBeVisible();
    await expect(page.getByText("Email: " + email)).toBeVisible();
    await page.getByRole("button", { name: "Edit", exact: true }).click();
    await page.getByLabel("Contact detail").fill("updated." + email);
    await page.getByRole("form").filter({ has: page.getByLabel("Contact detail") }).getByRole("button", { name: "Save contact" }).click();
    await expect(page.getByText("Email: updated." + email)).toBeVisible();

    await page.getByLabel("Customer name").fill(renamedOrganization);
    await page.getByRole("button", { name: "Save changes" }).click();
    await expect(page.getByRole("heading", { name: renamedOrganization })).toBeVisible();

    await page.getByLabel("Type").selectOption({ label: "Person" });
    await page.getByLabel("Name", { exact: true }).fill(person);
    await page.getByRole("button", { name: "Save customer" }).click();
    await expect(page.getByRole("heading", { name: person })).toBeVisible();

    await page.goto("/deals");
    await expect(page.getByRole("heading", { name: "Deals", exact: true })).toBeVisible();
    await page.getByLabel("Deal name").first().fill(deal);
    await page.getByLabel("Customer").selectOption({ label: organization });
    await page.getByRole("button", { name: "Save deal" }).click();
    await expect(page.getByRole("heading", { name: deal })).toBeVisible();
    await page.getByLabel("Deal name").last().fill(updatedDeal);
    await page.getByRole("button", { name: "Save changes" }).click();
    await expect(page.getByRole("heading", { name: updatedDeal })).toBeVisible();
    await page.getByRole("button", { name: "Advance to Qualified" }).click();
    await expect(page.getByText("Stage: qualified")).toBeVisible();
    await page.getByRole("button", { name: "Advance to Proposal" }).click();
    await expect(page.getByText("Stage: proposal")).toBeVisible();

    await page.goto("/tasks");
    await expect(page.getByRole("heading", { name: "Tasks", exact: true })).toBeVisible();
    await page.getByLabel("Subject").first().fill(taskName);
    await page.getByLabel("Related to").selectOption({ label: "Deal: " + updatedDeal });
    await page.getByRole("button", { name: "Save task" }).click();
    await expect(page.getByRole("heading", { name: taskName })).toBeVisible();
    await page.getByLabel("Subject").last().fill(updatedTask);
    await page.getByRole("button", { name: "Save changes" }).click();
    await expect(page.getByRole("heading", { name: updatedTask })).toBeVisible();
    await page.getByRole("button", { name: "Complete task" }).click();
    await expect(page.getByText("Status: Completed")).toBeVisible();

    await page.reload();
    await expect(page.getByRole("button", { name: new RegExp(updatedTask + " ·") })).toHaveCount(1);
    await page.goto("/customers");
    await page.getByLabel("Find by name").fill(renamedOrganization);
    await expect(page.getByRole("button", { name: new RegExp(renamedOrganization + " ·") })).toHaveCount(1);
    await page.getByRole("button", { name: new RegExp(renamedOrganization + " ·") }).click();
    await expect(page.getByText("Email: updated." + email)).toBeVisible();
  });

  test("tenant B cannot see tenant A records; session permissions remain server-owned", async ({ page }) => {
    test.skip(phase !== "before");
    await signIn(page, "bob");
    await page.getByLabel("Find by name").fill(renamedOrganization);
    await expect(page.getByRole("button", { name: new RegExp(renamedOrganization + " ·") })).toHaveCount(0);
    await page.goto("/deals");
    await expect(page.getByRole("button", { name: new RegExp(updatedDeal + " ·") })).toHaveCount(0);
    await page.goto("/tasks");
    await expect(page.getByRole("button", { name: new RegExp(updatedTask + " ·") })).toHaveCount(0);
    await page.getByRole("button", { name: "Sign out" }).click();
    await expect(page.getByRole("heading", { name: "Authentication required" })).toBeVisible();
    await page.goto("/customers");
    await expect(page.getByRole("heading", { name: "Authentication required" })).toBeVisible();
  });

  test("new sign-in after API restart sees persisted CRM records", async ({ page }) => {
    test.skip(phase !== "after");
    await signIn(page, "alice");
    await page.getByLabel("Find by name").fill(renamedOrganization);
    await expect(page.getByRole("button", { name: new RegExp(renamedOrganization + " ·") })).toHaveCount(1);
    await page.getByRole("button", { name: new RegExp(renamedOrganization + " ·") }).click();
    await expect(page.getByText("Email: updated." + email)).toBeVisible();
    await page.goto("/deals");
    await expect(page.getByRole("button", { name: new RegExp(updatedDeal + " ·") })).toHaveCount(1);
    await page.goto("/tasks");
    await expect(page.getByRole("button", { name: new RegExp(updatedTask + " ·") })).toHaveCount(1);
  });
});
