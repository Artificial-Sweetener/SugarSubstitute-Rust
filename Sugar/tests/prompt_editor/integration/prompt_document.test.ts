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
 * Prove the real prompt editor shows a whole file as the wildcard and list editor needs: every
 * line numbered, alternate lines shaded, rich prompt rendering for wildcards and none for lists.
 */
import type { Browser, Page } from '@playwright/test';
import { startPromptBrowser, type PromptBrowserFixture } from '../support/PromptBrowserServer.js';

let fixture: PromptBrowserFixture;
let browser: Browser;

beforeAll(async () => {
  fixture = await startPromptBrowser();
  browser = fixture.browser;
});

afterAll(async () => {
  await fixture.close();
});

async function open(source: string, kind: 'wildcard-txt' | 'tag-list'): Promise<Page> {
  const page = await browser.newPage();
  await page.goto(fixture.address);
  await page.waitForFunction(() => !!window.promptBrowserProof);
  await page.evaluate(({ source, kind }) => window.promptBrowserProof.mountDocument(source, kind), {
    source,
    kind,
  });
  await page.locator('[data-proof-document] .cm-content').waitFor({ state: 'visible' });
  return page;
}

test('a wildcard file is numbered, shaded every other line and rendered as prompts', async () => {
  const source = ['(bright eyes:1.2)', 'red', 'green', 'blue'].join('\n');
  const page = await open(source, 'wildcard-txt');
  try {
    await page.waitForFunction(
      () =>
        document.querySelectorAll('[data-proof-document] .sc-prompt-line-gutter__row').length === 4,
    );
    const observation = await page.evaluate(() => {
      const pane = document.querySelector<HTMLElement>('[data-proof-document]');
      const gutter = pane?.querySelector<HTMLElement>('.sc-prompt-line-gutter');
      const content = pane?.querySelector<HTMLElement>('.cm-content');
      const lines = [...(pane?.querySelectorAll<HTMLElement>('.cm-line') ?? [])];
      const rows = [...(pane?.querySelectorAll<HTMLElement>('.sc-prompt-line-gutter__row') ?? [])];
      const firstText = lines[0]?.getBoundingClientRect();
      return {
        numbers: rows.map((row) => row.textContent),
        alternate: rows.map((row) => row.classList.contains('is-alternate')),
        lineAlternate: lines.map((line) => line.classList.contains('sp-source-alternate')),
        aligned: rows.map((row, index) =>
          Math.abs(
            row.getBoundingClientRect().top - (lines[index]?.getBoundingClientRect().top ?? 0),
          ),
        ),
        clear:
          (gutter?.getBoundingClientRect().right ?? 0) <=
            (content?.getBoundingClientRect().left ?? 0) +
              parseFloat(getComputedStyle(content ?? document.body).paddingLeft) +
              1 && (firstText?.height ?? 0) > 0,
        emphasis: pane?.querySelectorAll('.sp-emphasis-weight').length ?? 0,
        height: pane?.querySelector<HTMLElement>('.sugarprompt-editor')?.getBoundingClientRect()
          .height,
      };
    });
    expect(observation.numbers).toEqual(['01', '02', '03', '04']);
    expect(observation.alternate).toEqual([false, true, false, true]);
    expect(observation.lineAlternate).toEqual([false, true, false, true]);
    expect(Math.max(...observation.aligned)).toBeLessThan(2);
    expect(observation.clear).toBe(true);
    expect(observation.emphasis).toBe(1);
    expect(observation.height).toBe(420);

    await page.locator('[data-proof-document] .cm-content').click();
    await page.keyboard.press('Control+End');
    await page.keyboard.type('\nyellow');
    await page.waitForFunction(() => window.promptBrowserProof.documentText()?.endsWith('yellow'));
    await page.waitForFunction(
      () =>
        document.querySelectorAll('[data-proof-document] .sc-prompt-line-gutter__row').length === 5,
    );
    const current = await page.evaluate(
      () =>
        document.querySelector('[data-proof-document] .sc-prompt-line-gutter__row.is-current')
          ?.textContent,
    );
    expect(current).toBe('05');
    expect(await page.evaluate(() => window.promptBrowserProof.errors())).toEqual([]);
  } finally {
    await page.close();
  }
});

test('a tag list shows its lines plainly, without prompt decorations', async () => {
  const page = await open(['(bright eyes:1.2)', '{colors}', 'long hair'].join('\n'), 'tag-list');
  try {
    await page.waitForFunction(
      () =>
        document.querySelectorAll('[data-proof-document] .sc-prompt-line-gutter__row').length === 3,
    );
    const decorations = await page.evaluate(
      () =>
        document.querySelectorAll(
          '[data-proof-document] :is(.sp-emphasis-weight, .sp-wildcard, .sp-lora-chip)',
        ).length,
    );
    expect(decorations).toBe(0);
    expect(await page.evaluate(() => window.promptBrowserProof.errors())).toEqual([]);
  } finally {
    await page.close();
  }
});
