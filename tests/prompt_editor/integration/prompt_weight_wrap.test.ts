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
 * Prove that stepping an emphasis weight with its arrows keeps a segment that sits at a soft wrap
 * on its row in every painted state, and that the arrows stay on the stepped number.
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

const prefix =
  '(wind lift, floating hair, hat lift, skirt lift, hand on own hat, ribbon lift:3.00), fighting stance, holding down headwear, hand on headwear, ';
const weighted = (weight: string): string => `${prefix}(oversized brim:${weight}),`;

beforeAll(async () => {
  fixture = await startPromptBrowser();
}, 60_000);

afterAll(async () => {
  await fixture.close();
}, 30_000);

/** Wait until queued native measurement and the wrap publication it schedules have painted. */
async function settle(page: Page): Promise<void> {
  await page.evaluate(
    () =>
      new Promise<void>((resolve) =>
        requestAnimationFrame(() =>
          requestAnimationFrame(() => requestAnimationFrame(() => setTimeout(resolve, 30))),
        ),
      ),
  );
}

/** Narrow the editor to the widest width at which the weighted segment starts the next row. */
async function wrapWeightedSegment(page: Page): Promise<void> {
  await page.evaluate(() => {
    const style = document.createElement('style');
    style.id = 'proof-wrap-width';
    document.head.append(style);
  });
  let chosen: number | null = null;
  for (let width = 260; width <= 720; width += 2) {
    await page.evaluate((value) => {
      const style = document.getElementById('proof-wrap-width');
      if (style)
        style.textContent = `[data-sc-prompt-editor]{width:${String(value)}px!important;max-width:${String(value)}px!important}`;
    }, width);
    await settle(page);
    const rows = await page.evaluate(() => window.promptWrapProof.rows());
    if (rows.segment > rows.preceding) chosen = width;
    else if (chosen !== null) break;
  }
  if (chosen === null) throw new Error('No width wraps the weighted segment');
  await page.evaluate((value) => {
    const style = document.getElementById('proof-wrap-width');
    if (style)
      style.textContent = `[data-sc-prompt-editor]{width:${String(value)}px!important;max-width:${String(value)}px!important}`;
  }, chosen);
  await settle(page);
}

/** Install row probes and a mutation recorder that observes every DOM state CodeMirror draws. */
async function installProbes(page: Page): Promise<void> {
  await page.evaluate(() => {
    const content = document.querySelector<HTMLElement>('[data-sc-prompt-editor] .cm-content');
    if (!content) throw new Error('Missing prompt content');
    /** Row top of the first character of the first text occurrence of a needle. */
    const rowOf = (needle: string): number => {
      const walker = document.createTreeWalker(content, NodeFilter.SHOW_TEXT);
      for (let node = walker.nextNode(); node; node = walker.nextNode()) {
        const index = node.textContent?.indexOf(needle) ?? -1;
        if (index < 0) continue;
        const range = document.createRange();
        range.setStart(node, index);
        range.setEnd(node, index + 1);
        return Math.round(range.getBoundingClientRect().top);
      }
      return Number.NaN;
    };
    const rows = (): { preceding: number; segment: number } => ({
      preceding: rowOf('on headwear'),
      segment: rowOf('oversized'),
    });
    const states: { segment: number; weight: string }[] = [];
    new MutationObserver(() => {
      states.push({
        segment: rowOf('oversized'),
        weight: [...content.querySelectorAll('.sp-emphasis-weight')].at(-1)?.textContent ?? '',
      });
    }).observe(content, { childList: true, subtree: true, characterData: true, attributes: true });
    Reflect.set(window, 'promptWrapProof', {
      rows,
      states: () => states.splice(0),
    });
  });
}

declare global {
  interface Window {
    promptWrapProof: {
      rows(): { preceding: number; segment: number };
      states(): { segment: number; weight: string }[];
    };
  }
}

test.each(['nodes-1', 'nodes-2'] as const)(
  '%s stepping emphasis arrows on a segment at a soft wrap keeps its row and its arrows',
  async (renderer) => {
    const page = await mountPrompt(fixture, renderer, weighted('-1.05'));
    try {
      await installProbes(page);
      await wrapWeightedSegment(page);
      const initial = await page.evaluate(() => window.promptWrapProof.rows());
      expect(initial.segment).toBeGreaterThan(initial.preceding);
      const label = page.locator('.cm-content .sp-emphasis-weight').last();
      const labelBox = await label.boundingBox();
      if (!labelBox) throw new Error('Missing weight label');
      await page.mouse.move(labelBox.x + labelBox.width / 2, labelBox.y + labelBox.height / 2);
      const increase = page.locator('.sp-weight-controls button[aria-label^="Increase"]');
      await page.locator('.sp-weight-controls:not([hidden])').waitFor({ state: 'attached' });
      const arrow = await increase.boundingBox();
      if (!arrow) throw new Error('Missing increase arrow');
      await page.mouse.move(arrow.x + arrow.width / 2, arrow.y + arrow.height / 2);
      await page.evaluate(() => window.promptWrapProof.states());
      for (const weight of ['-1.00', '-0.95', '-0.90', '-0.85']) {
        await page.mouse.down();
        await page.mouse.up();
        await expectSource(page, weighted(weight));
        await settle(page);
        // Every drawn state, including the one before wrap layout re-measures, keeps the row.
        const states = await page.evaluate(() => window.promptWrapProof.states());
        expect(states.length).toBeGreaterThan(0);
        expect(states.filter((state) => state.segment !== initial.segment)).toEqual([]);
        expect(states.at(-1)?.weight).toBe(weight);
        // The arrows stay visible on the stepped number, so the next click steps it again.
        const geometry = await page.evaluate(() => {
          const root = document.querySelector<HTMLElement>('.sp-weight-controls');
          const button = root?.querySelector('button')?.getBoundingClientRect();
          const value = [...document.querySelectorAll('.cm-content .sp-emphasis-weight')]
            .at(-1)
            ?.getBoundingClientRect();
          return {
            hidden: root?.hidden ?? true,
            button: button && { left: button.left, right: button.right, bottom: button.bottom },
            value: value && { left: value.left, right: value.right, top: value.top },
          };
        });
        expect(geometry.hidden).toBe(false);
        if (!geometry.button || !geometry.value) throw new Error('Missing arrow geometry');
        expect(geometry.button.right).toBeGreaterThan(geometry.value.left);
        expect(geometry.button.left).toBeLessThan(geometry.value.right);
        expect(Math.abs(geometry.button.bottom - geometry.value.top)).toBeLessThan(8);
      }
      expect(await page.evaluate(() => window.promptBrowserProof.read().errors)).toEqual([]);
    } finally {
      await page.close();
    }
  },
  90_000,
);
