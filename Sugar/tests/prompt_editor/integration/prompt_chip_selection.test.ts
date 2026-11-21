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
 * Prove a selection that covers a LoRA chip shows on the chip: the native highlight paints text
 * only, so the editor draws the selection colour behind every chip the selection covers.
 */
import type { Page } from '@playwright/test';
import {
  mountPrompt,
  startPromptBrowser,
  type PromptBrowserFixture,
} from '../support/PromptBrowserServer.js';

let fixture: PromptBrowserFixture;
const source = 'a cat, <lora:example:0.8>, tail';

beforeAll(async () => {
  fixture = await startPromptBrowser();
});

afterAll(async () => {
  await fixture.close();
});

/** Every chip box and every drawn selection box, in client coordinates. */
async function boxes(page: Page) {
  return page.evaluate(() => {
    const rect = (element: Element) => {
      const box = element.getBoundingClientRect();
      return { left: box.left, right: box.right, top: box.top, bottom: box.bottom };
    };
    return {
      chips: [...document.querySelectorAll('.sp-lora-chip')].map(rect),
      marks: [...document.querySelectorAll('.sp-widget-selection')].map(rect),
    };
  });
}

describe.each(['nodes-1', 'nodes-2'] as const)('%s', (renderer) => {
  test('select all draws the selection behind every chip, a little wider than it', async () => {
    const page = await mountPrompt(fixture, renderer, source);
    await page.locator('.sp-lora-chip').first().waitFor();
    await page.locator('.cm-content').click();
    await page.keyboard.press('Control+A');
    await page.waitForTimeout(100);
    const { chips, marks } = await boxes(page);
    expect(chips.length).toBeGreaterThan(0);
    expect(marks).toHaveLength(chips.length);
    for (const [index, chip] of chips.entries()) {
      const mark = marks[index];
      if (!mark) throw new Error('missing selection box');
      expect(mark.left).toBeLessThan(chip.left);
      expect(mark.right).toBeGreaterThan(chip.right);
      expect(Math.abs(mark.top - chip.top)).toBeLessThan(1);
      expect(Math.abs(mark.bottom - chip.bottom)).toBeLessThan(1);
    }
    await page.close();
  });

  test('a caret alone, or a selection that ends inside the chip, draws nothing', async () => {
    const page = await mountPrompt(fixture, renderer, source);
    await page.locator('.sp-lora-chip').first().waitFor();
    await page.locator('.cm-content').click();
    await page.keyboard.press('Control+Home');
    await page.waitForTimeout(100);
    expect((await boxes(page)).marks).toHaveLength(0);
    // Select only "a cat": the chip lies outside the selection.
    await page.keyboard.press('Shift+End');
    for (let step = 0; step < 30; step += 1) await page.keyboard.press('Shift+ArrowLeft');
    await page.keyboard.press('Control+Home');
    await page.keyboard.down('Shift');
    for (let step = 0; step < 5; step += 1) await page.keyboard.press('ArrowRight');
    await page.keyboard.up('Shift');
    await page.waitForTimeout(100);
    expect((await boxes(page)).marks).toHaveLength(0);
    await page.close();
  });
});
