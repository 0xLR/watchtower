import { test, expect } from '@playwright/test';
test('authorized saved stream plays in a grid', async ({ page }) => {
  test.skip(!process.env.WATCHTOWER_SAVED_SOURCES, 'Requires explicitly authorized saved-stream test');
  const errors: string[] = []; page.on('pageerror', error => errors.push(error.message));
  await page.goto('/e2e/harness.html?count=3');
  await expect(page.locator('.video-readout')).toHaveCount(3, { timeout: 45000 });
  if (process.env.WATCHTOWER_TEST_ENGINE === 'retina') await expect(page.locator('.video-readout').first()).toContainText('WEBRTC');
  for (const video of await page.locator('.camera-tile video').all()) {
    await expect.poll(() => video.evaluate(node => (node as HTMLVideoElement).currentTime), { timeout: 15000 }).toBeGreaterThan(1);
  }
  await page.screenshot({ path: `../../docs/screenshots/saved-${process.env.WATCHTOWER_TEST_ENGINE}.png`, fullPage: true });
  expect(errors).toEqual([]);
});
