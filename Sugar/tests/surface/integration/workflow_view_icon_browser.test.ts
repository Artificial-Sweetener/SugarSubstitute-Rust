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
/**
 * In a real browser, with the real button styles, the graph/masonry icon never paints outside its
 * 20 px box: at the graph end its last node and the link reaching for it are cut at the right
 * edge, as approved, and so is every frame of the way to the masonry.
 */
import { createServer, type Server } from 'node:http';
import { build } from 'esbuild';
import { chromium, type Browser, type Page } from '@playwright/test';
import {
  closeBrowserFixture,
  launchHeadlessBrowser,
} from '../../frontend_core/support/BrowserFixtureLifetime.js';
import type {} from '../support/WorkflowViewIconBrowser.js';

let server: Server;
let browser: Browser;
let page: Page;
const SCALE = 4;

beforeAll(async () => {
  const result = await build({
    entryPoints: ['tests/surface/support/WorkflowViewIconBrowser.ts'],
    bundle: true,
    format: 'esm',
    platform: 'browser',
    target: 'es2022',
    write: false,
    loader: { '.svg': 'dataurl' },
  });
  const script = result.outputFiles[0]?.contents;
  if (!script) throw new Error('Missing icon fixture');
  server = createServer((request, response) => {
    const module = request.url === '/fixture.js';
    response.writeHead(200, { 'Content-Type': module ? 'application/javascript' : 'text/html' });
    response.end(
      module ? script : '<!doctype html><script type="module" src="/fixture.js"></script>',
    );
  });
  await new Promise<void>((resolve) => server.listen(0, '127.0.0.1', resolve));
  const listening = server.address();
  if (!listening || typeof listening === 'string') throw new Error('Missing fixture address');
  browser = await launchHeadlessBrowser(chromium);
  page = await browser.newPage({
    viewport: { width: 120, height: 120 },
    deviceScaleFactor: SCALE,
  });
  await page.goto(`http://127.0.0.1:${String(listening.port)}`);
  await page.waitForFunction(() => !!window.workflowViewIconProof);
});

afterAll(async () => {
  await page.close();
  await closeBrowserFixture(browser, server);
});

/** The bounding box of every dark pixel on the stage, in device pixels. */
async function ink(): Promise<{ left: number; top: number; right: number; bottom: number }> {
  const shot = await page.locator('#stage').screenshot();
  return page.evaluate(async (data) => {
    const image = await createImageBitmap(
      new Blob([Uint8Array.from(atob(data), (character) => character.charCodeAt(0))]),
    );
    const canvas = new OffscreenCanvas(image.width, image.height);
    const context = canvas.getContext('2d');
    if (!context) throw new Error('No 2D context');
    context.drawImage(image, 0, 0);
    const pixels = context.getImageData(0, 0, image.width, image.height).data;
    let [left, top, right, bottom] = [Infinity, Infinity, -1, -1];
    for (let y = 0; y < image.height; y++)
      for (let x = 0; x < image.width; x++)
        if ((pixels[(y * image.width + x) * 4] ?? 255) < 200) {
          left = Math.min(left, x);
          top = Math.min(top, y);
          right = Math.max(right, x);
          bottom = Math.max(bottom, y);
        }
    return { left, top, right, bottom };
  }, shot.toString('base64'));
}

test('the icon is cut off at its own box in the graph state, the masonry state and every frame between', async () => {
  const box = await page.evaluate(() => window.workflowViewIconProof.box());
  expect([box.width, box.height]).toEqual([20, 20]);
  const edge = {
    left: box.left * SCALE,
    top: box.top * SCALE,
    right: (box.left + box.width) * SCALE - 1,
    bottom: (box.top + box.height) * SCALE - 1,
  };
  const FRAMES = 48;
  for (let frame = 0; frame <= FRAMES; frame++) {
    const position = frame / FRAMES;
    await page.evaluate((at) => window.workflowViewIconProof.pose(at), position);
    const drawn = await ink();
    // Nothing of the icon is painted outside its box, at any frame.
    expect(drawn.left).toBeGreaterThanOrEqual(edge.left);
    expect(drawn.top).toBeGreaterThanOrEqual(edge.top);
    expect(drawn.right).toBeLessThanOrEqual(edge.right);
    expect(drawn.bottom).toBeLessThanOrEqual(edge.bottom);
    // The graph: the link to the fourth node runs into the right edge, where it is cut.
    if (frame === 0) expect(drawn.right).toBe(edge.right);
    // The masonry: four tiles inside the box, three units clear of its right edge.
    if (frame === FRAMES) expect(drawn.right).toBeLessThan(edge.right - 2 * SCALE);
  }
});
