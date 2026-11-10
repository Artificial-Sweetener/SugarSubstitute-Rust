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
 * Prove that a multi-paragraph prompt with emphasis weights keeps standard text-editor caret
 * semantics: a click on an empty line, or past a row's end, lands on that line and row, and Up and
 * Down step one visual row at a time, stopping on empty lines and keeping the horizontal column.
 * @jest-environment node
 */
import type { Page } from '@playwright/test';
import {
  mountPrompt,
  startPromptBrowser,
  type PromptBrowserFixture,
} from '../support/PromptBrowserServer.js';

let fixture: PromptBrowserFixture;

const paragraphs = [
  'best quality, score_7, masterpiece, very aesthetic, Tonade, nai3,',
  'portrait, upper body, straight-on, front lighting, (backlighting:-1.00),',
  '1girl, (mature female:1.50), arms at sides, standing, looking at viewer,',
  'skinny, flat chest, black (inverted cross:2.00) hair ornament, (floating hair:-1.50),',
  'pink hair, (long hair between eyes:2.00), (blunt bangs:-1.00), long hair, twintails, (long sidelocks:-1.00),',
];
const source = paragraphs.join('\n\n');
/** Logical source offset of each source line's first character; empty lines own exactly one offset. */
const lineStarts = source
  .split('\n')
  .map((_, index, lines) =>
    lines.slice(0, index).reduce((offset, line) => offset + line.length + 1, 0),
  );
const lineLengths = source.split('\n').map((line) => line.length);
/** Wide enough for the first paragraph on one row and narrow enough that every other paragraph wraps. */
const editorWidth = 400;
/** The visual rows each source line occupies at that width: paragraphs wrap, empty lines keep one row. */
const rowsPerLine = [1, 1, 2, 1, 2, 1, 2, 1, 2];
const lineOfRow = rowsPerLine.flatMap((rows, line) => Array.from({ length: rows }, () => line));

beforeAll(async () => {
  fixture = await startPromptBrowser();
}, 60_000);

afterAll(async () => {
  await fixture.close();
}, 30_000);

/** Wait until queued native measurement and the wrap publication it schedules have painted. */
async function settle(page: Page): Promise<void> {
  await page.evaluate(
    () =>
      new Promise<void>((resolve) =>
        requestAnimationFrame(() =>
          requestAnimationFrame(() => requestAnimationFrame(() => setTimeout(resolve, 30))),
        ),
      ),
  );
}

interface LineBox {
  readonly top: number;
  readonly bottom: number;
  readonly left: number;
  readonly right: number;
}

/** Measure every painted source line, including empty ones. */
async function lineBoxes(page: Page): Promise<LineBox[]> {
  return page.evaluate(() =>
    [...document.querySelectorAll('.cm-content .cm-line')].map((line) => {
      const rect = line.getBoundingClientRect();
      return { top: rect.top, bottom: rect.bottom, left: rect.left, right: rect.right };
    }),
  );
}

/** Narrow the editor like the reported card and wait until every paragraph has its wrapped rows. */
async function wrapLikeTheRecording(page: Page): Promise<LineBox[]> {
  await page.evaluate((width) => {
    const style = document.createElement('style');
    style.textContent = `[data-sc-prompt-editor]{width:${String(width)}px!important;max-width:${String(width)}px!important}`;
    document.head.append(style);
  }, editorWidth);
  for (let attempt = 0; attempt < 20; attempt++) {
    await settle(page);
    const boxes = await lineBoxes(page);
    const rows = boxes.map((box) => Math.round((box.bottom - box.top) / 19));
    if (JSON.stringify(rows) === JSON.stringify(rowsPerLine)) return boxes;
  }
  throw new Error(`The prompt never wrapped into ${JSON.stringify(rowsPerLine)} rows`);
}

/** Assert one value while naming where it was observed, because Jest's `expect` takes no message. */
function expectAt<T>(at: string, actual: T, expected: T): void {
  expect({ at, actual }).toEqual({ at, actual: expected });
}

/** The logical offset of the editor's main selection head and its source line. */
async function caret(page: Page): Promise<{ head: number; line: number; column: number }> {
  const selection = await page.evaluate(() => window.promptBrowserProof.selection());
  if (!selection || selection.anchor !== selection.head)
    throw new Error(`Expected a collapsed caret, got ${JSON.stringify(selection)}`);
  let line = 0;
  lineStarts.forEach((start, index) => {
    if (start <= selection.head) line = index;
  });
  return { head: selection.head, line, column: selection.head - (lineStarts[line] ?? 0) };
}

/** The painted native caret's horizontal position, or null where the browser paints none. */
async function paintedCaretX(page: Page): Promise<number | null> {
  return page.evaluate(() => {
    const selection = getSelection();
    if (!selection?.rangeCount) return null;
    const rect = selection.getRangeAt(0).getBoundingClientRect();
    return rect.height > 0 ? rect.left : null;
  });
}

/** Painted visual rows of one decorated source line, measured from its rendered characters. */
async function visualRows(page: Page, line: number): Promise<LineBox[]> {
  return page.evaluate((index) => {
    const element = document.querySelectorAll('.cm-content .cm-line')[index];
    if (!element) throw new Error('Missing prompt line');
    const rows: { top: number; bottom: number; left: number; right: number }[] = [];
    const walker = document.createTreeWalker(element, NodeFilter.SHOW_TEXT);
    for (let node = walker.nextNode(); node; node = walker.nextNode()) {
      const text = node.textContent ?? '';
      for (let at = 0; at < text.length; at++) {
        const range = document.createRange();
        range.setStart(node, at);
        range.setEnd(node, at + 1);
        const rect = range.getBoundingClientRect();
        if (!rect.width) continue;
        const row = rows.at(-1);
        if (!row || rect.top >= row.bottom - 2)
          rows.push({ top: rect.top, bottom: rect.bottom, left: rect.left, right: rect.right });
        else row.right = Math.max(row.right, rect.right);
      }
    }
    return rows;
  }, line);
}

test.each([
  ['nodes-1', 1.09],
  ['nodes-2', 1.09],
] as const)(
  '%s at scale %d puts the caret on a clicked empty line and at the end of a clicked row',
  async (renderer, scale) => {
    const page = await mountPrompt(fixture, renderer, source, scale);
    try {
      const boxes = await wrapLikeTheRecording(page);
      for (const [line, box] of boxes.entries()) {
        const middle = (box.top + box.bottom) / 2;
        if (lineLengths[line] === 0) {
          // Left, middle and right of an empty line all belong to that line.
          for (const x of [box.left + 4, (box.left + box.right) / 2, box.right - 4]) {
            await page.mouse.click(x, middle);
            await settle(page);
            const placed = await caret(page);
            expectAt(
              `empty line ${String(line)} at x=${String(Math.round(x))}`,
              placed.head,
              lineStarts[line] ?? -1,
            );
          }
          continue;
        }
        // Past the end of every visual row the caret stays on that row; the last row ends the line.
        const rows = await visualRows(page, line);
        expect(rows.length).toBe(rowsPerLine[line]);
        for (const [index, row] of rows.entries()) {
          await page.mouse.click(
            Math.min(row.right + 24, box.right - 2),
            (row.top + row.bottom) / 2,
          );
          await settle(page);
          const placed = await caret(page);
          expectAt(`row ${String(index)} of line ${String(line)}`, placed.line, line);
          const painted = await page.evaluate(() => {
            const rect = getSelection()?.getRangeAt(0).getBoundingClientRect();
            return rect ? (rect.top + rect.bottom) / 2 : null;
          });
          if (painted === null) throw new Error('The collapsed caret is not painted');
          expect(painted).toBeGreaterThan(row.top - 2);
          expect(painted).toBeLessThan(row.bottom + 2);
          if (index === rows.length - 1) expect(placed.column).toBe(lineLengths[line]);
          else expect(placed.column).toBeLessThan(lineLengths[line] ?? 0);
        }
      }
      expect(await page.evaluate(() => window.promptBrowserProof.read().errors)).toEqual([]);
    } finally {
      await page.close();
    }
  },
  90_000,
);

test.each([
  ['nodes-1', 1.09],
  ['nodes-2', 1.09],
] as const)(
  '%s at scale %d moves Up and Down one visual row at a time, stops on empty lines and keeps its column',
  async (renderer, scale) => {
    const page = await mountPrompt(fixture, renderer, source, scale);
    try {
      const boxes = await wrapLikeTheRecording(page);
      const [first, last] = [boxes[0], boxes.at(-1)] as [LineBox, LineBox];
      // From the document start every Down press lands on the next visual row, empty rows included.
      await page.mouse.click(first.left + 2, (first.top + first.bottom) / 2);
      await settle(page);
      for (let row = 0; row < lineOfRow.length; row++) {
        expectAt(`Down row ${String(row)}`, (await caret(page)).line, lineOfRow[row] ?? -1);
        await page.keyboard.press('ArrowDown');
        await settle(page);
      }
      // From the document end every Up press does the same in reverse.
      await page.mouse.click(last.right - 2, (last.top + last.bottom) / 2 + 9);
      await settle(page);
      for (let row = lineOfRow.length - 1; row >= 0; row--) {
        expectAt(`Up row ${String(row)}`, (await caret(page)).line, lineOfRow[row] ?? -1);
        await page.keyboard.press('ArrowUp');
        await settle(page);
      }
      // A column chosen inside the second paragraph survives a trip over the empty line and the short
      // first paragraph and back: the caret returns to the same offset and the same painted column.
      const row = (await visualRows(page, 2))[0];
      if (!row) throw new Error('Missing wrapped paragraph row');
      await page.mouse.click(row.left + 120, (row.top + row.bottom) / 2);
      await settle(page);
      const start = await caret(page);
      const startX = await paintedCaretX(page);
      expect(start.line).toBe(2);
      expect(startX).not.toBeNull();
      const visited: number[] = [];
      for (const key of ['ArrowUp', 'ArrowUp', 'ArrowDown', 'ArrowDown']) {
        await page.keyboard.press(key);
        await settle(page);
        visited.push((await caret(page)).line);
      }
      expect(visited).toEqual([1, 0, 1, 2]);
      expect((await caret(page)).head).toBe(start.head);
      expect(
        Math.abs((await paintedCaretX(page)) ?? Number.NaN) - Math.abs(startX ?? 0),
      ).toBeLessThan(2);
      expect(await page.evaluate(() => window.promptBrowserProof.read().errors)).toEqual([]);
    } finally {
      await page.close();
    }
  },
  90_000,
);
