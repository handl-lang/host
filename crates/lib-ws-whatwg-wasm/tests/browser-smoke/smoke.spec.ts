import { test, expect } from '@playwright/test';

test('browser websocket echo smoke', async ({ page }) => {
  await page.goto('http://localhost:8787/');
  const result = page.locator('#result');
  await expect(result).toHaveText('hello', { timeout: 10000 });
});
