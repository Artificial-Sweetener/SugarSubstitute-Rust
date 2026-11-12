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
 * Prove that typing over a selection replaces it exactly, without native editing of its markers.
 * @jest-environment node
 */
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

test.each(['nodes-1', 'nodes-2'] as const)(
  '%s replaces a whole selection spanning empty lines with exactly the typed text',
  async (renderer) => {
    for (const source of ['blue_ey\n', 'one\n\ntwo, three\n']) {
      const page = await mountPrompt(fixture, renderer, source);
      try {
        await page.evaluate(() => {
          window.__nativeTyping = [];
          document.addEventListener('beforeinput', (event) =>
            window.__nativeTyping.push(!event.defaultPrevented),
          );
        });
        await page.locator('.cm-content').click();
        await page.keyboard.press('Control+a');
        await page.keyboard.type('xy');
        await expectSource(page, 'xy');
        // Chrome's native replacement merges the zero-width text of deleted empty-line markers
        // into the typed text node, so a selection is replaced by the editor itself.
        expect(await page.evaluate(() => window.__nativeTyping[0])).toBe(false);
        expect(await page.evaluate(() => window.promptBrowserProof.read().errors)).toEqual([]);
      } finally {
        await page.close();
      }
    }
  },
);

declare global {
  interface Window {
    __nativeTyping: boolean[];
  }
}
