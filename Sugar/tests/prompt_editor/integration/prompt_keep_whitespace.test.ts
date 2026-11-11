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
 * Prove that word groups kept on one row preserve whitespace like the rest of the prompt.
 * @jest-environment node
 */
import {
  expectSource,
  mountPrompt,
  startPromptBrowser,
  type PromptBrowserFixture,
} from '../support/PromptBrowserServer.js';
import { BROWSER_FIXTURE_SETUP_TIMEOUT_MS } from '../../frontend_core/support/BrowserFixtureLifetime.js';

let fixture: PromptBrowserFixture;

beforeAll(async () => {
  fixture = await startPromptBrowser();
}, BROWSER_FIXTURE_SETUP_TIMEOUT_MS);

afterAll(async () => {
  await fixture.close();
}, 30_000);

test.each(['nodes-1', 'nodes-2'] as const)(
  '%s keeps whitespace preserved inside kept groups and types plain spaces there',
  async (renderer) => {
    const page = await mountPrompt(fixture, renderer, 'big  cat, tail');
    try {
      const kept = await page.evaluate(() => ({
        groups: document.querySelectorAll('.cm-content .sp-prompt-keep').length,
        collapse: [...document.styleSheets].flatMap((sheet) =>
          [...sheet.cssRules]
            .filter(
              (rule): rule is CSSStyleRule =>
                rule instanceof CSSStyleRule && /\.sp-prompt-keep\b/u.test(rule.selectorText),
            )
            .map((rule) => rule.style.getPropertyValue('white-space-collapse')),
        ),
      }));
      expect(kept.groups).toBeGreaterThan(0);
      // Collapsible whitespace makes Firefox type U+00A0 at a group edge and hides repeated spaces.
      expect(kept.collapse).toEqual(['preserve']);
      await page.locator('.cm-content').focus();
      await page.keyboard.press('Control+Home');
      for (let step = 0; step < 3; step++) await page.keyboard.press('ArrowRight');
      await page.keyboard.press('Space');
      await expectSource(page, 'big   cat, tail');
      expect(await page.evaluate(() => window.promptBrowserProof.read().errors)).toEqual([]);
    } finally {
      await page.close();
    }
  },
  30_000,
);
