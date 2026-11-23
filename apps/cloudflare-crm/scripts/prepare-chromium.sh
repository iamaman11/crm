#!/usr/bin/env bash
set -euo pipefail
# GitHub-hosted Ubuntu already ships many browser libraries; do not
# unconditionally call apt and depend on an external Ubuntu mirror.
# Verify by launching the SAME pinned Playwright Chromium headlessly.
for attempt in 1 2; do
  if npx playwright install chromium --only-shell; then
    break
  fi
  if [ "$attempt" -eq 2 ]; then
    echo "::error::Pinned Playwright Chromium download failed"
    exit 1
  fi
  sleep 10
done
launch_browser() {
  node --input-type=module -e '
    import { chromium } from "playwright";
    const browser = await chromium.launch({headless: true});
    const page = await browser.newPage();
    await page.goto("data:text/html,<h1>CRM browser preflight</h1>");
    if (await page.locator("h1").innerText() !== "CRM browser preflight")
      throw new Error("Chromium rendered unexpected content");
    await browser.close();
  '
}
if launch_browser; then
  echo "Chromium physically launched with existing system libraries"
else
  echo "::warning::Chromium needs OS libraries; installing declared Playwright dependencies"
  for attempt in 1 2; do
    if npx playwright install-deps chromium && launch_browser; then
      exit 0
    fi
    if [ "$attempt" -eq 2 ]; then
      echo "::error::Chromium system dependencies or physical launch failed"
      exit 1
    fi
    sleep 10
  done
fi
