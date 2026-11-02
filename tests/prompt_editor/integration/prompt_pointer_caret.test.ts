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
 * Prove that real mouse clicks place the caret on the clicked visual row and column.
 * @jest-environment node
 */
import type { Page } from '@playwright/test';
import {
  expectSource,
  mountPrompt,
  startPromptBrowser,
  type PromptBrowserFixture,
} from '../support/PromptBrowserServer.js';

let fixture: PromptBrowserFixture;
const wrapped =
  'a very long description of a cat sitting on a windowsill looking at the rain outside while the city lights blur into soft colors, tail';

beforeAll(async () => {
  fixture = await startPromptBrowser();
}, 60_000);

afterAll(async () => {
  await fixture.close();
}, 30_000);

interface VisualRow {
  readonly top: number;
  readonly bottom: number;
  readonly left: number;
  readonly right: number;
  /** Logical source offset of the first character painted on the row. */
  readonly from: number;
}

/** Measure the painted rows of one plain prompt line from its rendered characters. */
async function visualRows(page: Page, line: number): Promise<VisualRow[]> {
  return page.evaluate((index) => {
    const element = document.querySelectorAll('.cm-content .cm-line')[index];
    if (!element) throw new Error('Missing prompt line');
    const rows: { top: number; bottom: number; left: number; right: number; from: number }[] = [];
    const walker = document.createTreeWalker(element, NodeFilter.SHOW_TEXT);
    let offset = 0;
    for (let node = walker.nextNode(); node; node = walker.nextNode()) {
      const text = node.textContent ?? '';
      for (let index = 0; index < text.length; index++, offset++) {
        const range = document.createRange();
        range.setStart(node, index);
        range.setEnd(node, index + 1);
        const rect = range.getBoundingClientRect();
        if (!rect.width) continue;
        const row = rows.at(-1);
        if (!row || rect.top >= row.bottom - 2)
          rows.push({
            top: rect.top,
            bottom: rect.bottom,
            left: rect.left,
            right: rect.right,
            from: offset,
          });
        else row.right = Math.max(row.right, rect.right);
      }
    }
    return rows;
  }, line);
}

/** Read the painted rectangle of the collapsed native caret inside the prompt. */
async function paintedCaret(page: Page): Promise<{ x: number; top: number; bottom: number }> {
  return page.evaluate(() => {
    const selection = getSelection(),
      content = document.querySelector('.cm-content');
    if (!selection?.isCollapsed || !selection.focusNode || !content?.contains(selection.focusNode))
      throw new Error('The prompt has no collapsed caret');
    const rect = selection.getRangeAt(0).getBoundingClientRect();
    return { x: rect.left, top: rect.top, bottom: rect.bottom };
  });
}

/** Wait until the native caret is painted on the row containing the given vertical point. */
async function expectCaretOnRow(page: Page, row: VisualRow): Promise<void> {
  await page.waitForFunction(
    ({ top, bottom }) => {
      const selection = getSelection();
      if (!selection?.rangeCount) return false;
      const rect = selection.getRangeAt(0).getBoundingClientRect();
      const middle = (rect.top + rect.bottom) / 2;
      return rect.height > 0 && middle >= top && middle <= bottom;
    },
    { top: row.top, bottom: row.bottom },
    { timeout: 2_000 },
  );
}

test.each([
  ['nodes-1', 1],
  ['nodes-2', 0.85],
] as const)(
  '%s at scale %d keeps a click past a wrapped row end on that row and a click at the next row start on the next row',
  async (renderer, scale) => {
    const page = await mountPrompt(fixture, renderer, wrapped, scale);
    try {
      const rows = await visualRows(page, 0);
      expect(rows.length).toBeGreaterThanOrEqual(2);
      const [first, second] = rows as [VisualRow, VisualRow];
      const content = await page.locator('.cm-content').boundingBox();
      if (!content) throw new Error('Missing prompt geometry');
      const pastFirst = { x: content.x + content.width - 2, y: (first.top + first.bottom) / 2 },
        secondStart = { x: second.left + 1, y: (second.top + second.bottom) / 2 };
      // Alternate between both sides of the same wrap offset in both orders.
      for (const [point, row] of [
        [pastFirst, first],
        [secondStart, second],
        [pastFirst, first],
        [secondStart, second],
      ] as const) {
        await page.mouse.click(point.x, point.y);
        await expectCaretOnRow(page, row);
      }
      await page.mouse.click(pastFirst.x, pastFirst.y);
      await expectCaretOnRow(page, first);
      expect(Math.abs((await paintedCaret(page)).x - first.right)).toBeLessThan(2);
      await page.keyboard.insertText('|');
      await expectSource(page, wrapped.slice(0, second.from) + '|' + wrapped.slice(second.from));
      expect(await page.evaluate(() => window.promptBrowserProof.read().errors)).toEqual([]);
    } finally {
      await page.close();
    }
  },
  30_000,
);

test.each(['nodes-1', 'nodes-2'] as const)(
  '%s places clicks at the clicked column, at line ends and on empty lines of a decorated prompt',
  async (renderer) => {
    const source = 'first, (bright eyes:1.2), {colors}\n\nshort\n';
    const page = await mountPrompt(fixture, renderer, source);
    try {
      const lines = await page.evaluate(() =>
        [...document.querySelectorAll('.cm-content .cm-line')].map((line) => {
          const rect = line.getBoundingClientRect();
          return { top: rect.top, bottom: rect.bottom, left: rect.left, right: rect.right };
        }),
      );
      expect(lines).toHaveLength(4);
      const [, empty, short, last] = lines as [VisualRow, VisualRow, VisualRow, VisualRow];
      // Inside a word: between "s" and "h" of "short".
      const word = await page.evaluate(() => {
        const line = document.querySelectorAll('.cm-content .cm-line')[2];
        if (!line) throw new Error('Missing short line');
        const walker = document.createTreeWalker(line, NodeFilter.SHOW_TEXT);
        let text = walker.nextNode();
        while (text && !text.textContent?.startsWith('short')) text = walker.nextNode();
        if (!text) throw new Error('Missing short text');
        const range = document.createRange();
        range.setStart(text, 1);
        range.setEnd(text, 2);
        const rect = range.getBoundingClientRect();
        return { x: rect.left + 1, y: (rect.top + rect.bottom) / 2 };
      });
      await page.mouse.click(word.x, word.y);
      await page.keyboard.insertText('|');
      await expectSource(page, 'first, (bright eyes:1.2), {colors}\n\ns|hort\n');
      await page.keyboard.press('Backspace');
      // Past the end of a line.
      await page.mouse.click(short.right - 2, (short.top + short.bottom) / 2);
      await page.keyboard.insertText('|');
      await expectSource(page, 'first, (bright eyes:1.2), {colors}\n\nshort|\n');
      await page.keyboard.press('Backspace');
      // Empty lines between and after text.
      for (const [line, expected] of [
        [empty, 'first, (bright eyes:1.2), {colors}\n|\nshort\n'],
        [last, 'first, (bright eyes:1.2), {colors}\n\nshort\n|'],
      ] as const) {
        await page.mouse.click((line.left + line.right) / 2, (line.top + line.bottom) / 2);
        await page.keyboard.insertText('|');
        await expectSource(page, expected);
        await page.keyboard.press('Backspace');
        await expectSource(page, source);
      }
      // Just right of the weight badge stays on the first row, before the following comma.
      const badge = await page.locator('.cm-content').evaluate((content) => {
        const walker = document.createTreeWalker(content, NodeFilter.SHOW_TEXT);
        for (let node = walker.nextNode(); node; node = walker.nextNode())
          if (node.textContent === '1.2') {
            const range = document.createRange();
            range.selectNodeContents(node);
            const rect = range.getBoundingClientRect();
            return { x: rect.right + 1, y: (rect.top + rect.bottom) / 2 };
          }
        return null;
      });
      if (!badge) throw new Error('Missing rendered weight');
      await page.mouse.click(badge.x, badge.y);
      await page.keyboard.insertText('|');
      await expectSource(page, 'first, (bright eyes:1.2)|, {colors}\n\nshort\n');
      expect(await page.evaluate(() => window.promptBrowserProof.read().errors)).toEqual([]);
    } finally {
      await page.close();
    }
  },
  30_000,
);
