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
 * Prove that only Tab accepts a suggestion while space and comma stay ordinary typing.
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
}, 60_000);

afterAll(async () => {
  await fixture.close();
}, 30_000);

/** Wait for the visible suggestion row and its inline ghost completion. */
async function suggests(page: Page, label: RegExp, ghost: string): Promise<void> {
  await page.getByRole('option', { name: label }).first().waitFor({ timeout: 10_000 });
  await page.locator('.cm-content .cm-tag-ghost', { hasText: ghost }).waitFor();
}

test.each(['nodes-1', 'nodes-2'] as const)(
  '%s inserts comma and space as typed text and accepts only with Tab',
  async (renderer) => {
    const page = await mountPrompt(fixture, renderer, '');
    try {
      await page.locator('.cm-content').focus();
      await page.keyboard.type('blue_ey');
      await suggests(page, /^blue eyes,/, 'es');
      // Comma ends the tag as typed and closes the list.
      await page.keyboard.press('Comma');
      await expectSource(page, 'blue_ey,');
      await page.getByRole('listbox').waitFor({ state: 'detached' });
      await page.keyboard.press('Tab');
      await expectSource(page, 'blue_ey,');

      // A chosen row is still only accepted by Tab, never by comma.
      await page.keyboard.press('Control+a');
      await page.keyboard.type('blue_ey');
      await suggests(page, /^blue eyes,/, 'es');
      await page.keyboard.press('ArrowDown');
      await page.getByRole('option', { name: /^blue eyeshadow/, selected: true }).waitFor();
      await page.keyboard.press('Comma');
      await expectSource(page, 'blue_ey,');

      // Space is part of multi-word tags: it is inserted and the list refreshes, then Tab accepts.
      await page.keyboard.press('Control+a');
      await page.keyboard.type('blue');
      await suggests(page, /^blue eyes,/, 'eyes');
      await page.keyboard.press('Space');
      await expectSource(page, 'blue ');
      await page.keyboard.type('ey');
      await suggests(page, /^blue eyes,/, 'es');
      await page.keyboard.press('Space');
      await expectSource(page, 'blue ey ');
      await page.keyboard.press('Comma');
      await expectSource(page, 'blue ey ,');

      await page.keyboard.press('Control+a');
      await page.keyboard.type('long_hai');
      await suggests(page, /^long hair,/, 'r');
      await page.keyboard.press('Tab');
      await expectSource(page, 'long hair, ');
      const observed = await page.evaluate(() => window.promptBrowserProof.read());
      expect(observed.errors).toEqual([]);
    } finally {
      await page.close();
    }
  },
  60_000,
);
