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
 * In a real browser, the finder's current match in a text box is painted where, and as, the box's
 * own selection of the same characters is painted: on one line, in wrapped text and in scrolled
 * text. jsdom lays nothing out, so only a browser can show this.
 */
import { createServer, type Server } from 'node:http';
import { build } from 'esbuild';
import { chromium, type Browser, type Page } from '@playwright/test';
import {
  closeBrowserFixture,
  launchHeadlessBrowser,
} from '../../frontend_core/support/BrowserFixtureLifetime.js';
import type {} from '../support/FinderHighlightBrowser.js';
import { PROOF_MATCH, PROOF_SELECTION } from '../support/FinderHighlightColours.js';

let server: Server;
let browser: Browser;
let page: Page;

beforeAll(async () => {
  const result = await build({
    entryPoints: ['tests/masonry_search/support/FinderHighlightBrowser.ts'],
    bundle: true,
    format: 'esm',
    platform: 'browser',
    target: 'es2022',
    write: false,
    loader: { '.svg': 'dataurl' },
  });
  const script = result.outputFiles[0]?.contents;
  if (!script) throw new Error('Missing finder highlight fixture');
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
  page = await browser.newPage({ viewport: { width: 480, height: 520 }, deviceScaleFactor: 2 });
  await page.goto(`http://127.0.0.1:${String(listening.port)}`);
  await page.waitForFunction(() => !!window.finderHighlightProof);
});

afterAll(async () => {
  await page.close();
  await closeBrowserFixture(browser, server);
});

interface Painted {
  left: number;
  top: number;
  width: number;
  height: number;
  pixels: number;
}

/** The bounding box, in device pixels, of everything painted in `colour` inside one row. */
async function painted(row: string, colour: string): Promise<Painted | null> {
  const shot = await page.locator(`#row-${row}`).screenshot();
  return page.evaluate(
    async ({ data, colour: wanted }) => {
      const channels = (/\d+, \d+, \d+/.exec(wanted)?.[0] ?? '').split(', ').map(Number);
      const image = await createImageBitmap(
        new Blob([Uint8Array.from(atob(data), (character) => character.charCodeAt(0))]),
      );
      const canvas = new OffscreenCanvas(image.width, image.height);
      const context = canvas.getContext('2d');
      if (!context) throw new Error('No 2D context');
      context.drawImage(image, 0, 0);
      const pixels = context.getImageData(0, 0, image.width, image.height).data;
      let [left, top, right, bottom, count] = [Infinity, Infinity, -1, -1, 0];
      for (let y = 0; y < image.height; y++)
        for (let x = 0; x < image.width; x++) {
          const at = (y * image.width + x) * 4;
          if (channels.some((channel, index) => Math.abs((pixels[at + index] ?? 0) - channel) > 2))
            continue;
          left = Math.min(left, x);
          top = Math.min(top, y);
          right = Math.max(right, x);
          bottom = Math.max(bottom, y);
          count++;
        }
      return count
        ? { left, top, width: right - left + 1, height: bottom - top + 1, pixels: count }
        : null;
    },
    { data: shot.toString('base64'), colour },
  );
}

for (const [row, ordinal, about] of [
  ['line', 0, 'on one line'],
  ['line', 1, 'on one line, scrolled sideways to a match past the box’s edge'],
  ['wrapped', 1, 'in wrapped text, on a later row'],
  ['scrolled', 12, 'in text scrolled down to a match below the box'],
] as const)
  test(`the current match is painted exactly as the box’s own selection ${about}`, async () => {
    const matches = await page.evaluate(
      ([field, index]) => window.finderHighlightProof.highlight('sunset', field, index),
      [row, ordinal] as const,
    );
    expect(matches).toBeGreaterThan(ordinal);
    const highlighted = await painted(row, PROOF_SELECTION);
    // Its other matches are in the match colour, and none of those is the current one.
    expect(await painted(row, PROOF_MATCH)).not.toBeNull();
    await page.evaluate(
      ([field, index]) => window.finderHighlightProof.select('sunset', field, index),
      [row, ordinal] as const,
    );
    const selected = await painted(row, PROOF_SELECTION);
    expect(await painted(row, PROOF_MATCH)).toBeNull();
    await page.evaluate(() => window.finderHighlightProof.clear());
    if (!highlighted || !selected) throw new Error('Nothing was painted');
    // The same box to the device pixel: the highlight stands on the selected glyphs. Inside it
    // only the glyphs' antialiased edges may differ, by where the text sits within a pixel.
    const box = ({ left, top, width, height }: Painted): number[] => [left, top, width, height];
    expect(box(highlighted)).toEqual(box(selected));
    expect(Math.abs(highlighted.pixels - selected.pixels)).toBeLessThan(selected.pixels / 50);
    expect(await painted(row, PROOF_SELECTION)).toBeNull();
  });
