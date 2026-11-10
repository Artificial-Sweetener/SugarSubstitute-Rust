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
 * Prove CodeMirror places the browser caret on the row a soft-wrap selection names, including in a
 * browser whose adjacent row rectangles overlap by rounding noise. Firefox reports touching rows
 * that way beneath a fractional canvas scale; CodeMirror used to read the overlap as one visual row,
 * never asked the browser to move its caret to the row end, and the painted caret stayed at the start
 * of the next row.
 * @jest-environment node
 */
import type { Page } from '@playwright/test';
import {
  mountPrompt,
  startPromptBrowser,
  type PromptBrowserFixture,
} from '../support/PromptBrowserServer.js';

let fixture: PromptBrowserFixture;

const source =
  'a very long description of a cat sitting on a windowsill looking at the rain outside while the city lights blur into soft colors, (tail:1.20), and a second sentence that wraps as well';
const editorWidth = 320;

beforeAll(async () => {
  fixture = await startPromptBrowser();
}, 60_000);

afterAll(async () => {
  await fixture.close();
}, 30_000);

/** Wait for queued native measurement and the wrap publication it schedules to paint. */
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

/** Make every rectangle taller by `noise` pixels, as a browser whose row edges meet within rounding error. */
async function addRectangleNoise(page: Page, noise: number): Promise<void> {
  await page.evaluate((amount) => {
    const original = Range.prototype.getClientRects;
    Range.prototype.getClientRects = function getClientRects(this: Range): DOMRectList {
      const rects = [...original.call(this)].map(
        (rect) => new DOMRect(rect.x, rect.y, rect.width, rect.height + amount),
      );
      return Object.assign(rects, { item: (index: number) => rects[index] ?? null }) as DOMRectList;
    };
  }, noise);
}

/** Record every browser-native selection movement CodeMirror requests. */
async function recordSelectionMoves(page: Page): Promise<void> {
  await page.evaluate(() => {
    const moves: string[] = [];
    Reflect.set(window, 'selectionMoves', moves);
    const original = Selection.prototype.modify;
    Selection.prototype.modify = function modify(
      this: Selection,
      ...args: [string, string, string]
    ) {
      moves.push(args.join(' '));
      return original.apply(this, args);
    };
  });
}

test.each(
  (['nodes-1', 'nodes-2'] as const).flatMap((renderer) =>
    [0, 0.02].map((noise) => [renderer, noise] as const),
  ),
)(
  '%s with %s px of rectangle noise has the browser move its caret to the row end or row start named by the side',
  async (renderer, noise) => {
    const page = await mountPrompt(fixture, renderer, source, 1.09);
    try {
      const boundaries = await wrapBoundaries(page);
      await addRectangleNoise(page, noise);
      await recordSelectionMoves(page);
      await page.evaluate(() =>
        document.querySelector<HTMLElement>('[data-sc-prompt-editor] .cm-content')?.focus(),
      );
      for (const boundary of boundaries) {
        for (const [side, expected] of [
          [-1, 'move forward lineboundary'],
          [1, 'move backward lineboundary'],
        ] as const) {
          await page.evaluate(() => {
            (Reflect.get(window, 'selectionMoves') as string[]).length = 0;
          });
          await page.evaluate(
            ([position, wanted]) => {
              // Leave the boundary first so the placement below always changes the selection.
              window.promptBrowserProof.placeCaret(Math.max(position - 30, 0), 1);
              window.promptBrowserProof.placeCaret(position, wanted);
            },
            [boundary, side] as const,
          );
          await settle(page);
          const moves = await page.evaluate(
            () => Reflect.get(window, 'selectionMoves') as string[],
          );
          expect({ boundary, side, moves: moves.includes(expected) }).toEqual({
            boundary,
            side,
            moves: true,
          });
          expect(await page.evaluate(() => window.promptBrowserProof.selection())).toEqual({
            anchor: boundary,
            head: boundary,
          });
        }
      }
      expect(await page.evaluate(() => window.promptBrowserProof.read().errors)).toEqual([]);
    } finally {
      await page.close();
    }
  },
  90_000,
);
