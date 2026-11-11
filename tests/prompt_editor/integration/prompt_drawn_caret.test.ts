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
 * Prove the drawn caret looks and behaves like the native caret it replaces, that the native selection
 * highlight keeps its colour and that focus, pointer selection and IME composition keep working.
 */
import type { Page } from '@playwright/test';
import {
  expectSource,
  mountPrompt,
  startPromptBrowser,
  type PromptBrowserFixture,
} from '../support/PromptBrowserServer.js';
import { drawnCaret } from '../support/DrawnCaret.js';

let fixture: PromptBrowserFixture;
const source = 'best quality, (wind:2.00), petite\n\nlooking away';
const selectionColour = 'rgb(103, 179, 255)';

beforeAll(async () => {
  fixture = await startPromptBrowser();
}, 60_000);

afterAll(async () => {
  await fixture.close();
}, 30_000);

/** Wait for queued measurement and the drawn layers to paint. */
async function settle(page: Page): Promise<void> {
  await page.evaluate(
    () =>
      new Promise<void>((resolve) =>
        requestAnimationFrame(() => requestAnimationFrame(() => setTimeout(resolve, 40))),
      ),
  );
}

/** Mount the prompt inside a host that publishes a selection colour, and focus it with a real click. */
async function mountFocused(renderer: 'nodes-1' | 'nodes-2'): Promise<Page> {
  const page = await mountPrompt(fixture, renderer, source);
  await page.addStyleTag({
    content: `[data-sc-prompt-editor]{--sugarprompt-selection-background:${selectionColour}}`,
  });
  const box = await page.locator('.cm-content').boundingBox();
  if (!box) throw new Error('Missing prompt geometry');
  await page.mouse.click(box.x + 20, box.y + 10);
  await settle(page);
  return page;
}

/** Text offset of `text` in the prompt source. */
function offsetOf(text: string): number {
  const offset = source.indexOf(text);
  if (offset < 0) throw new Error(`Missing ${text}`);
  return offset;
}

test.each(['nodes-1', 'nodes-2'] as const)(
  '%s draws a one-pixel caret of the text colour, as tall as the caret rectangle, blinking every 530 ms',
  async (renderer) => {
    const page = await mountFocused(renderer);
    try {
      for (const [position, side] of [
        [offsetOf('quality') + 3, 1],
        [offsetOf('wind') + 2, 1],
        [offsetOf(':2.00'), -1],
        [offsetOf(', petite'), 1],
        [offsetOf('\n\nlooking') + 1, 1],
      ] as const) {
        await page.evaluate(([at, assoc]) => window.promptBrowserProof.placeCaret(at, assoc), [
          position,
          side,
        ] as const);
        await settle(page);
        const caret = await drawnCaret(page);
        const expected = await page.evaluate(
          ([at, assoc]) => {
            const content = document.querySelector('[data-sc-prompt-editor] .cm-content');
            const cursor = document.querySelector('[data-sc-prompt-editor] .cm-cursor-primary');
            if (!content || !cursor) throw new Error('Missing drawn caret');
            const style = getComputedStyle(cursor);
            const row = window.promptBrowserProof.caretRect(at, assoc);
            return {
              colour: style.backgroundColor === getComputedStyle(content).color,
              width: style.width,
              row,
            };
          },
          [position, side] as const,
        );
        expect({ position, colour: expected.colour, width: expected.width }).toEqual({
          position,
          colour: true,
          width: '1px',
        });
        if (!caret || !expected.row) throw new Error(`No caret drawn at ${String(position)}`);
        // The one-pixel caret is centred on the caret position.
        expect(Math.abs(caret.x + caret.width / 2 - expected.row.left)).toBeLessThan(0.25);
        expect(Math.abs(caret.top - expected.row.top)).toBeLessThan(0.5);
        expect(Math.abs(caret.bottom - expected.row.bottom)).toBeLessThan(0.5);
      }
      expect(
        await page.evaluate(
          () =>
            document.querySelector<HTMLElement>('[data-sc-prompt-editor] .cm-cursorLayer')?.style
              .animationDuration,
        ),
      ).toBe('1060ms');
      // The blink runs on its own, as the native caret's does.
      expect(
        await page.evaluate(() =>
          document
            .querySelector('[data-sc-prompt-editor] .cm-cursorLayer')
            ?.getAnimations()
            .map((animation) => animation.playState),
        ),
      ).toEqual(['running']);
      expect(await page.evaluate(() => window.promptBrowserProof.read().errors)).toEqual([]);
    } finally {
      await page.close();
    }
  },
  30_000,
);

test.each(
  (['nodes-1', 'nodes-2'] as const).flatMap((renderer) =>
    [
      [0.8, 1.25],
      [1, 1],
      [1.48, 1],
    ].map(([scale, width]) => [renderer, scale, width] as const),
  ),
)(
  '%s at scale %d draws a caret %d CSS pixels wide, so it never paints narrower than one device pixel',
  async (renderer, scale, width) => {
    const page = await mountPrompt(fixture, renderer, source, scale);
    try {
      const box = await page.locator('.cm-content').boundingBox();
      if (!box) throw new Error('Missing prompt geometry');
      await page.mouse.click(box.x + 20, box.y + 10);
      await settle(page);
      await page.evaluate(
        (at) => window.promptBrowserProof.placeCaret(at, 1),
        offsetOf('quality') + 3,
      );
      await settle(page);
      const drawn = await page.evaluate(() => {
        const cursor = document.querySelector('[data-sc-prompt-editor] .cm-cursor-primary');
        return cursor ? Number.parseFloat(getComputedStyle(cursor).width) : null;
      });
      expect(drawn).not.toBeNull();
      expect(Math.abs((drawn ?? 0) - width)).toBeLessThan(0.01);
    } finally {
      await page.close();
    }
  },
  30_000,
);

test.each(['nodes-1', 'nodes-2'] as const)(
  '%s keeps the native highlight colour for a dragged selection, draws no caret over it and keeps it when blurred',
  async (renderer) => {
    const page = await mountFocused(renderer);
    try {
      const points = await page.evaluate(
        ([from, to]) => {
          const start = window.promptBrowserProof.caretRect(from, 1);
          const end = window.promptBrowserProof.caretRect(to, -1);
          if (!start || !end) throw new Error('Missing selection geometry');
          return {
            start: { x: start.left + 1, y: (start.top + start.bottom) / 2 },
            end: { x: end.left - 1, y: (end.top + end.bottom) / 2 },
          };
        },
        [offsetOf('quality'), offsetOf(', petite')] as const,
      );
      await page.mouse.move(points.start.x, points.start.y);
      await page.mouse.down();
      await page.mouse.move(points.end.x, points.end.y, { steps: 8 });
      await page.mouse.up();
      await settle(page);
      const selection = await page.evaluate(() => window.promptBrowserProof.selection());
      expect(selection?.anchor).toBe(offsetOf('quality'));
      expect(Math.abs((selection?.head ?? 0) - offsetOf(', petite'))).toBeLessThanOrEqual(1);
      expect(await drawnCaret(page)).toBeNull();
      const highlight = () =>
        page.evaluate(() => {
          const line = document.querySelector('[data-sc-prompt-editor] .cm-line');
          const layer = document.querySelector('[data-sc-prompt-editor] .cm-selectionLayer');
          if (!line || !layer) throw new Error('Missing selection surfaces');
          return {
            native: getComputedStyle(line, '::selection').backgroundColor,
            drawn: getComputedStyle(layer).display,
          };
        });
      expect(await highlight()).toEqual({ native: selectionColour, drawn: 'none' });
      await page.evaluate(() => (document.activeElement as HTMLElement | null)?.blur());
      await settle(page);
      expect(await highlight()).toEqual({ native: selectionColour, drawn: 'none' });
      expect(await drawnCaret(page)).toBeNull();
    } finally {
      await page.close();
    }
  },
  30_000,
);

test.each(['nodes-1', 'nodes-2'] as const)(
  '%s draws the caret only while focused and composes IME text at the caret',
  async (renderer) => {
    const page = await mountFocused(renderer);
    try {
      expect(await drawnCaret(page)).not.toBeNull();
      await page.evaluate(() => (document.activeElement as HTMLElement | null)?.blur());
      await settle(page);
      expect(await drawnCaret(page)).toBeNull();
      const box = await page.locator('.cm-content').boundingBox();
      if (!box) throw new Error('Missing prompt geometry');
      await page.mouse.click(box.x + 20, box.y + 10);
      await settle(page);
      await page.evaluate((at) => window.promptBrowserProof.placeCaret(at, 1), offsetOf('looking'));
      await settle(page);
      const ime = await page.context().newCDPSession(page);
      await ime.send('Input.imeSetComposition', {
        text: 'かな',
        selectionStart: 2,
        selectionEnd: 2,
      });
      await settle(page);
      await ime.send('Input.insertText', { text: '仮名' });
      await expectSource(page, source.replace('looking', '仮名looking'));
      await settle(page);
      const caret = await drawnCaret(page);
      const expected = await page.evaluate(
        (at) => window.promptBrowserProof.caretRect(at, 1),
        offsetOf('looking') + 2,
      );
      if (!caret || !expected) throw new Error('No caret after composition');
      expect(Math.abs(caret.x + caret.width / 2 - expected.left)).toBeLessThan(0.25);
      expect(await page.evaluate(() => window.promptBrowserProof.read().errors)).toEqual([]);
    } finally {
      await page.close();
    }
  },
  30_000,
);
