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
 * Prove that typing into a long rich prompt only does work near the edit.
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

/** A prompt well past CodeMirror's long-line threshold, with weights, LoRA chips and wildcards. */
const longPrompt = Array.from({ length: 400 }, (_, index) =>
  index % 7 === 0
    ? `(tag number ${String(index)}:1.1)`
    : index % 11 === 0
      ? `<lora:example:0.${String(index % 9)}>`
      : index % 13 === 0
        ? '{colors}'
        : `tag ${String(index)}`,
).join(', ');

beforeAll(async () => {
  fixture = await startPromptBrowser();
}, 60_000);

afterAll(async () => {
  await fixture.close();
}, 30_000);

/** Count rebuilt rich widgets and native layout range reads from now on. */
async function observeWork(page: Page): Promise<() => Promise<{ widgets: number; reads: number }>> {
  await page.evaluate(() => {
    const counts = { widgets: 0, reads: 0 };
    const read = Range.prototype.getClientRects;
    Range.prototype.getClientRects = function (this: Range) {
      counts.reads++;
      return read.call(this);
    };
    new MutationObserver((records) => {
      for (const record of records)
        for (const node of record.addedNodes)
          if (
            node instanceof Element &&
            (node.matches('.sp-lora-chip') || node.querySelector('.sp-lora-chip'))
          )
            counts.widgets += node.matches('.sp-lora-chip')
              ? 1
              : node.querySelectorAll('.sp-lora-chip').length;
    }).observe(document.querySelector('.cm-content') ?? document.body, {
      childList: true,
      subtree: true,
    });
    Reflect.set(window, '__promptWork', counts);
  });
  return () =>
    page.evaluate(() => Reflect.get(window, '__promptWork') as { widgets: number; reads: number });
}

/** Wait until queued native measurement and paint after the last key have run. */
async function settle(page: Page): Promise<void> {
  await page.evaluate(
    () =>
      new Promise<void>((resolve) =>
        requestAnimationFrame(() => requestAnimationFrame(() => setTimeout(resolve, 50))),
      ),
  );
}

test.each(['nodes-1', 'nodes-2'] as const)(
  '%s typing at the end of a long prompt rebuilds no distant widgets and re-measures no settled layout',
  async (renderer) => {
    const page = await mountPrompt(fixture, renderer, longPrompt);
    try {
      await page.locator('.sp-lora-chip[data-media="ready"]').first().waitFor();
      await page.locator('.cm-content').focus();
      await page.keyboard.press('Control+End');
      await settle(page);
      const work = await observeWork(page);
      const typed = ', zzzz';
      for (const key of typed) {
        await page.keyboard.type(key);
        await settle(page);
      }
      await expectSource(page, longPrompt + typed);
      const observed = await work();
      expect(observed.widgets).toBe(0);
      // Settled word groups keep their measured widths; a full re-measure reads hundreds.
      expect(observed.reads).toBeLessThan(typed.length * 40);
      expect(await page.evaluate(() => window.promptBrowserProof.read().errors)).toEqual([]);
    } finally {
      await page.close();
    }
  },
  60_000,
);
