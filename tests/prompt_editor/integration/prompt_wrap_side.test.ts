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
 * Prove the prompt draws its caret on the visual row a soft-wrap selection's side names, at whole and
 * fractional canvas scales, whether the caret arrives from elsewhere or only changes side, and that the
 * browser's own caret is never painted.
 * @jest-environment node
 */
import type { Page } from '@playwright/test';
import {
  mountPrompt,
  startPromptBrowser,
  type PromptBrowserFixture,
} from '../support/PromptBrowserServer.js';
import { drawnCaret } from '../support/DrawnCaret.js';
import { BROWSER_FIXTURE_SETUP_TIMEOUT_MS } from '../../frontend_core/support/BrowserFixtureLifetime.js';

let fixture: PromptBrowserFixture;

const source =
  'a very long description of a cat, sitting on a windowsill, looking at the rain outside, while the city lights blur into soft colors, (tail:1.20), and a second sentence that wraps as well';
const editorWidth = 320;

beforeAll(async () => {
  fixture = await startPromptBrowser();
}, BROWSER_FIXTURE_SETUP_TIMEOUT_MS);

afterAll(async () => {
  await fixture.close();
}, 30_000);

/** Wait for queued native measurement, the wrap publication it schedules and the drawn layers to paint. */
async function settle(page: Page): Promise<void> {
  await page.evaluate(
    () =>
      new Promise<void>((resolve) =>
        requestAnimationFrame(() =>
          requestAnimationFrame(() => requestAnimationFrame(() => setTimeout(resolve, 40))),
        ),
      ),
  );
}

/** Narrow the editor until the paragraph wraps and return its soft-wrap offsets. */
async function wrapBoundaries(page: Page): Promise<number[]> {
  await page.evaluate((width) => {
    const style = document.createElement('style');
    style.textContent = `[data-sc-prompt-editor]{width:${String(width)}px!important;max-width:${String(width)}px!important}`;
    document.head.append(style);
  }, editorWidth);
  for (let attempt = 0; attempt < 20; attempt++) {
    await settle(page);
    const boundaries = await page.evaluate(() => [...window.promptBrowserProof.wrapBoundaries()]);
    if (boundaries.length >= 2) return boundaries;
  }
  throw new Error('The prompt never wrapped');
}

/** How the caret reaches the wrap offset: from elsewhere in the prompt, or by changing only its side there. */
type Arrival = 'from elsewhere' | 'side only';

/** Place the caret on one side of a wrap offset after arriving the given way. */
async function placeOnSide(
  page: Page,
  boundary: number,
  side: -1 | 1,
  arrival: Arrival,
): Promise<void> {
  await page.evaluate(
    ([position, wanted, sideOnly]) => {
      if (sideOnly) window.promptBrowserProof.placeCaret(position, wanted === 1 ? -1 : 1);
      else window.promptBrowserProof.placeCaret(Math.max(position - 30, 0), 1);
    },
    [boundary, side, arrival === 'side only'] as const,
  );
  await settle(page);
  await page.evaluate(
    ([position, wanted]) => window.promptBrowserProof.placeCaret(position, wanted),
    [boundary, side] as const,
  );
  await settle(page);
}

test.each(
  (['nodes-1', 'nodes-2'] as const).flatMap((renderer) =>
    [1.09, 0.8].map((scale) => [renderer, scale] as const),
  ),
)(
  '%s at scale %d draws the caret on the row each side of every wrap offset names',
  async (renderer, scale) => {
    const page = await mountPrompt(fixture, renderer, source, scale);
    try {
      const boundaries = await wrapBoundaries(page);
      await page.evaluate(() =>
        document.querySelector<HTMLElement>('[data-sc-prompt-editor] .cm-content')?.focus(),
      );
      for (const boundary of boundaries) {
        // The upper row ends with the character before the offset; the lower row starts with the one at it.
        const rows = await page.evaluate(
          (position) => ({
            upper: window.promptBrowserProof.characterRow(position - 1),
            lower: window.promptBrowserProof.characterRow(position),
          }),
          boundary,
        );
        if (!rows.upper || !rows.lower) throw new Error(`No characters around ${String(boundary)}`);
        for (const [side, arrival] of [
          [-1, 'from elsewhere'],
          [1, 'from elsewhere'],
          [-1, 'side only'],
          [1, 'side only'],
        ] as const) {
          await placeOnSide(page, boundary, side, arrival);
          const row = side < 0 ? rows.upper : rows.lower;
          const caret = await drawnCaret(page);
          const middle = caret ? (caret.top + caret.bottom) / 2 : Number.NaN;
          expect({
            boundary,
            side,
            arrival,
            onRow: middle >= row.top && middle <= row.bottom,
          }).toEqual({ boundary, side, arrival, onRow: true });
          expect(await page.evaluate(() => window.promptBrowserProof.selection())).toEqual({
            anchor: boundary,
            head: boundary,
          });
        }
      }
      // Only the drawn caret is painted: the browser's own caret is transparent.
      expect(
        await page.evaluate(() => {
          const line = document.querySelector('[data-sc-prompt-editor] .cm-line');
          if (!line) throw new Error('Missing prompt line');
          return getComputedStyle(line).caretColor;
        }),
      ).toBe('rgba(0, 0, 0, 0)');
      expect(await page.evaluate(() => window.promptBrowserProof.read().errors)).toEqual([]);
    } finally {
      await page.close();
    }
  },
  90_000,
);
