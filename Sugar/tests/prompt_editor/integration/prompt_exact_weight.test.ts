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
 * Prove exact weight editing through real double-clicks, keys and clicks.
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
const source = 'a cat, (bright eyes:1.2), tail and more words here';

beforeAll(async () => {
  fixture = await startPromptBrowser();
});

afterAll(async () => {
  await fixture.close();
});

/** Open the exact weight input by double-clicking the rendered weight. */
async function editWeight(page: Page): Promise<void> {
  const weight = page.locator('.cm-content .sp-weight-value').first();
  await weight.dblclick();
  await page.locator('input.sp-exact-weight-input').waitFor();
  await page.waitForFunction(
    () => document.activeElement?.classList.contains('sp-exact-weight-input') === true,
  );
}

/** Return the centre of one character of a rendered word in the prompt. */
async function characterPoint(
  page: Page,
  word: string,
  offset: number,
): Promise<{ x: number; y: number }> {
  return page.evaluate(
    ({ word, offset }) => {
      const walker = document.createTreeWalker(
        document.querySelector('.cm-content') ?? document.body,
        NodeFilter.SHOW_TEXT,
      );
      for (let node = walker.nextNode(); node; node = walker.nextNode()) {
        const index = node.textContent?.indexOf(word) ?? -1;
        if (index < 0) continue;
        const range = document.createRange();
        range.setStart(node, index + offset);
        range.setEnd(node, index + offset + 1);
        const rect = range.getBoundingClientRect();
        return { x: rect.left + 1, y: rect.top + rect.height / 2 };
      }
      throw new Error(`Missing rendered ${word}`);
    },
    { word, offset },
  );
}

test.each(['nodes-1', 'nodes-2'] as const)(
  '%s leaves an untouched weight unchanged on Enter and commits an edited one',
  async (renderer) => {
    const page = await mountPrompt(fixture, renderer, source);
    try {
      const before = await page.evaluate(() => window.promptBrowserProof.read().callbacks);
      await editWeight(page);
      await page.keyboard.press('Enter');
      // Focus returns to the prompt only after any committed weight has been published.
      await page.waitForFunction(
        () => document.activeElement?.classList.contains('cm-content') === true,
      );
      expect(await page.evaluate(() => window.promptBrowserProof.read())).toMatchObject({
        source,
        callbacks: before,
      });
      await editWeight(page);
      await page.keyboard.press('Control+a');
      await page.keyboard.type('1.45');
      await page.keyboard.press('Enter');
      await expectSource(page, source.replace('1.2', '1.45'));
      await page.keyboard.press('Control+z');
      // Undo reaches the original spelling, proving the untouched Enter added no history step.
      await expectSource(page, source);
      await page.keyboard.press('Control+z');
      await expectSource(page, source);
      expect(await page.evaluate(() => window.promptBrowserProof.read().errors)).toEqual([]);
    } finally {
      await page.close();
    }
  },
);

test.each(['nodes-1', 'nodes-2'] as const)(
  '%s commits an edited weight and places the caret where the prompt was clicked',
  async (renderer) => {
    const page = await mountPrompt(fixture, renderer, source);
    try {
      await editWeight(page);
      await page.keyboard.press('Control+a');
      await page.keyboard.type('1.5');
      const point = await characterPoint(page, 'more', 2);
      await page.mouse.click(point.x, point.y);
      await expectSource(page, source.replace('1.2', '1.50'));
      await page.keyboard.insertText('|');
      await expectSource(page, 'a cat, (bright eyes:1.50), tail and mo|re words here');
    } finally {
      await page.close();
    }
  },
);
