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
/** Prove a pasted Danbooru link becomes the post's tags in one undo step, in both renderers. */
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

const url = 'https://danbooru.donmai.us/posts/123';

/** Paste `text` at the end of the prompt with a real Ctrl+V, which the browser trusts. */
async function paste(page: Page, text: string): Promise<void> {
  await page.context().grantPermissions(['clipboard-read', 'clipboard-write'], {
    origin: fixture.address,
  });
  await page.evaluate((value) => navigator.clipboard.writeText(value), text);
  await page.locator('[data-sc-prompt-editor] .cm-content').click();
  await page.keyboard.press('End');
  await page.keyboard.press('Control+v');
}

const post = (tags: string) => ({
  status: 'found',
  post: {
    id: 123,
    md5: null,
    tag_string_general: tags,
    tag_string_artist: 'an_artist',
    tag_string_copyright: '',
    tag_string_character: 'saber_(fate)',
    tag_string_meta: 'highres',
  },
});

test.each(['nodes-1', 'nodes-2'] as const)(
  '%s pastes the tags only once the post is found, and one undo removes them',
  async (renderer) => {
    const page = await mountPrompt(fixture, renderer, 'portrait, ');
    try {
      await paste(page, url);
      await page.waitForFunction(() => window.promptBrowserProof.booruAsked());
      // Nothing is pasted while the post is looked up.
      expect(await page.evaluate(() => window.promptBrowserProof.read().source)).toBe('portrait, ');
      await page.evaluate(
        (reply) => window.promptBrowserProof.answerBooru(reply),
        post('long_hair smile'),
      );
      await expectSource(page, 'portrait, long hair, smile, an artist, saber \\(fate\\)');
      await page.keyboard.press('Control+z');
      await expectSource(page, 'portrait, ');
      expect(await page.evaluate(() => window.promptBrowserProof.read().errors)).toEqual([]);
    } finally {
      await page.close();
    }
  },
);

test('a post that cannot be found pastes the link as written', async () => {
  const page = await mountPrompt(fixture, 'nodes-2', '');
  try {
    await paste(page, url);
    await page.waitForFunction(() => window.promptBrowserProof.booruAsked());
    await page.evaluate(() => window.promptBrowserProof.answerBooru({ status: 'not-found' }));
    await expectSource(page, url);
    await page.keyboard.press('Control+z');
    await expectSource(page, '');
  } finally {
    await page.close();
  }
});

test('turning Danbooru URL import off pastes a link as ordinary text', async () => {
  const page = await mountPrompt(fixture, 'nodes-2', '');
  try {
    await page.evaluate(() =>
      window.promptBrowserProof.preferences({ allowed: { danbooru_url_import: false } }),
    );
    await paste(page, url);
    await expectSource(page, url);
    expect(await page.evaluate(() => window.promptBrowserProof.booruAsked())).toBe(false);
  } finally {
    await page.close();
  }
});
