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
/** Prove a mounted prompt editor follows the person's preferences without being mounted again. */
import type { Browser } from '@playwright/test';
import { startPromptBrowser, type PromptBrowserFixture } from '../support/PromptBrowserServer.js';

let fixture: PromptBrowserFixture;
let browser: Browser;

beforeAll(async () => {
  fixture = await startPromptBrowser();
  browser = fixture.browser;
});

afterAll(async () => {
  await fixture.close();
});

test.each(['nodes-1', 'nodes-2'] as const)(
  '%s turns emphasis off and on again in the open editor, keeping its document',
  async (renderer) => {
    const page = await browser.newPage();
    const source = 'a cat, (bright eyes:1.2), tail';
    try {
      await page.goto(fixture.address);
      await page.waitForFunction(() => !!window.promptBrowserProof);
      await page.evaluate(
        ({ renderer, source }) => window.promptBrowserProof.mount(renderer, 1, source),
        { renderer, source },
      );
      await page.locator('[data-sc-prompt-editor] .sp-emphasis-weight').first().waitFor();
      await page.evaluate(() => {
        const editor = document.querySelector<HTMLElement>('[data-sc-prompt-editor] .cm-editor');
        if (!editor) throw new Error('No mounted prompt editor');
        editor.dataset.proofIdentity = 'first';
      });

      await page.evaluate(() =>
        window.promptBrowserProof.preferences({ allowed: { emphasis: false } }),
      );
      await page.waitForFunction(
        () => !document.querySelector('[data-sc-prompt-editor] .sp-emphasis-weight'),
      );
      await page.evaluate(() =>
        window.promptBrowserProof.preferences({ allowed: { emphasis: true } }),
      );
      await page.locator('[data-sc-prompt-editor] .sp-emphasis-weight').first().waitFor();

      const observation = await page.evaluate(() => ({
        identity: document.querySelector<HTMLElement>('[data-sc-prompt-editor] .cm-editor')?.dataset
          .proofIdentity,
        editors: document.querySelectorAll('[data-sc-prompt-editor] .cm-editor').length,
        read: window.promptBrowserProof.read(),
      }));
      expect(observation.identity).toBe('first');
      expect(observation.editors).toBe(1);
      expect(observation.read.source).toBe(source);
      expect(observation.read.errors).toEqual([]);
    } finally {
      await page.close();
    }
  },
);
