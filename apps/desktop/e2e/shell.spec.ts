import { test, expect, type Page } from '@playwright/test';
const pageErrors = new WeakMap<Page, string[]>();
test.beforeEach(({ page }) => { const errors: string[] = []; pageErrors.set(page, errors); page.on('pageerror', error => errors.push(error.message)); });
test.afterEach(async ({ page }, info) => { if (info.status !== info.expectedStatus) console.log('Video state', await page.locator('video').evaluateAll(videos => videos.map(v => ({ paused: (v as HTMLVideoElement).paused, width: (v as HTMLVideoElement).videoWidth, ready: (v as HTMLVideoElement).readyState, time: (v as HTMLVideoElement).currentTime, error: (v as HTMLVideoElement).error?.code })))); expect(pageErrors.get(page)).toEqual([]); });
test('camera setup requires a current successful connection test', async ({ page }) => {
  await page.goto('/e2e/harness.html');
  await expect(page.getByRole('button', { name: 'Add manually' })).toBeVisible();
  await page.screenshot({ path: '../../docs/screenshots/empty-state.png', fullPage: true });
  await page.getByRole('button', { name: 'Add manually' }).click();
  await page.getByLabel('Camera name', { exact: true }).fill('Front entrance');
  await page.getByLabel('Camera address or full RTSP URL').fill('127.0.0.1:8554');
  const save = page.getByRole('button', { name: 'Save camera', exact: true });
  await expect(save).toBeDisabled();
  await page.getByRole('button', { name: 'Test connection', exact: true }).click();
  await expect(save).toBeEnabled();
  await page.getByLabel('Camera name', { exact: true }).fill('North entrance');
  await expect(save).toBeDisabled();
  await page.getByRole('button', { name: 'Test connection', exact: true }).click();
  await save.click();
  await expect(page.getByRole('button', { name: 'Expand North entrance', exact: true })).toBeVisible();
});
test('nine-camera grid, quality switch, keyboard return, reorder and hidden pause', async ({ page }) => {
  await page.goto('/e2e/harness.html?count=9');
  await expect(page.locator('.camera-tile')).toHaveCount(9);
  if (process.env.WATCHTOWER_REAL_MEDIA) expect(await page.evaluate(() => (window as unknown as { __hasMedia: boolean }).__hasMedia)).toBe(true);
  if (await page.evaluate(() => (window as unknown as { __hasMedia: boolean }).__hasMedia)) {
    await expect.poll(() => page.locator('video').evaluateAll(videos => videos.filter(v => (v as HTMLVideoElement).readyState >= 2).length), { timeout: 45000 }).toBe(9);
    const before = await page.locator('video').evaluateAll(videos => videos.map(v => (v as HTMLVideoElement).currentTime));
    await expect.poll(() => page.locator('video').evaluateAll((videos, before) => videos.every((v, index) => (v as HTMLVideoElement).currentTime > before[index]), before), { timeout: 5000 }).toBe(true);
    await page.screenshot({ path: '../../docs/screenshots/live-grid.png', fullPage: true });
  }
  await page.getByRole('button', { name: 'Expand Test camera 01', exact: true }).click();
  await expect(page.locator('.camera-tile')).toHaveCount(1);
  await expect.poll(() => page.evaluate(() => (window as unknown as { __lastPlayback: { main: boolean } }).__lastPlayback?.main)).toBe(true);
  if (await page.evaluate(() => (window as unknown as { __hasMedia: boolean }).__hasMedia)) {
    await expect(page.locator('.video-readout')).toContainText('1280 × 720', { timeout: 25000 });
  }
  await page.keyboard.press('Escape');
  await expect(page.locator('.camera-tile')).toHaveCount(9);
  await page.getByRole('button', { name: 'Move Test camera 01 later', exact: true }).click();
  await expect(page.locator('.camera-name').first()).toHaveText('Test camera 02');
  await page.getByLabel('Grid layout').selectOption('2');
  await expect(page.locator('.camera-grid')).toHaveCSS('grid-template-columns', /\d+.*\d+/);
  await page.evaluate(() => (window as unknown as { __setMinimized: (value: boolean) => void }).__setMinimized(true));
  await expect(page.getByText('PLAYBACK PAUSED', { exact: true })).toBeVisible({ timeout: 5000 });
  await expect(page.locator('video')).toHaveCount(0);
});
test('discovery requires explicit interface selection', async ({ page }) => {
  await page.goto('/e2e/harness.html');
  await page.getByRole('button', { name: 'Find cameras', exact: true }).first().click();
  const scan = page.getByRole('dialog').getByRole('button', { name: 'Find cameras', exact: true });
  await expect(scan).toBeDisabled();
  await page.getByRole('checkbox', { name: /Test Ethernet/ }).check();
  await expect(scan).toBeEnabled(); await scan.click();
  await expect(page.getByText('0 devices found', { exact: true })).toBeVisible();
});

test('HLS receives video when WebRTC and MSE signalling fail', async ({ page }) => {
  test.skip(process.env.WATCHTOWER_TEST_ENGINE === 'retina', 'Retina advertises WebRTC only');
  await page.route('**/whep', route => route.abort());
  await page.addInitScript(() => {
    const Native = window.WebSocket;
    window.WebSocket = class extends Native {
      send(data: string | ArrayBufferLike | Blob | ArrayBufferView) {
        if (typeof data === 'string') {
          try { const type = JSON.parse(data).type; if (type === 'mse' || type === 'webrtc/offer') return; } catch { /* binary or non-JSON data */ }
        }
        super.send(data);
      }
    };
  });
  await page.goto('/e2e/harness.html?count=1');
  await expect(page.locator('.camera-tile')).toHaveCount(1);
  if (process.env.WATCHTOWER_REAL_MEDIA) expect(await page.evaluate(() => (window as unknown as { __hasMedia: boolean }).__hasMedia)).toBe(true);
  test.skip(!await page.evaluate(() => (window as unknown as { __hasMedia: boolean }).__hasMedia), 'Requires pnpm test:media');
  await expect(page.locator('.video-readout')).toContainText('HLS', { timeout: 45000 });
  await expect.poll(() => page.locator('video').evaluate(video => (video as HTMLVideoElement).readyState)).toBeGreaterThanOrEqual(2);
});


test('discovered camera keeps its advertised RTSP port and name', async ({ page }) => {
  await page.goto('/e2e/harness.html?found=1');
  await page.getByRole('button', { name: 'Find cameras', exact: true }).first().click();
  await page.getByRole('checkbox', { name: /Test Ethernet/ }).check();
  await page.getByRole('dialog').getByRole('button', { name: 'Find cameras', exact: true }).click();
  await page.getByRole('dialog').getByRole('button', { name: 'Add', exact: true }).click();
  await expect(page.getByLabel('Camera name', { exact: true })).toHaveValue('Discovered test');
  await expect(page.getByLabel('Camera address or full RTSP URL')).toHaveValue('127.0.0.1:8554');
});


test('editing a main-only camera preserves its empty substream', async ({ page }) => {
  await page.goto('/e2e/harness.html?count=1&no_sub=1');
  await page.getByRole('button', { name: 'Edit Test camera 01', exact: true }).click();
  await page.getByText('Stream paths', { exact: true }).click();
  await expect(page.getByLabel('Substream path (empty uses main)')).toHaveValue('');
  await page.getByRole('checkbox', { name: 'Use an empty password', exact: true }).check();
  await expect(page.getByLabel('Substream path (empty uses main)')).toBeEnabled();
});

test('engine preference persists and camera override invalidates its test', async ({ page }) => {
  await page.goto('/e2e/harness.html');
  await page.getByRole('button', { name: 'Settings', exact: true }).click();
  await page.getByLabel('Media engine', { exact: true }).selectOption('retina');
  await page.getByRole('button', { name: 'Save preferences' }).click();
  await page.getByRole('button', { name: 'Settings', exact: true }).click();
  await expect(page.getByLabel('Media engine', { exact: true })).toHaveValue('retina');
  await page.screenshot({ path: '../../docs/screenshots/engine-settings.png', fullPage: true });
  await page.getByRole('button', { name: 'Close panel', exact: true }).click();
  await page.getByRole('button', { name: 'Add manually', exact: true }).click();
  await page.getByLabel('Camera name', { exact: true }).fill('Engine test');
  await page.getByLabel('Camera address or full RTSP URL').fill('127.0.0.1');
  await page.getByRole('button', { name: 'Test connection', exact: true }).click();
  await expect(page.getByRole('button', { name: 'Save camera', exact: true })).toBeEnabled();
  await page.getByLabel('Media engine', { exact: true }).selectOption('mediamtx');
  await expect(page.getByRole('button', { name: 'Save camera', exact: true })).toBeDisabled();
  await page.getByRole('button', { name: 'Test connection', exact: true }).click();
  await page.getByRole('button', { name: 'Save camera', exact: true }).click();
  await page.getByRole('button', { name: 'Edit Engine test', exact: true }).click();
  await expect(page.getByLabel('Media engine', { exact: true })).toHaveValue('mediamtx');
});

test('native-only build offers Retina while preserving an unavailable saved choice', async ({ page }) => {
  await page.goto('/e2e/harness.html?native_only');
  await page.getByRole('button', { name: 'Settings', exact: true }).click();
  const engine = page.getByLabel('Media engine', { exact: true });
  await expect(engine.locator('option:not([disabled])')).toHaveCount(1);
  await expect(engine.locator('option:not([disabled])')).toHaveAttribute('value', 'retina');
  if (await engine.inputValue() !== 'retina') {
    await expect(engine.locator('option:checked')).toBeDisabled();
    await expect(engine.locator('option:checked')).toContainText('not included; using Retina');
  }
  await engine.selectOption('retina');
  await page.getByRole('button', { name: 'Save preferences' }).click();
  await page.getByRole('button', { name: 'Settings', exact: true }).click();
  await expect(page.getByLabel('Media engine', { exact: true })).toHaveValue('retina');
});

test('native and MediaMTX deliver synthetic video through WebRTC', async ({ page }) => {
  test.skip(!['retina', 'mediamtx'].includes(process.env.WATCHTOWER_TEST_ENGINE ?? ''), 'WHEP engine test');
  test.skip(!!process.env.WATCHTOWER_SAVED_SOURCES, 'Uses the synthetic H.264 fixture');
  const statuses: number[] = [];
  page.on('response', response => { if (response.url().endsWith('/whep')) statuses.push(response.status()); });
  await page.addInitScript(() => {
    const events: string[] = []; Object.assign(window, { __rtcDebug: events });
    const Native = window.RTCPeerConnection;
    window.RTCPeerConnection = class extends Native {
      constructor(config?: RTCConfiguration) {
        super(config);
        this.addEventListener('connectionstatechange', () => events.push(`peer:${this.connectionState}`));
        this.addEventListener('iceconnectionstatechange', () => events.push(`ice:${this.iceConnectionState}`));
        this.addEventListener('track', () => events.push('track'));
      }
    };
  });
  await page.goto('/e2e/harness.html?count=1');
  try { await expect(page.locator('.video-readout')).toContainText('WEBRTC', { timeout: 25000 }); }
  catch (error) { console.log('WHEP status/state', statuses, await page.evaluate(() => (window as unknown as { __rtcDebug: string[] }).__rtcDebug)); throw error; }
});
