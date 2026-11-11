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
 * Prove that regional separator rows never receive typed text from a click or an arrow key.
 * @jest-environment node
 */
import type { Page } from '@playwright/test';
import {
  expectSource,
  mountPrompt,
  startPromptBrowser,
  type PromptBrowserFixture,
} from '../support/PromptBrowserServer.js';
import { BROWSER_FIXTURE_SETUP_TIMEOUT_MS } from '../../frontend_core/support/BrowserFixtureLifetime.js';

let fixture: PromptBrowserFixture;
const source = 'global tags\n[SEP|left side]\nred hair\n[SEP|end]';

beforeAll(async () => {
  fixture = await startPromptBrowser();
}, BROWSER_FIXTURE_SETUP_TIMEOUT_MS);

afterAll(async () => {
  await fixture.close();
}, 30_000);

/** Return the rendered row of one regional separator. */
async function separatorRow(
  page: Page,
  index: number,
): Promise<{ left: number; right: number; middle: number }> {
  return page.evaluate((ordinal) => {
    const row = document
      .querySelectorAll('.cm-content .sp-region-separator')
      [ordinal]?.closest('.cm-line');
    if (!row) throw new Error('Missing regional separator row');
    const rect = row.getBoundingClientRect();
    return { left: rect.left, right: rect.right, middle: (rect.top + rect.bottom) / 2 };
  }, index);
}

/** Type a marker, require the exact resulting source, then remove the marker again. */
async function typeMarker(page: Page, expected: string): Promise<void> {
  await page.keyboard.type('|');
  await expectSource(page, expected);
  await page.keyboard.press('Backspace');
  await expectSource(page, source);
}

test.each(['nodes-1', 'nodes-2'] as const)(
  '%s places a click anywhere on a separator row in that region content',
  async (renderer) => {
    const page = await mountPrompt(fixture, renderer, source);
    try {
      await page.locator('.cm-content .sp-region-separator').first().waitFor();
      const row = await separatorRow(page, 0);
      for (const x of [row.left + 3, (row.left + row.right) / 2, row.right - 3]) {
        await page.mouse.click(x, row.middle);
        await typeMarker(page, source.replace('\nred hair', '\n|red hair'));
      }
      // The final separator has no content line yet; typing opens it below the marker.
      const final = await separatorRow(page, 1);
      await page.mouse.click((final.left + final.right) / 2, final.middle);
      await page.keyboard.type('|');
      await expectSource(page, source + '\n|');
      expect(await page.evaluate(() => window.promptBrowserProof.read().errors)).toEqual([]);
    } finally {
      await page.close();
    }
  },
  30_000,
);

test.each(['nodes-1', 'nodes-2'] as const)(
  '%s moves arrow keys across a separator row into the neighbouring region content',
  async (renderer) => {
    const page = await mountPrompt(fixture, renderer, source);
    try {
      await page.locator('.cm-content .sp-region-separator').first().waitFor();
      await page.locator('.cm-content').focus();
      await page.keyboard.press('Control+Home');
      for (const _character of 'global tags') await page.keyboard.press('ArrowRight');
      await page.keyboard.press('ArrowRight');
      await typeMarker(page, source.replace('\nred hair', '\n|red hair'));
      await page.keyboard.press('ArrowLeft');
      await typeMarker(page, source.replace('global tags', 'global tags|'));
      await page.keyboard.press('Control+Home');
      await page.keyboard.press('ArrowDown');
      await typeMarker(page, source.replace('\nred hair', '\n|red hair'));
      expect(await page.evaluate(() => window.promptBrowserProof.read().errors)).toEqual([]);
    } finally {
      await page.close();
    }
  },
  30_000,
);
