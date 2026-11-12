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
 * Pin the generation actions, history button included, to Substitute's own painting: the segments sit transparent on the
 * accent, hover adds white alpha 30 and pressed adds black alpha 32 (titlebar_buttons.py,
 * GenerationTitleBarSegmentButton._apply_theme_palette), in both themes and for every accent.
 */
import { readFileSync } from 'node:fs';
import { chromium, type Browser, type Page } from '@playwright/test';
import { build } from 'esbuild';
import UPNG from 'upng-js';
import { launchHeadlessBrowser } from '../../frontend_core/support/BrowserFixtureLifetime.js';

type Mode = 'light' | 'dark';
type Rgb = readonly [number, number, number];

/** One reference row: the seed, the mode and the PRIMARY shade qfluentwidgets derives for it. */
interface Accent {
  readonly seed: string;
  readonly mode: Mode;
  readonly primary: string;
}

const vectors = readFileSync(new URL('../support/accent-vectors.txt', import.meta.url), 'utf8')
  .split('\n')
  .filter((line) => line.trim() && !line.startsWith('#'))
  .map((line): Accent => {
    const [seed, mode, primary] = line.trim().split(/\s+/);
    return { seed: `#${seed}`, mode: mode as Mode, primary: `#${primary}` };
  });

/** The Windows accent of the proof machine and its PRIMARY shade in each mode. */
const windowsAccent: readonly Accent[] = [
  { seed: '#c30052', mode: 'light', primary: '#c30052' },
  { seed: '#c30052', mode: 'dark', primary: '#ff2983' },
];

function rgb(hex: string): Rgb {
  return [1, 3, 5].map((index) => parseInt(hex.slice(index, index + 2), 16)) as unknown as Rgb;
}

/** Qt's source-over of an 8-bit-alpha ink on an opaque fill, rounded to the nearest level. */
function over(ink: number, alpha: number, fill: Rgb): Rgb {
  return fill.map((channel) =>
    Math.round((ink * alpha + channel * (255 - alpha)) / 255),
  ) as unknown as Rgb;
}

describe('Chromium generation action overlays', () => {
  let browser: Browser;
  let page: Page;

  beforeAll(async () => {
    browser = await launchHeadlessBrowser(chromium);
    const output = await build({
      stdin: {
        contents: `
        import {generationStripCss} from './frontend/comfyui/ui/surface/frame/GenerationStripStyles.ts';
        import {accentThemeCss} from './frontend/comfyui/ui/surface/appearance/AccentThemeStyles.ts';
        const style = document.createElement('style');
        style.textContent = generationStripCss;
        document.head.append(style);
        const accent = document.createElement('style');
        document.head.append(accent);
        globalThis.useAccent = (seed, dark) => {
          accent.textContent = accentThemeCss(seed, dark);
          document.querySelector('.sc-generation-strip').dataset.scColorScheme = dark ? 'dark' : 'light';
        };
        globalThis.accentInstalled = true;
      `,
        resolveDir: process.cwd(),
      },
      bundle: true,
      write: false,
      format: 'esm',
    });
    page = await browser.newPage({ viewport: { width: 420, height: 60 } });
    await page.setContent(`<!doctype html><html data-sc-substitute-frame><body style="margin:0;background:#202020">
      <div class="sc-generation-strip" style="position:absolute;left:0;top:0;right:auto">
        <div class="sc-generation-batch"></div>
        <div class="sc-generation-actions">
          <button class="sc-generation-action" data-action="cancel" aria-label="Cancel generation"><span class="sc-generation-glyph"></span></button>
          <button class="sc-generation-action" data-action="generate" aria-label="Generate"><span class="sc-generation-glyph"></span></button>
          <button class="sc-generation-action" data-action="skip" aria-label="Skip generation"><span class="sc-generation-glyph"></span></button>
          <span class="sc-generation-queue-slot"></span>
        </div>
      </div>
      <button data-sc-generation-queue aria-label="Generation queue" style="--sc-queue-left:186px;--sc-queue-top:0px"><span class="sc-generation-queue-glyph"></span></button>
    </body></html>`);
    await page.addScriptTag({ content: output.outputFiles[0]!.text, type: 'module' });
    await page.waitForFunction(() => Reflect.get(globalThis, 'accentInstalled') === true);
  });

  afterAll(async () => {
    await page?.close();
    await browser?.close();
  });

  const selectors = [
    ...['cancel', 'generate', 'skip'].map((name) => `[data-action="${name}"]`),
    'button[data-sc-generation-queue]',
  ];

  /** Read the pixel the user sees at the top-left of one action, clear of dividers and glyphs. */
  async function paintedFill(selector: string): Promise<Rgb> {
    const box = await page.locator(selector).boundingBox();
    if (!box) throw new Error(`${selector} has no painted bounds.`);
    const png = await page.screenshot({
      clip: { x: Math.floor(box.x) + 2, y: Math.floor(box.y) + 2, width: 3, height: 3 },
    });
    const image = UPNG.decode(Uint8Array.from(png).buffer);
    return [...new Uint8Array(UPNG.toRGBA8(image)[0]!)].slice(0, 3) as unknown as Rgb;
  }

  function withinOne(actual: Rgb, expected: Rgb): void {
    for (const [channel, value] of actual.entries())
      expect(Math.abs(value - expected[channel]!)).toBeLessThanOrEqual(1);
  }

  test.each([...windowsAccent, ...vectors])(
    'rests on PRIMARY, hovers with white alpha 30 and presses with black alpha 32 for $seed in $mode mode',
    async (row) => {
      await page.evaluate(
        ([seed, mode]) => {
          Reflect.get(globalThis, 'useAccent')(seed, mode === 'dark');
        },
        [row.seed, row.mode] as const,
      );
      const primary = rgb(row.primary);
      await page.mouse.move(400, 50);
      for (const selector of selectors) {
        expect(await paintedFill(selector)).toEqual(primary);
        await page.locator(selector).hover();
        withinOne(await paintedFill(selector), over(255, 30, primary));
        await page.mouse.down();
        withinOne(await paintedFill(selector), over(0, 32, primary));
        await page.mouse.up();
        await page.mouse.move(400, 50);
      }
    },
  );

  test.each([
    ['dark', 'rgb(0, 0, 0)'],
    ['light', 'rgb(255, 255, 255)'],
  ] as const)(
    'draws %s-mode glyphs in one colour through rest, hover and pressed',
    async (mode, glyph) => {
      await page.evaluate(
        (dark) => Reflect.get(globalThis, 'useAccent')('#c30052', dark),
        mode === 'dark',
      );
      await page.mouse.move(400, 50);
      for (const selector of selectors) {
        const colour = (): Promise<string> =>
          page.locator(selector).evaluate((element) => getComputedStyle(element).color);
        expect(await colour()).toBe(glyph);
        await page.locator(selector).hover();
        expect(await colour()).toBe(glyph);
        await page.mouse.down();
        expect(await colour()).toBe(glyph);
        await page.mouse.up();
        await page.mouse.move(400, 50);
      }
    },
  );
});
