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
 * Prove a right-click anywhere on the prompt's text opens the prompt's own menu and never the
 * browser's, including on an empty first or last line and in an empty prompt, where the precise
 * position lookup finds nothing.
 * @jest-environment node
 */
import type { Page } from '@playwright/test';
import {
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

/** Record whether the browser's default action survived each context-menu event. */
async function watchDefaults(page: Page): Promise<void> {
  await page.evaluate(() => {
    const seen: boolean[] = [];
    Object.assign(window, { contextDefaults: seen });
    // The prompt host stops the event's propagation, so it is caught on the way in and read once
    // every listener has run.
    window.addEventListener(
      'contextmenu',
      (event) => setTimeout(() => seen.push(event.defaultPrevented), 0),
      true,
    );
  });
}

/** Right-click the middle of source line `line` and report what the page did with it. */
async function rightClickLine(
  page: Page,
  line: number,
): Promise<{ readonly prevented: boolean | undefined; readonly menu: boolean }> {
  const box = await page.evaluate((index) => {
    const element = document.querySelectorAll('.cm-content .cm-line')[index];
    if (!element) return null;
    const rect = element.getBoundingClientRect();
    return { x: rect.left + Math.max(4, rect.width / 2), y: rect.top + rect.height / 2 };
  }, line);
  if (!box) throw new Error(`line ${String(line)} is not painted`);
  await page.mouse.click(box.x, box.y, { button: 'right' });
  await page.waitForTimeout(150);
  return page.evaluate(() => ({
    prevented: (window as unknown as { contextDefaults: boolean[] }).contextDefaults.at(-1),
    menu: !!document.querySelector('[data-sc-prompt-menu], .sc-context-menu'),
  }));
}

describe.each(['nodes-1', 'nodes-2'] as const)('%s', (renderer) => {
  test.each([
    ['an empty prompt', '', 0],
    ['an empty first line', '\nabc', 0],
    ['an empty last line', 'abc\n', 1],
    ['an empty middle line', 'a\n\nb', 1],
    ['a line with text', 'abc', 0],
  ])('%s opens the prompt’s menu, never the browser’s', async (_name, source, line) => {
    const page = await mountPrompt(fixture, renderer, source);
    await watchDefaults(page);
    const result = await rightClickLine(page, line);
    expect(result.prevented).toBe(true);
    expect(result.menu).toBe(true);
    await page.close();
  });
});
