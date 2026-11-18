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
});

afterAll(async () => {
  await fixture.close();
});

/** Wait for the visible suggestion row and its inline ghost completion. */
async function suggests(page: Page, label: RegExp, ghost: string): Promise<void> {
  await page.getByRole('option', { name: label }).first().waitFor();
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
);

test.each(['nodes-1', 'nodes-2'] as const)(
  '%s retexts the mounted ghost completion while typing refreshes the suggestion',
  async (renderer) => {
    const page = await mountPrompt(fixture, renderer, '');
    try {
      await page.locator('.cm-content').focus();
      await page.keyboard.type('blue');
      await suggests(page, /^blue eyes,/, 'eyes');
      const ghost = await page.locator('.cm-content .cm-tag-ghost').elementHandle();
      if (!ghost) throw new Error('Missing ghost completion');
      await page.keyboard.type('_ey');
      await suggests(page, /^blue eyes,/, 'es');
      expect(
        await ghost.evaluate((element) => ({
          connected: element.isConnected,
          text: element.textContent,
          ghosts: document.querySelectorAll('.cm-content .cm-tag-ghost').length,
        })),
      ).toEqual({ connected: true, text: 'es', ghosts: 1 });
      await expectSource(page, 'blue_ey');
      // Leaving the query retires the ghost entirely.
      await page.keyboard.press('Comma');
      await expectSource(page, 'blue_ey,');
      await page.waitForFunction(() => !document.querySelector('.cm-content .cm-tag-ghost'));
      expect(await page.evaluate(() => window.promptBrowserProof.read().errors)).toEqual([]);
    } finally {
      await page.close();
    }
  },
);

test.each(['nodes-1', 'nodes-2'] as const)(
  '%s relabels the existing suggestion rows when typing refreshes the list',
  async (renderer) => {
    const page = await mountPrompt(fixture, renderer, '');
    try {
      await page.locator('.cm-content').focus();
      await page.keyboard.type('blue');
      await suggests(page, /^blue eyes,/, 'eyes');
      const first = await page.getByRole('option').first().elementHandle();
      if (!first) throw new Error('Missing first suggestion row');
      const before = await first.getAttribute('aria-label');
      await page.keyboard.type('_hai');
      await page.waitForFunction(([row, label]) => row?.getAttribute('aria-label') !== label, [
        first,
        before,
      ] as const);
      const shown = await page.evaluate(
        (row) => ({
          first: document.querySelector('[role="option"]') === row,
          label: row.querySelector('.cm-completionLabel')?.textContent ?? '',
          aria: row.getAttribute('aria-label') ?? '',
        }),
        first,
      );
      expect(shown.first).toBe(true);
      expect(shown.label).toMatch(/^blue hair/u);
      expect(shown.aria.startsWith(shown.label)).toBe(true);
      await page.keyboard.press('Tab');
      await expectSource(page, `${shown.label.replace(/,$/u, '')}, `);
      expect(await page.evaluate(() => window.promptBrowserProof.read().errors)).toEqual([]);
    } finally {
      await page.close();
    }
  },
);

test.each(['nodes-1', 'nodes-2'] as const)(
  '%s completes a LoRA name in the model picker popup only where the prompt loads LoRAs',
  async (renderer) => {
    const page = await mountPrompt(fixture, renderer, '', 1, true);
    try {
      await page.locator('.cm-content').focus();
      // Before the opener is complete the word list is the only completion.
      await page.keyboard.type('blue_ey');
      await suggests(page, /^blue eyes,/, 'es');
      await page.keyboard.type(', <lora:ex');
      const popup = page.locator('.sp-model-completions .sc-model-popup');
      await popup.locator('.sc-model-tile.sc-model-active').waitFor();
      // Models and words are never shown together.
      await page.locator('.cm-tooltip-autocomplete').waitFor({ state: 'detached' });
      expect(await popup.locator('.sp-completion-row').count()).toBe(0);
      expect(await popup.locator('.sc-model-tile').getAttribute('data-model-value')).toBe(
        'folder/example.safetensors',
      );
      // The ghost previews the model's name as literal text.
      await page.locator('.cm-content .cm-tag-ghost', { hasText: 'ample <b>model</b>' }).waitFor();
      // Enter is ordinary typing; Tab accepts with the default weight.
      await page.keyboard.press('Tab');
      await expectSource(page, 'blue_ey, <lora:folder/example:1.00>');
      await popup.waitFor({ state: 'detached' });

      // A click on the tile accepts too, keeping a weight already typed.
      await page.keyboard.press('Control+a');
      await page.keyboard.type('<lora::0.6>');
      for (let step = 0; step < 5; step++) await page.keyboard.press('ArrowLeft');
      await page.keyboard.type('e');
      await popup.locator('.sc-model-tile').click();
      await expectSource(page, '<lora:folder/example:0.6>');
      expect(await page.evaluate(() => !!document.activeElement?.closest('.cm-editor'))).toBe(true);
      const observed = await page.evaluate(() => window.promptBrowserProof.read());
      expect(observed.errors).toEqual([]);
    } finally {
      await page.close();
    }

    // A prompt that reaches no LoRA-loading encoder is never offered model names.
    const plain = await mountPrompt(fixture, renderer, '');
    try {
      await plain.locator('.cm-content').focus();
      await plain.keyboard.type('<lora:ex');
      await plain.waitForTimeout(600);
      expect(await plain.locator('.sp-model-completions').count()).toBe(0);
      await plain.keyboard.press('Tab');
      expect((await plain.evaluate(() => window.promptBrowserProof.read())).source).not.toContain(
        'folder/example',
      );
    } finally {
      await plain.close();
    }
  },
);

test.each(['nodes-1', 'nodes-2'] as const)(
  '%s schedules a LoRA from the prompt menu and offers its trigger words while typing',
  async (renderer) => {
    const page = await mountPrompt(fixture, renderer, 'a girl, ', 1, true);
    try {
      const content = page.locator('.cm-content');
      await content.focus();
      await page.keyboard.press('Control+End');
      await content.click({ button: 'right', position: { x: 300, y: 8 } });
      await page.getByRole('menuitem', { name: 'Schedule LoRA' }).first().click();
      // The model picker opens on its own, with a search box named for what it searches.
      const search = page.locator('.sc-model-popup:not(.sc-model-popup-inline) input');
      await search.waitFor();
      expect(await search.getAttribute('placeholder')).toBe('Search LoRA');
      await page.keyboard.type('exam');
      await page.locator('.sc-model-popup .sc-model-tile.sc-model-active').waitFor();
      await page.keyboard.press('Enter');
      await expectSource(page, 'a girl, <lora:folder/example:1.00>');
      await page.locator('.sc-model-popup').waitFor({ state: 'detached' });
      await page.waitForFunction(() => !!document.activeElement?.closest('.cm-editor'));

      // Its trigger words are offered as words while typing, never as model tiles.
      await page.locator('.sp-lora-chip').first().waitFor();
      await page.keyboard.press('Control+End');
      await page.keyboard.type(', examp');
      await page
        .getByRole('option', { name: /^example trigger,/ })
        .first()
        .waitFor();
      expect(await page.locator('.sp-model-completions').count()).toBe(0);
      const observed = await page.evaluate(() => window.promptBrowserProof.read());
      expect(observed.errors).toEqual([]);
    } finally {
      await page.close();
    }
  },
);

test.each(['nodes-1', 'nodes-2'] as const)(
  '%s lists the LoRAs affecting a prompt in the menu opened beside a LoRA',
  async (renderer) => {
    const page = await mountPrompt(
      fixture,
      renderer,
      'a girl, <lora:folder/example:1.00>',
      1,
      true,
    );
    try {
      await page.locator('.sp-lora-chip').first().waitFor();
      const box = await page.locator('.cm-editor').boundingBox();
      if (!box) throw new Error('Missing editor geometry');
      // Empty space after a LoRA is the prompt, not that LoRA.
      await page.mouse.click(box.x + box.width - 12, box.y + box.height - 6, { button: 'right' });
      // Its submenu lists them; the entries themselves are proven with the editor's own cases.
      await page.getByRole('menuitem', { name: 'Insert trigger words' }).first().waitFor();
      expect(await page.getByRole('menuitem', { name: /^Trigger words:/ }).count()).toBe(0);
      const observed = await page.evaluate(() => window.promptBrowserProof.read());
      expect(observed.errors).toEqual([]);
    } finally {
      await page.close();
    }
  },
);
