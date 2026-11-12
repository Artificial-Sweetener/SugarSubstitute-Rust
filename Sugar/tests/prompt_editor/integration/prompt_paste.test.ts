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
 * Prove keyboard paste through the real clipboard, including a clipboard that changes mid-read.
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

beforeAll(async () => {
  fixture = await startPromptBrowser();
});

afterAll(async () => {
  await fixture.close();
});

/** Mount a focused prompt with its caret at the end and system clipboard access granted. */
async function pastePrompt(renderer: 'nodes-1' | 'nodes-2', source: string): Promise<Page> {
  const page = await mountPrompt(fixture, renderer, source);
  await page.context().grantPermissions(['clipboard-read', 'clipboard-write']);
  await page.locator('.cm-content').focus();
  await page.keyboard.press('Control+End');
  return page;
}

/** Place text on the system clipboard and return the exact text a paste will receive. */
async function copyToClipboard(page: Page, text: string): Promise<string> {
  const copied = await page.evaluate(async (value) => {
    await navigator.clipboard.writeText(value);
    return navigator.clipboard.readText();
  }, text);
  // The platform clipboard owns newline spelling (Windows stores CRLF); the prompt keeps it exactly.
  expect(copied.replace(/\r\n/gu, '\n')).toBe(text);
  return copied;
}

test.each(['nodes-1', 'nodes-2'] as const)(
  '%s pastes plain, multiline and Unicode text at the caret with one undo step each',
  async (renderer) => {
    const page = await pastePrompt(renderer, 'a cat');
    try {
      const pasted: string[] = [];
      for (const text of [', blue eyes', ',\nline two,\nline three', ', 猫 👨‍👩‍👧 émoji']) {
        pasted.push(await copyToClipboard(page, text));
        await page.keyboard.press('Control+v');
        await expectSource(page, 'a cat' + pasted.join(''));
      }
      await page.keyboard.press('Control+z');
      await expectSource(page, 'a cat' + pasted.slice(0, -1).join(''));
      expect(await page.evaluate(() => window.promptBrowserProof.read().errors)).toEqual([]);
    } finally {
      await page.close();
    }
  },
);

test.each(['nodes-1', 'nodes-2'] as const)(
  '%s still pastes the pressed clipboard text when the clipboard changes during its read',
  async (renderer) => {
    const page = await pastePrompt(renderer, 'a cat');
    const failures: string[] = [];
    page.on('pageerror', (error) => failures.push(error.message));
    try {
      await page.evaluate(async () => {
        await navigator.clipboard.writeText(', pasted');
        const read = navigator.clipboard.read.bind(navigator.clipboard);
        // Another application replaces the clipboard after the editor began reading it.
        navigator.clipboard.read = async () => {
          const items = await read();
          await navigator.clipboard.writeText('changed meanwhile');
          return items;
        };
      });
      await page.keyboard.press('Control+v');
      await expectSource(page, 'a cat, pasted');
      expect(failures).toEqual([]);
    } finally {
      await page.close();
    }
  },
);
