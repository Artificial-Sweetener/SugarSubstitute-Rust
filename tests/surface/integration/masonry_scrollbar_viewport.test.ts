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
/** Prove native scrolling cannot move masonry's track before JavaScript catches up. */
import { build } from 'esbuild';
import { chromium, type Browser, type Page } from '@playwright/test';

let browser: Browser;
let fixture: string;

beforeAll(async () => {
  browser = await chromium.launch({ headless: true });
  const output = await build({
    stdin: {
      contents: `import {OverlayScrollbarHost} from './frontend/comfyui/ui/surface/appearance/OverlayScrollbarHost.ts';import {ensureWorkflowMasonryStyles} from './frontend/comfyui/ui/surface/masonry/WorkflowMasonryStyles.ts';import {scrollbarCss} from './frontend/comfyui/ui/surface/appearance/ScrollbarStyles.ts';ensureWorkflowMasonryStyles(document);const style=document.createElement('style');style.textContent=scrollbarCss(true);document.head.append(style);const host=new OverlayScrollbarHost(document,error=>{throw error});host.enable();`,
      resolveDir: process.cwd(),
    },
    bundle: true,
    write: false,
    format: 'iife',
  });
  fixture = output.outputFiles[0]!.text;
});

afterAll(async () => {
  await browser.close();
});

/** Mount the actual masonry styles and scrollbar owner inside a fixed panel viewport. */
async function mount(page: Page, corner: number, scale = 1): Promise<void> {
  await page.setContent(
    `<!doctype html><html data-sc-visual-style="photopro" style="--sc-panel-resize-corner:${corner}px"><body style="margin:0"><main style="position:relative;width:480px;height:640px;transform:scale(${scale});transform-origin:0 0"><div id="masonry" class="sugarcubes-workflow-masonry"><div class="sugarcubes-workflow-masonry__content" style="height:3200px">Cards</div></div></main></body></html>`,
  );
  await page.addScriptTag({ content: fixture });
  await page.waitForSelector(
    '[aria-controls="masonry"][aria-orientation="vertical"]:not([hidden])',
  );
  await page.waitForTimeout(50);
}

test.each([1, 0.85])(
  'keeps the track fixed before deferred scroll updates at scale %s',
  async (scale) => {
    const page = await browser.newPage();
    try {
      await mount(page, 0, scale);
      const positions = await page.evaluate(() => {
        const owner = document.querySelector<HTMLElement>('#masonry')!;
        const track = document.querySelector(
          '[aria-controls="masonry"] .sc-style-scrollbar-track',
        )!;
        const maximum = owner.scrollHeight - owner.clientHeight;
        window.requestAnimationFrame = () => 1;
        return [0, 120, 640, maximum, 300, 0].map((position) => {
          owner.scrollTop = position;
          const rectangle = track.getBoundingClientRect();
          return {
            top: rectangle.top,
            bottom: rectangle.bottom,
            maximum: owner.scrollHeight - owner.clientHeight,
          };
        });
      });
      for (const position of positions) {
        expect(position.top).toBeCloseTo(positions[0]!.top, 2);
        expect(position.bottom).toBeCloseTo(positions[0]!.bottom, 2);
        expect(position.maximum).toBe(positions[0]!.maximum);
      }
    } finally {
      await page.close();
    }
  },
);

test('keeps the bottom arrow clear of the resize corner without shortening the masonry viewport', async () => {
  const page = await browser.newPage();
  try {
    await mount(page, 24);
    const geometry = await page.evaluate(() => {
      const owner = document.querySelector('#masonry')!,
        track = document.querySelector('[aria-controls="masonry"] .sc-style-scrollbar-track')!,
        arrow = track.querySelector('[data-sc-scroll-step="end"]')!;
      return {
        viewportBottom: owner.getBoundingClientRect().bottom,
        trackBottom: track.getBoundingClientRect().bottom,
        arrowBottom: arrow.getBoundingClientRect().bottom,
      };
    });
    expect(geometry.viewportBottom).toBe(640);
    expect(geometry.trackBottom).toBe(616);
    expect(geometry.arrowBottom).toBe(616);
  } finally {
    await page.close();
  }
});

test('retains arrows, wheel, dragging and keyboard control on the stationary track', async () => {
  const page = await browser.newPage();
  try {
    await mount(page, 24);
    const thumb = page.locator('[aria-controls="masonry"][aria-orientation="vertical"]');
    await thumb.locator('[data-sc-scroll-step="end"]').click();
    await page.waitForFunction(() => document.querySelector('#masonry')!.scrollTop === 40);
    await thumb.hover();
    await page.mouse.wheel(0, 200);
    await page.waitForFunction(() => document.querySelector('#masonry')!.scrollTop > 40);
    const before = await page.locator('#masonry').evaluate((e) => e.scrollTop);
    const bounds = await thumb.boundingBox();
    if (!bounds) throw new Error('Missing stationary thumb');
    await page.mouse.move(bounds.x + bounds.width / 2, bounds.y + bounds.height / 2);
    await page.mouse.down();
    await page.mouse.move(bounds.x + bounds.width / 2, bounds.y + bounds.height / 2 + 80, {
      steps: 5,
    });
    await page.mouse.up();
    expect(await page.locator('#masonry').evaluate((e) => e.scrollTop)).toBeGreaterThan(before);
    await thumb.focus();
    await page.keyboard.press('End');
    await page.waitForFunction(() => {
      const owner = document.querySelector('#masonry')!;
      return owner.scrollTop === owner.scrollHeight - owner.clientHeight;
    });
    expect(
      await thumb
        .locator('[data-sc-scroll-step="end"]')
        .evaluate((e) => e.getBoundingClientRect().bottom),
    ).toBeCloseTo(616, 1);
    await page.evaluate(() => {
      document.documentElement.style.setProperty('--sc-panel-resize-corner', '0px');
      window.dispatchEvent(new Event('resize'));
    });
    await page.waitForFunction(
      () =>
        Math.abs(
          document
            .querySelector('[aria-controls="masonry"] .sc-style-scrollbar-track')!
            .getBoundingClientRect().bottom - 640,
        ) < 0.05,
    );
  } finally {
    await page.close();
  }
});

test('keeps PhotoPro scroll arrows unchanged under hover', async () => {
  const page = await browser.newPage();
  try {
    await mount(page, 0);
    await page.locator('#masonry').evaluate((e) => (e.scrollTop = 500));
    await page.waitForTimeout(50);
    for (const direction of ['start', 'end']) {
      const button = page.locator(
        `[aria-controls="masonry"][aria-orientation="vertical"] [data-sc-scroll-step="${direction}"]`,
      );
      const before = await button.evaluate((e) => ({
        fill: getComputedStyle(e).backgroundColor,
        color: getComputedStyle(e).color,
        border: getComputedStyle(e).borderColor,
      }));
      await button.hover();
      expect(
        await button.evaluate((e) => ({
          fill: getComputedStyle(e).backgroundColor,
          color: getComputedStyle(e).color,
          border: getComputedStyle(e).borderColor,
        })),
      ).toEqual(before);
      await page.mouse.move(700, 700);
    }
  } finally {
    await page.close();
  }
});
