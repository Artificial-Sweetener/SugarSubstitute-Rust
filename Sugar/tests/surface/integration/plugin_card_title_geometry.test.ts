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
/** Prove PhotoPro card geometry with actual browser layout and shared production styles. */
import { chromium, type Browser } from '@playwright/test';
import {
  BROWSER_FIXTURE_SETUP_TIMEOUT_MS,
  launchHeadlessBrowser,
} from '../../frontend_core/support/BrowserFixtureLifetime.js';
import { JSDOM } from 'jsdom';
import { ensureRelationStyles } from '../../../frontend/comfyui/ui/relations/presentation/RelationStyles.js';
import { photoProAppearanceCss } from '../../../frontend/comfyui/ui/surface/appearance/PhotoProAppearanceStyles.js';

let browser: Browser;

beforeAll(async () => {
  browser = await launchHeadlessBrowser(chromium);
}, BROWSER_FIXTURE_SETUP_TIMEOUT_MS);

afterAll(async () => {
  await browser.close();
}, 30_000);

test('keeps the override pin at the top right across card widths with the activation footprint', async () => {
  const dom = new JSDOM();
  ensureRelationStyles(dom.window.document);
  const relationCss = dom.window.document.getElementById('sc-relation-styles')!.textContent!;
  const page = await browser.newPage();
  try {
    await page.setContent(`<!doctype html><html data-sc-visual-style="photopro"><body>
      <main style="width:260px"><div class="sc-overrides-toolbar" data-presentation="plugin" data-sc-theme="photopro">
        <button class="sc-overrides-menu"><span></span><span></span></button>
        <h3 class="sc-overrides-card-title">Global Overrides</h3>
        <div class="sc-overrides-fields">Fields</div>
      </div></main>
      <div data-sc-titlebar-controls><div style="display:flex;align-items:center">
        <span id="node-title">Enabled Node</span>
        <div data-cube-card-activation><input type="checkbox" data-sc-boolean-checkbox></div>
      </div></div>
    </body></html>`);
    await page.addStyleTag({ content: relationCss + photoProAppearanceCss() });
    for (const width of [260, 480, 800]) {
      await page.locator('main').evaluate((main, cardWidth) => {
        main.style.width = `${String(cardWidth)}px`;
      }, width);
      const geometry = await page.evaluate(() => {
        const pin = document.querySelector('.sc-overrides-menu')!;
        const title = document.querySelector('.sc-overrides-card-title')!;
        const checkbox = document.querySelector('[data-sc-boolean-checkbox]')!;
        const nodeTitle = document.querySelector('#node-title')!;
        const pinBounds = pin.getBoundingClientRect();
        const titleBounds = title.getBoundingClientRect();
        const checkboxBounds = checkbox.getBoundingClientRect();
        return {
          pinSize: [pinBounds.width, pinBounds.height],
          checkboxSize: [checkboxBounds.width, checkboxBounds.height],
          rightInset:
            pin.closest('.sc-overrides-toolbar')!.getBoundingClientRect().right - pinBounds.right,
          titleBeforePin: titleBounds.right <= pinBounds.left,
          checkboxGap: checkboxBounds.left - nodeTitle.getBoundingClientRect().right,
          centered:
            pinBounds.top + pinBounds.height / 2 === titleBounds.top + titleBounds.height / 2,
          chevron: getComputedStyle(pin.lastElementChild!).display,
        };
      });
      expect(geometry.pinSize).toEqual([14, 14]);
      expect(geometry.pinSize).toEqual(geometry.checkboxSize);
      expect(geometry.rightInset).toBe(11);
      expect(geometry.titleBeforePin).toBe(true);
      expect(geometry.checkboxGap).toBe(8);
      expect(geometry.centered).toBe(true);
      expect(geometry.chevron).toBe('none');
    }
    await page.evaluate(() => document.documentElement.removeAttribute('data-sc-visual-style'));
    expect(
      await page.locator('.sc-overrides-menu').evaluate((pin) => getComputedStyle(pin).width),
    ).toBe('40px');
  } finally {
    await page.close();
    dom.window.close();
  }
});

test('fills the available override card width and wraps the same controls into any fitting row count', async () => {
  const dom = new JSDOM();
  ensureRelationStyles(dom.window.document);
  const page = await browser.newPage();
  try {
    await page.setContent(`<!doctype html><html data-sc-visual-style="photopro"><body>
      <main style="width:260px"><div class="sc-overrides-toolbar sugarcubes-cube-face__node-card" data-presentation="plugin">
        <h3 class="sc-overrides-card-title">Global Overrides</h3>
        <div class="sc-overrides-fields">${Array.from(
          { length: 5 },
          (_, index) => `
          <div class="sc-override-control" data-sc-style-row data-sc-style-surface style="--sc-override-label-width:32px">
            <div><div data-sc-style-layout><label data-testid="widget-layout-field-label">Field</label><input value="${String(index)}"></div></div>
          </div>`,
        ).join('')}
        </div>
      </div></main>
      <div class="sugarcubes-cube-face__node-card"><div data-sc-style-node><div id="normal-row" data-sc-style-row></div></div></div>
      </body></html>`);
    await page.addStyleTag({
      content:
        dom.window.document.getElementById('sc-relation-styles')!.textContent! +
        photoProAppearanceCss(),
    });
    for (const columns of [1, 2, 3, 4, 5, 1]) {
      await page.locator('main').evaluate((main, width) => {
        main.style.width = `${String(width)}px`;
      }, columns * 260);
      const geometry = await page.locator('.sc-overrides-toolbar').evaluate((card) => {
        const fields = [...card.querySelectorAll('.sc-override-control')];
        const bounds = fields.map((field) => field.getBoundingClientRect());
        return {
          width: card.getBoundingClientRect().width,
          available: card.parentElement!.getBoundingClientRect().width,
          firstRow: bounds.filter((box) => box.top === bounds[0]!.top).length,
          overflow: card.scrollWidth > card.clientWidth,
          values: [...card.querySelectorAll('input')].map((input) => input.value),
        };
      });
      expect(geometry.width).toBe(geometry.available);
      expect(geometry.firstRow).toBe(columns);
      expect(geometry.overflow).toBe(false);
      expect(geometry.values).toEqual(['0', '1', '2', '3', '4']);
    }
    for (const compact of [false, true]) {
      await page.evaluate(
        (enabled) => document.documentElement.toggleAttribute('data-sc-compact-masonry', enabled),
        compact,
      );
      const density = await page.locator('.sc-override-control').evaluateAll((fields) => ({
        normalPadding: getComputedStyle(document.querySelector('#normal-row')!).paddingTop,
        overrides: fields.map((field) => ({
          padding: getComputedStyle(field).paddingTop,
          border: getComputedStyle(field).borderTopWidth,
        })),
      }));
      for (const field of density.overrides) {
        expect(field.padding).toBe(density.normalPadding);
        expect(field.border).toBe('0px');
      }
    }
  } finally {
    await page.close();
    dom.window.close();
  }
});

test('aligns PhotoPro graph values to the far edge and removes field row dividers', async () => {
  const page = await browser.newPage();
  try {
    await page.setContent(`<!doctype html><html data-sc-visual-style="photopro"><body>
      <style>[data-sc-style-layout] { display:grid;grid-template-columns:50% 50%; }</style>
      <div data-sc-style-node data-sc-style-theme-root style="width:400px"><div data-sc-style-fields>
        ${['Width', 'Height']
          .map(
            (label) => `<div data-sc-style-row data-sc-style-widget="WidgetInputNumber">
          <div data-sc-style-layout><label>${label}</label><div><div><div><input value="1024"></div></div></div></div>
        </div>`,
          )
          .join('')}
      </div></div></body></html>`);
    await page.addStyleTag({ content: photoProAppearanceCss() });
    const rows = await page.locator('[data-sc-style-row]').evaluateAll((elements) =>
      elements.map((row) => {
        const layout = row.querySelector('[data-sc-style-layout]')!;
        const value = layout.lastElementChild!;
        return {
          border: getComputedStyle(row).borderTopWidth,
          endGap: layout.getBoundingClientRect().right - value.getBoundingClientRect().right,
          oppositeEdges:
            value.getBoundingClientRect().left >
            layout.firstElementChild!.getBoundingClientRect().right,
        };
      }),
    );
    for (const row of rows) {
      expect(row.border).toBe('0px');
      expect(row.endGap).toBeLessThanOrEqual(1);
      expect(row.oppositeEdges).toBe(true);
    }
  } finally {
    await page.close();
  }
});
