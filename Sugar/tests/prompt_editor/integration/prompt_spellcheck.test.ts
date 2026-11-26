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
 * Prove the spellchecker's worker, module and dictionary underline a misspelling, correct it from
 * the prompt menu and accept a word added to the dictionary, in both renderers.
 */
import type { Page } from '@playwright/test';
import {
  expectSource,
  mountPrompt,
  startPromptBrowser,
  type PromptBrowserFixture,
} from '../support/PromptBrowserServer.js';

let fixture: PromptBrowserFixture;

beforeAll(async () => {
  fixture = await startPromptBrowser();
});

afterAll(async () => {
  await fixture.close();
});

/** The underlined word, waited for until the dictionary has answered. */
function mark(page: Page, word: string) {
  return page.locator('[data-sc-prompt-editor] .sp-spelling-diagnostic', { hasText: word });
}

/** Choose one entry of the menu opened on an underlined word. */
async function choose(page: Page, word: string, entry: string): Promise<void> {
  await mark(page, word).first().click({ button: 'right' });
  const item = page.getByRole('menuitem', { name: entry, exact: true });
  await item.waitFor({ state: 'visible' });
  await item.click();
}

test.each(['nodes-1', 'nodes-2'] as const)(
  '%s underlines a misspelling, corrects it and accepts an added word',
  async (renderer) => {
    const page = await mountPrompt(fixture, renderer, 'a beautful garden, gentle smile');
    try {
      await page.evaluate(() =>
        window.promptBrowserProof.preferences({ allowed: { spellcheck: true } }),
      );
      await mark(page, 'beautful').first().waitFor({ state: 'visible' });
      // Correct words, proper nouns written in lower case included, are never underlined.
      expect(await mark(page, 'garden').count()).toBe(0);

      await choose(page, 'beautful', 'beautiful');
      await expectSource(page, 'a beautiful garden, gentle smile');
      await page.waitForFunction(
        () => !document.querySelector('[data-sc-prompt-editor] .sp-spelling-diagnostic'),
      );

      await page.evaluate(() => window.promptBrowserProof.replace('a zorblax garden, victorian'));
      await mark(page, 'zorblax').first().waitFor({ state: 'visible' });
      expect(await mark(page, 'victorian').count()).toBe(0);
      await choose(page, 'zorblax', 'Add to dictionary');
      await page.waitForFunction(
        () => !document.querySelector('[data-sc-prompt-editor] .sp-spelling-diagnostic'),
      );
      expect(await page.evaluate(() => window.promptBrowserProof.read().errors)).toEqual([]);
    } finally {
      await page.close();
    }
  },
);

test('spellcheck turned off removes every mark from the open editor', async () => {
  const page = await mountPrompt(fixture, 'nodes-2', 'a beautful garden');
  try {
    await page.evaluate(() =>
      window.promptBrowserProof.preferences({ allowed: { spellcheck: true } }),
    );
    await mark(page, 'beautful').first().waitFor({ state: 'visible' });
    await page.evaluate(() =>
      window.promptBrowserProof.preferences({ allowed: { spellcheck: false } }),
    );
    await page.waitForFunction(
      () => !document.querySelector('[data-sc-prompt-editor] .sp-spelling-diagnostic'),
    );
  } finally {
    await page.close();
  }
});

test('words of a known Danbooru tag are never underlined', async () => {
  const page = await mountPrompt(
    fixture,
    'nodes-2',
    'bokeh, long hair, absurdres, a kemonomimi girl, a beautful garden',
  );
  try {
    await page.evaluate(() =>
      window.promptBrowserProof.preferences({ allowed: { spellcheck: true } }),
    );
    await mark(page, 'beautful').first().waitFor({ state: 'visible' });
    for (const tag of ['bokeh', 'long', 'hair', 'absurdres', 'kemonomimi'])
      expect(await mark(page, tag).count()).toBe(0);
  } finally {
    await page.close();
  }
});
