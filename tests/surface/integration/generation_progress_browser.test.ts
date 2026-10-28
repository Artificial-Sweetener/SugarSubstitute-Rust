/** @jest-environment node */
//    SugarCubes - composable workflow units for ComfyUI
//    Copyright (C) 2026  Artificial Sweetener and contributors
//
//    This program is free software: you can redistribute it and/or modify
//    it under the terms of the GNU Affero General Public License as published by
//    the Free Software Foundation, either version 3 of the License, or
//    (at your option) any later version.
//
//    This program is distributed in the hope that it will be useful,
//    but WITHOUT ANY WARRANTY; without even the implied warranty of
//    MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
//    GNU Affero General Public License for more details.
//
//    You should have received a copy of the GNU Affero General Public License
//    along with this program.  If not, see <https://www.gnu.org/licenses/>.
/** Prove Substitute's two-track geometry and reversible native progress presentation. */
import { createServer, type Server } from 'node:http';
import { build } from 'esbuild';
import { chromium, type Browser, type Page } from '@playwright/test';
import { launchHeadlessBrowser } from '../../frontend_core/support/BrowserFixtureLifetime.js';
import type {} from '../support/GenerationProgressBrowser.js';
import { closeBrowserFixture } from '../../frontend_core/support/BrowserFixtureLifetime.js';

let server: Server;
let browser: Browser;
let pageReady: Promise<Page> | undefined;
let address: string;

beforeAll(async () => {
  const result = await build({
    entryPoints: ['tests/surface/support/GenerationProgressBrowser.ts'],
    bundle: true,
    format: 'esm',
    platform: 'browser',
    target: 'es2022',
    write: false,
  });
  const script = result.outputFiles[0]?.contents;
  if (!script) throw new Error('Missing progress browser fixture');
  server = createServer((request, response) => {
    const module = request.url === '/fixture.js';
    response.writeHead(200, { 'Content-Type': module ? 'application/javascript' : 'text/html' });
    response.end(
      module ? script : '<!doctype html><script type="module" src="/fixture.js"></script>',
    );
  });
  await new Promise<void>((resolve) => server.listen(0, '127.0.0.1', resolve));
  const listening = server.address();
  if (!listening || typeof listening === 'string')
    throw new Error('Missing progress fixture address');
  address = `http://127.0.0.1:${String(listening.port)}`;
  browser = await launchHeadlessBrowser(chromium);
}, 30_000);

afterAll(async () => {
  await closeBrowserFixture(browser, server);
}, 30_000);

/** Acquire native browser resources separately from observing progress presentation. */
beforeEach(async () => {
  pageReady = browser.newPage();
  await pageReady;
});

/** Retain custody of late page creation until its native resources can be released. */
afterEach(async () => {
  const pending = pageReady;
  pageReady = undefined;
  await (await pending)?.close();
});

test('disabling Substitute restores the native progress surface', async () => {
  const page = await pageReady;
  if (!page) throw new Error('Progress browser page is unavailable.');
  await page.goto(address);
  await page.waitForFunction(() => !!window.progressBrowserProof);
  expect(
    await page.locator('#native-progress').evaluate((element) => getComputedStyle(element).display),
  ).toBe('none');
  await page.evaluate(() => window.progressBrowserProof.mode(false));
  expect(
    await page.locator('#native-progress').evaluate((element) => getComputedStyle(element).display),
  ).toBe('block');
});

test('compact generation feedback replaces the collapsed overlay while queue history remains available', async () => {
  const page = await pageReady;
  if (!page) throw new Error('Progress browser page is unavailable.');
  await page.goto(address);
  await page.waitForFunction(() => !!window.progressBrowserProof);
  const overlay = page.locator('[data-testid="queue-progress-overlay"]');
  expect(await overlay.evaluate((element) => getComputedStyle(element).display)).toBe('none');
  await page.evaluate(() => window.progressBrowserProof.queueExpanded(true));
  expect(await overlay.evaluate((element) => getComputedStyle(element).display)).toBe('block');
  await page.evaluate(() => {
    window.progressBrowserProof.queueExpanded(false);
    window.progressBrowserProof.mode(false);
  });
  expect(await overlay.evaluate((element) => getComputedStyle(element).display)).toBe('block');
});

test('Substitute progress uses two independent three-pixel tracks at the menu edge', async () => {
  const page = await pageReady;
  if (!page) throw new Error('Progress browser page is unavailable.');
  await page.goto(address);
  await page.waitForFunction(() => !!window.progressBrowserProof);
  await page.evaluate(() => window.progressBrowserProof.update(true, 75, 40, true));
  const geometry = await page.locator('.sc-generation-progress').evaluate((root) => {
    const first = root.children[0],
      second = root.children[1];
    if (!(first instanceof HTMLElement) || !(second instanceof HTMLElement) || !root.parentElement)
      throw new Error('Missing progress tracks');
    return {
      frame: root.parentElement.getBoundingClientRect().toJSON(),
      first: first.getBoundingClientRect().toJSON(),
      second: second.getBoundingClientRect().toJSON(),
      workflow: getComputedStyle(first).backgroundColor,
      sampler: getComputedStyle(second).backgroundColor,
      background: getComputedStyle(root).backgroundColor,
    };
  });
  expect(geometry.first.height).toBe(3);
  expect(geometry.second.height).toBe(3);
  expect(geometry.first.bottom).toBe(geometry.frame.bottom);
  expect(geometry.second.top).toBe(geometry.frame.bottom);
  expect(geometry.first.width).toBeCloseTo(geometry.frame.width * 0.75, 0);
  expect(geometry.second.width).toBeCloseTo(geometry.frame.width * 0.4, 0);
  expect(geometry.workflow).toBe('rgb(182, 58, 203)');
  expect(geometry.sampler).toBe('rgb(245, 158, 11)');
  expect(geometry.background).toBe('rgba(0, 0, 0, 0)');
  await page.evaluate(() => window.progressBrowserProof.update(true, 20, 90, false));
  expect(
    await page
      .locator('.sc-generation-progress__sampler')
      .evaluate((bar) => getComputedStyle(bar).backgroundColor),
  ).toBe('rgb(197, 106, 0)');
  await page.evaluate(() => window.progressBrowserProof.update(false, 0, 0, false));
  await page.locator('.sc-generation-progress').waitFor({ state: 'hidden' });
});
