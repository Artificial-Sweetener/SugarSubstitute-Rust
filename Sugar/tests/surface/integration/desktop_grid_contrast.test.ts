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
/** Prove Comfy's grid stays clearly visible on the single editor-panel wash in both themes. */
import { chromium, type Browser, type Page } from '@playwright/test';
import { build } from 'esbuild';
import UPNG from 'upng-js';
import {
  BROWSER_FIXTURE_SETUP_TIMEOUT_MS,
  launchHeadlessBrowser,
} from '../../frontend_core/support/BrowserFixtureLifetime.js';

type Theme = 'dark' | 'light';

/**
 * The grid tile each shipped Comfy palette assigns to LiteGraph (frontend 1.47.11, measured on the
 * decoded 100x100 tiles): a dominant field, 1px minor lines every ten pixels and 1px major lines on
 * the tile edge. The palettes disagree on polarity: dark lines are darker than their field, light
 * lines are lighter than theirs.
 */
const TILES: Record<Theme, { field: number; minor: number; major: number }> = {
  dark: { field: 34, minor: 25, major: 20 },
  light: { field: 221, minor: 230, major: 235 },
};

/** Real Mica behind the washes: the DWM material tone for each theme. */
const MICA: Record<Theme, number> = { dark: 32, light: 243 };

function tilePixels({
  field,
  minor,
  major,
}: {
  field: number;
  minor: number;
  major: number;
}): number[] {
  const pixels: number[] = [];
  for (let y = 0; y < 100; y += 1)
    for (let x = 0; x < 100; x += 1) {
      const value = x === 0 || y === 0 ? major : x % 10 === 0 || y % 10 === 0 ? minor : field;
      pixels.push(value, value, value, 255);
    }
  return pixels;
}

describe('Chromium desktop graph grid', () => {
  let browser: Browser;
  let page: Page;
  let install: string;

  beforeAll(async () => {
    browser = await launchHeadlessBrowser(chromium);
    const output = await build({
      stdin: {
        contents: `
        import {translucentGridPixels} from './frontend/comfyui/ui/surface/frame/DesktopGraphBackdrop.ts';
        import {desktopMaterialWash} from './frontend/comfyui/ui/surface/frame/DesktopMaterialWash.ts';
        /** Draw the tile exactly as LiteGraph does: a repeating pattern at a zoom-dependent alpha. */
        globalThis.paintGrid = (tile, wash, scale) => {
          const body = desktopMaterialWash(wash === 'dark').body;
          const canvas = document.querySelector('canvas');
          canvas.style.background = body;
          const source = document.createElement('canvas');
          source.width = 100; source.height = 100;
          const sourceContext = source.getContext('2d');
          const image = sourceContext.createImageData(100, 100);
          image.data.set(translucentGridPixels(new Uint8ClampedArray(tile), wash === 'dark'));
          sourceContext.putImageData(image, 0, 0);
          const context = canvas.getContext('2d');
          context.clearRect(0, 0, 200, 200);
          context.globalAlpha = (1 - 0.5 / scale);
          context.fillStyle = context.createPattern(source, 'repeat');
          context.fillRect(0, 0, 200, 200);
        };
        globalThis.gridInstalled = true;
      `,
        resolveDir: process.cwd(),
      },
      bundle: true,
      write: false,
      format: 'esm',
    });
    install = output.outputFiles[0]!.text;
    page = await browser.newPage({ viewport: { width: 220, height: 220 } });
    await page.addScriptTag({ content: install, type: 'module' });
    await page.waitForFunction(() => Reflect.get(globalThis, 'gridInstalled') === true);
  }, BROWSER_FIXTURE_SETUP_TIMEOUT_MS);

  afterAll(async () => {
    await page?.close();
    await browser?.close();
  });

  async function sample(theme: Theme, tile: Theme, scale: number): Promise<Record<string, number>> {
    await page.setContent(
      `<!doctype html><body style="margin:0;background:rgb(${String(MICA[theme])},${String(MICA[theme])},${String(MICA[theme])})"><canvas width="200" height="200" style="position:absolute;left:10px;top:10px"></canvas></body>`,
    );
    await page.addScriptTag({ content: install, type: 'module' });
    await page.waitForFunction(() => Reflect.get(globalThis, 'gridInstalled') === true);
    await page.evaluate(
      ([pixels, wash, zoom]) => Reflect.get(globalThis, 'paintGrid')(pixels, wash, zoom),
      [tilePixels(TILES[tile]), theme, scale] as const,
    );
    const png = await page.screenshot({ clip: { x: 10, y: 10, width: 100, height: 100 } });
    const image = UPNG.decode(Uint8Array.from(png).buffer);
    const rgba = new Uint8Array(UPNG.toRGBA8(image)[0]!);
    const at = (x: number, y: number): number => rgba[(y * 100 + x) * 4]!;
    return { field: at(5, 5), minor: at(10, 5), major: at(0, 5), minorCross: at(10, 10) };
  }

  test.each(['dark', 'light'] as const)(
    '%s theme keeps the grid clearly visible at the default zoom, whichever palette tile is current',
    async (theme) => {
      for (const tile of ['dark', 'light'] as const) {
        const { field, minor, major } = await sample(theme, tile, 1);
        const sign = theme === 'dark' ? 1 : -1;
        expect(sign * (minor - field)).toBeGreaterThanOrEqual(20);
        expect(sign * (major - field)).toBeGreaterThanOrEqual(30);
        expect(Math.abs(major - field)).toBeGreaterThan(Math.abs(minor - field));
      }
    },
  );

  test.each(['dark', 'light'] as const)(
    '%s theme grid remains visible at the zoom a fresh graph opens at',
    async (theme) => {
      const { field, minor } = await sample(theme, theme, 0.9);
      expect(Math.abs(minor - field)).toBeGreaterThanOrEqual(18);
    },
  );

  test.each(['dark', 'light'] as const)('%s theme paints no field over the wash', async (theme) => {
    const { field } = await sample(theme, theme, 1);
    const wash =
      theme === 'dark' ? { value: 32, alpha: 150 / 255 } : { value: 251, alpha: 188 / 255 };
    const expected = wash.value * wash.alpha + MICA[theme] * (1 - wash.alpha);
    expect(Math.abs(field - expected)).toBeLessThanOrEqual(1);
  });
});
