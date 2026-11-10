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
 * Prove that the zero-width widget buffer CodeMirror places beside every widget sits on the text
 * baseline of its own line, whatever the host page does to bare images. Firefox paints the native
 * caret from this box whenever the selection sits before it, which is every empty line and every line
 * that begins with a superscript chip. A buffer aligned to `text-top` makes Firefox paint the caret
 * about one row above the row CodeMirror's selection names, so clicks and Up/Down appear to skip
 * empty lines even though the selection itself is correct.
 * @jest-environment node
 */
import {
  mountPrompt,
  startPromptBrowser,
  type PromptBrowserFixture,
} from '../support/PromptBrowserServer.js';

let fixture: PromptBrowserFixture;

/** Empty lines, lines that begin with a chip and lines that end with one: every buffer shape. */
const source = [
  'best quality, (wind:2.00), dusk,',
  '',
  '(flat chest:2.00), small breasts,',
  '',
  'hand on headwear, (oversized brim:-1.00),',
  '',
  'pink witch hat, (notched hat:-1.00), slim',
].join('\n');

/** Reset rules of the kind host pages and user stylesheets apply to every bare `img`. */
const hostImageRules = {
  'untouched host page': '',
  'responsive images': 'img { max-width: 100% !important; height: auto !important; }',
  'block images with a middle alignment':
    'img { display: block !important; vertical-align: middle !important; }',
  'images aligned to the line top': 'img { vertical-align: top !important; }',
  'text-top images': 'img { vertical-align: text-top !important; }',
} as const;

beforeAll(async () => {
  fixture = await startPromptBrowser();
}, 60_000);

afterAll(async () => {
  await fixture.close();
}, 30_000);

interface BufferBox {
  readonly verticalAlign: string;
  readonly display: string;
  readonly width: number;
  readonly aboveLineTop: number;
  readonly belowLineBottom: number;
}

test.each(
  (['nodes-1', 'nodes-2'] as const).flatMap((renderer) =>
    Object.entries(hostImageRules).map(([name, rule]) => [renderer, name, rule] as const),
  ),
)(
  '%s keeps every widget buffer on the baseline of its line under %s',
  async (renderer, _name, rule) => {
    const page = await mountPrompt(fixture, renderer, source, 1.09);
    try {
      if (rule) await page.addStyleTag({ content: rule });
      await page.evaluate(
        () =>
          new Promise<void>((resolve) =>
            requestAnimationFrame(() => requestAnimationFrame(() => setTimeout(resolve, 60))),
          ),
      );
      const buffers = await page.evaluate((): BufferBox[] =>
        [...document.querySelectorAll('[data-sc-prompt-editor] .cm-widgetBuffer')].map((image) => {
          const style = getComputedStyle(image);
          const box = image.getBoundingClientRect();
          const owner = image.closest('.cm-line');
          if (!owner) throw new Error('A widget buffer sits outside its editor line');
          const line = owner.getBoundingClientRect();
          return {
            verticalAlign: style.verticalAlign,
            display: style.display,
            width: box.width,
            aboveLineTop: box.top - line.top,
            belowLineBottom: line.bottom - box.bottom,
          };
        }),
      );
      // Three empty lines, three lines that begin with a chip and the chips that end other lines.
      expect(buffers.length).toBeGreaterThanOrEqual(6);
      for (const buffer of buffers) {
        expect(buffer.verticalAlign).toBe('baseline');
        expect(buffer.display).toBe('inline');
        expect(buffer.width).toBe(0);
        expect(buffer.aboveLineTop).toBeGreaterThanOrEqual(-0.5);
        expect(buffer.belowLineBottom).toBeGreaterThanOrEqual(-0.5);
      }
      expect(await page.evaluate(() => window.promptBrowserProof.read().errors)).toEqual([]);
    } finally {
      await page.close();
    }
  },
  60_000,
);
