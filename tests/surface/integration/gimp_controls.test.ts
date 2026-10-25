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
/** Exercise real GIMP Vue controls and native interactions in headless Chromium and Firefox. */
import { chromium, firefox, type Browser, type Page } from '@playwright/test';
import { build } from 'esbuild';
import { afterAll, beforeAll, beforeEach, describe, expect, test } from '@jest/globals';
import type {} from '../support/GimpControlFixture.js';
import { gimpAppearanceCss } from '../../../frontend/comfyui/ui/surface/appearance/GimpAppearanceStyles.js';
import { gimpTokenDeclarations } from '../../../frontend/comfyui/ui/surface/appearance/GimpThemeTokens.js';
import { gimpIconCss } from '../../../frontend/comfyui/ui/surface/appearance/GimpIconStyles.js';
import { closeBrowserFixture } from '../../frontend_core/support/BrowserFixtureLifetime.js';

let source: string;
const address = 'data:text/html,' + encodeURIComponent('<!doctype html><html><body></body></html>');
const browsers: Browser[] = [];
const retirements: Array<Promise<PromiseSettledResult<void>[]>> = [];

testInBrowsers('swaps GTK3 Adwaita icons only while GIMP is active in browser %s', async (page) => {
  try {
    await page.setContent(
      '<!doctype html><html><body><span id="card" class="sugarcubes-workflow-masonry__node-icon" data-masonry-node-icon="edit" style="--sc-node-icon:url(https://example.invalid/default.svg)"></span><span id="negative" class="sugarcubes-workflow-masonry__node-icon" data-masonry-node-icon="eraser" style="--sc-node-icon:url(https://example.invalid/eraser.svg)"></span><section data-sc-style-surface><button id="action"><i id="owned" class="pi pi-cog"></i></button><img id="authored" src="data:image/svg+xml,%3Csvg xmlns=%22http://www.w3.org/2000/svg%22/%3E"></section><span data-sugarcubes-save-cube-icon><i id="save" class="icon-[lucide--save]" style="display:inline-block;width:10px;height:10px"></i></span><button><i id="outside" class="pi pi-cog"></i></button></body></html>',
    );
    await page.addStyleTag({
      content:
        '.sugarcubes-workflow-masonry__node-icon::before {content:"";mask-image:var(--sc-node-icon);width:20px} .pi {display:inline-block;mask-image:url(https://example.invalid/original.svg)} [data-sugarcubes-save-cube-icon] i {mask-image:url(https://example.invalid/save.svg)} .pi::before {content:"original"}',
    });
    await page.addStyleTag({ content: gimpIconCss() });
    const baseline = await page.evaluate(() => ({
      card: getComputedStyle(document.getElementById('card')!, '::before').maskImage,
      negative: getComputedStyle(document.getElementById('negative')!, '::before').maskImage,
      owned: getComputedStyle(document.getElementById('owned')!).maskImage,
      save: getComputedStyle(document.getElementById('save')!).maskImage,
      outside: getComputedStyle(document.getElementById('outside')!).maskImage,
      authored: getComputedStyle(document.getElementById('authored')!).maskImage,
    }));
    const actionWidth = await page
      .locator('#owned')
      .evaluate((icon) => icon.getBoundingClientRect().width);
    for (const theme of ['gimp', 'fluent', 'spectrum', 'photopro', '', 'gimp']) {
      await page.evaluate((value) => {
        document.documentElement.dataset.scVisualStyle = value;
      }, theme);
      const paint = await page.evaluate(() => ({
        card: getComputedStyle(document.getElementById('card')!, '::before').maskImage,
        negative: getComputedStyle(document.getElementById('negative')!, '::before').maskImage,
        owned: getComputedStyle(document.getElementById('owned')!).maskImage,
        save: getComputedStyle(document.getElementById('save')!).maskImage,
        outside: getComputedStyle(document.getElementById('outside')!).maskImage,
        authored: getComputedStyle(document.getElementById('authored')!).maskImage,
      }));
      if (theme === 'gimp') {
        expect(paint.card).toContain('/icons/node/gimp/document-edit-symbolic.svg');
        expect(paint.negative).toContain('/icons/node/gimp/edit-clear-all-symbolic.svg');
        expect(paint.owned).toContain('/icons/node/gimp/preferences-system-symbolic.svg');
        expect(paint.save).toContain('/icons/node/gimp/document-save-symbolic.svg');
        expect(paint.outside).toBe(baseline.outside);
        expect(paint.authored).toBe(baseline.authored);
      } else expect(paint).toEqual(baseline);
      expect(
        await page.evaluate(() => ({
          card: getComputedStyle(document.getElementById('card')!, '::before').width,
          save: document.getElementById('save')!.getBoundingClientRect().width,
        })),
      ).toEqual({ card: '20px', save: 10 });
      expect(
        await page.locator('#owned').evaluate((icon) => icon.getBoundingClientRect().width),
      ).toBe(actionWidth);
    }
    expect(await page.locator('#action').count()).toBe(1);
    expect(await page.locator('#card').getAttribute('style')).toContain('default.svg');
  } finally {
    await disposePage(page);
  }
});
testInBrowsers(
  'separates GTK menu glyphs from labels without resizing slots in browser %s',
  async (page) => {
    try {
      await page.setContent(
        '<!doctype html><html><body><div data-sc-theme="gimp" data-sc-menu-surface="actions" role="menu"><div role="menuitem"><i id="rename" aria-hidden="true" class="pi pi-pencil" style="width:16px;height:16px"></i><span id="rename-label">Rename</span></div><div role="menuitem"><i id="capture" aria-hidden="true" class="pi pi-save" style="width:16px;height:16px"></i><span id="capture-label">Capture Cube</span></div></div></body></html>',
      );
      await page.addStyleTag({ content: '.pi{display:inline-block;font-size:16px}' });
      await page.addStyleTag({ content: gimpAppearanceCss(true) });
      await page.evaluate((tokens) => {
        document.documentElement.dataset.scVisualStyle = 'gimp';
        document.body.style.cssText = tokens;
      }, gimpTokenDeclarations(true));
      const paint = await page.evaluate(() =>
        ['rename', 'capture'].map((id) => {
          const icon = document.getElementById(id)!;
          const label = document.getElementById(`${id}-label`)!;
          const iconBox = icon.getBoundingClientRect();
          const labelBox = label.getBoundingClientRect();
          return {
            width: iconBox.width,
            height: iconBox.height,
            gap: labelBox.left - iconBox.right,
            centers: labelBox.top + labelBox.height / 2 - (iconBox.top + iconBox.height / 2),
            mask: getComputedStyle(icon).maskImage,
          };
        }),
      );
      expect(paint).toHaveLength(2);
      for (const row of paint) {
        expect(row.width).toBe(16);
        expect(row.height).toBe(16);
        expect(row.gap).toBe(6);
        expect(row.centers).toBeCloseTo(0);
      }
      expect(paint[0]?.mask).toContain('/gimp/document-edit-symbolic.svg');
      expect(paint[1]?.mask).toContain('/gimp/document-save-symbolic.svg');
    } finally {
      await disposePage(page);
    }
  },
);

beforeAll(async () => {
  const output = await build({
    stdin: {
      contents:
        "import {mountGimpControlFixture} from './tests/surface/support/GimpControlFixture.ts';mountGimpControlFixture();",
      resolveDir: process.cwd(),
    },
    bundle: true,
    write: false,
    format: 'iife',
    define: {
      'process.env.NODE_ENV': '"production"',
      __VUE_OPTIONS_API__: 'false',
      __VUE_PROD_DEVTOOLS__: 'false',
      __VUE_PROD_HYDRATION_MISMATCH_DETAILS__: 'false',
    },
  });
  source = output.outputFiles[0]!.text;
}, 30_000);

// Keep the measured Windows Firefox shutdown outside each UI case's 30-second budget.
afterAll(async () => {
  const releases = await Promise.all([
    ...retirements,
    Promise.allSettled(browsers.map((browser) => closeBrowserFixture(browser, undefined))),
  ]);
  const failures = releases.flatMap((results) =>
    results.flatMap((result): unknown[] => (result.status === 'rejected' ? [result.reason] : [])),
  );
  if (failures.length) throw new AggregateError(failures, 'GIMP browser fixture cleanup failed');
}, 60_000);

/** Isolate each fixture from Firefox context retirement and navigation lifecycles. */
async function openBrowser(index: number): Promise<Browser> {
  const browser = await (index === 0 ? chromium : firefox).launch({
    headless: true,
    timeout: 20_000,
    // Ignore the desktop user's modifier keys when starting an isolated browser.
    env: { ...process.env, MOZ_DISABLE_SAFE_MODE_KEY: '1' },
  });
  browsers.push(browser);
  return browser;
}

/** Give each test a fresh private page and JavaScript realm. */
async function openPage(index: number): Promise<Page> {
  const page = await (await openBrowser(index)).newPage();
  page.setDefaultTimeout(10_000);
  await page.goto(address);
  await page.bringToFront();
  return page;
}

/** Retire the private browser immediately and verify shutdown with the suite lifetime. */
async function disposePage(page: Page): Promise<void> {
  const browser = page.context().browser();
  if (!browser) throw new Error('The private fixture browser is unavailable.');
  try {
    await page.evaluate(() => window.gimpFixture?.dispose());
  } finally {
    retirements.push(Promise.allSettled([browser.close()]));
  }
}

/** Budget engine startup independently while retaining the 30-second native interaction deadline. */
function testInBrowsers(name: string, run: (page: Page) => Promise<void>): void {
  for (const index of [0, 1]) {
    describe(`browser ${String(index)}`, () => {
      let page: Page | undefined;
      beforeEach(async () => {
        page = await openPage(index);
      }, 45_000);
      test(
        name.replace('%s', String(index)),
        async () => {
          if (!page) throw new Error('The private browser fixture did not start.');
          await run(page);
        },
        30_000,
      );
    });
  }
}

testInBrowsers('keeps real native values, focus and literal text in browser %s', async (page) => {
  const errors: string[] = [];
  page.on('pageerror', (error) => errors.push(error.message));
  try {
    await page.setContent('<!doctype html><html><body></body></html>');
    await page.addScriptTag({ content: source });
    expect(errors).toEqual([]);
    expect(await page.locator('#scale .spin-scale-label').textContent()).toBe('Denoise');
    expect(
      await page.locator('#scale input').evaluate((input) => input.getBoundingClientRect().width),
    ).toBeGreaterThan(0);
    expect(await page.locator('#enabled input').getAttribute('aria-label')).toBe('Enabled');
    expect(await page.locator('#command').innerText()).toBe('<b>Literal command</b>');
    expect(await page.locator('#command b').count()).toBe(0);
    await page.locator('#enabled input').check();
    expect(
      await page.evaluate(() => [
        window.gimpFixture.checkbox.checked,
        window.gimpFixture.edits.checkbox,
      ]),
    ).toEqual([true, 1]);
    await page.locator('#search input').fill('<svg>literal query</svg>');
    expect(await page.evaluate(() => window.gimpFixture.search.value)).toBe(
      '<svg>literal query</svg>',
    );
    await page.locator('#scale input').focus();
    await page.keyboard.press('ArrowRight');
    expect(await page.evaluate(() => window.gimpFixture.slider.readValue())).toBeCloseTo(0.61);
    expect(await page.evaluate(() => window.gimpFixture.edits.slider)).toBe(1);
    await page.evaluate(() => {
      window.gimpFixture.slider.synchronize(0, 1, 0.01, false, 0.2);
    });
    expect(await page.locator('#scale input').inputValue()).toBe('0.2');
    expect(await page.evaluate(() => window.gimpFixture.edits.slider)).toBe(1);
    await page.locator('#search input').focus();
    await page.keyboard.press('Tab');
    expect(
      await page
        .locator('#segments')
        .evaluate(
          (group) => document.activeElement === group.querySelector('[aria-selected="true"]'),
        ),
    ).toBe(true);
    await page.keyboard.press('Tab');
    expect(
      await page.evaluate(() => document.activeElement === window.gimpFixture.prompt.contentDOM),
    ).toBe(true);
    await page.locator('#segments').evaluate((group) => {
      const tab = group.querySelector<HTMLElement>('[role="tab"]');
      if (!tab) throw new Error('The native tab group did not mount.');
      tab.focus();
    });
    await page.keyboard.press('ArrowRight');
    expect(
      await page
        .locator('#segments')
        .evaluate((group) => group.querySelector('[aria-selected="true"]')?.textContent),
    ).toBe('On');
    await page.evaluate(() => {
      window.gimpFixture.checkbox.disabled = true;
    });
    expect(await page.locator('#enabled input').isDisabled()).toBe(true);
    await page.locator('#search input').focus();
    await page.evaluate(() => {
      document.body.style.setProperty('--sc-gimp-widget', '#dcdcdc');
    });
    expect(
      await page.locator('#search input').evaluate((input) => {
        const root = input.getRootNode();
        return root instanceof ShadowRoot && input === root.activeElement;
      }),
    ).toBe(true);
    expect(await page.locator('#search input').inputValue()).toBe('<svg>literal query</svg>');
    expect(await page.locator('#editor').textContent()).toBe('retained document');
  } finally {
    await disposePage(page);
  }
});

testInBrowsers(
  'insets real prompt text and retains editing after palette changes in browser %s',
  async (page) => {
    try {
      await page.setContent('<!doctype html><html><body></body></html>');
      await page.addScriptTag({ content: source });
      await page.addStyleTag({ content: gimpAppearanceCss(true) });
      await page.locator('[data-sc-editor-field="gimp"]').evaluate((field) => {
        field.style.width = '300px';
        field.style.height = '120px';
      });
      for (const dark of [true, false, true]) {
        await page.evaluate((tokens) => {
          document.body.style.cssText = tokens;
        }, gimpTokenDeclarations(dark));
        const padding = await page.locator('#editor').evaluate((content) => {
          const style = getComputedStyle(content);
          return [style.paddingTop, style.paddingRight, style.paddingBottom, style.paddingLeft];
        });
        expect(padding).toEqual(['4px', '6px', '4px', '6px']);
        await page.locator('#editor').focus();
        await page.keyboard.press('Control+End');
        await page.keyboard.type(' edited');
        expect(await page.evaluate(() => window.gimpFixture.prompt.state.doc.toString())).toContain(
          'retained document edited',
        );
        expect(
          await page.locator('#editor').evaluate((content) => {
            const frame = content.closest('[data-sc-editor-field="gimp"]');
            const line = content.querySelector('.cm-line');
            if (!frame || !line) throw new Error('The real prompt field did not mount.');
            return line.getBoundingClientRect().left - frame.getBoundingClientRect().left;
          }),
        ).toBeGreaterThanOrEqual(6);
      }
    } finally {
      await disposePage(page);
    }
  },
);

testInBrowsers(
  'scrubs an actual spin scale absolutely and relatively at fractional scale in browser %s',
  async (page) => {
    try {
      await page.setContent('<!doctype html><html><body></body></html>');
      await page.addScriptTag({ content: source });
      await page.locator('#scale').evaluate((scale) => {
        scale.style.width = '240px';
        scale.style.transform = 'scale(.85)';
        scale.style.transformOrigin = '0 0';
      });
      const input = page.locator('#scale input');
      const bounds = await input.boundingBox();
      if (!bounds) throw new Error('The real spin scale did not mount.');
      const y = bounds.y + bounds.height / 2;
      await page.mouse.click(bounds.x + bounds.width * 0.5, y);
      const start = await page.evaluate(() => window.gimpFixture.slider.readValue());
      expect(start).toBeCloseTo(0.5, 1);
      await page.keyboard.down('Shift');
      await page.mouse.move(bounds.x + bounds.width * 0.2, y);
      await page.mouse.down();
      expect(await page.evaluate(() => window.gimpFixture.slider.readValue())).toBe(start);
      await page.mouse.move(bounds.x + bounds.width * 0.7, y, { steps: 4 });
      await page.mouse.up();
      await page.keyboard.up('Shift');
      expect(await page.evaluate(() => window.gimpFixture.slider.readValue())).toBeCloseTo(
        start + 0.05,
        2,
      );
      await page.mouse.move(bounds.x + bounds.width * 0.2, y);
      await page.mouse.down({ button: 'right' });
      await page.mouse.move(bounds.x + bounds.width * 0.7, y, { steps: 4 });
      await page.mouse.up({ button: 'right' });
      expect(await page.evaluate(() => window.gimpFixture.slider.readValue())).toBeCloseTo(
        start + 0.1,
        2,
      );
      await page.evaluate(() => window.gimpFixture.slider.synchronize(0, 1, 0.01, true, 0.4));
      expect(await input.isDisabled()).toBe(true);
      expect(await input.inputValue()).toBe('0.4');
    } finally {
      await disposePage(page);
    }
  },
);

testInBrowsers('keeps entries and steppers on one geometry source in browser %s', async (page) => {
  try {
    await page.addScriptTag({ content: source });
    await page.addStyleTag({ content: gimpAppearanceCss(true) });
    for (const height of [22, 26, 34]) {
      await page.evaluate((height) => {
        const node = document.getElementById('numeric-node');
        if (!node) throw new Error('The numeric fixture did not mount.');
        node.style.setProperty('--sc-control-height', `${String(height)}px`);
        node.style.transform = 'scale(.85)';
        node.style.transformOrigin = '0 0';
      }, height);
      for (const id of ['numeric-size', 'numeric-scale', 'numeric-slider']) {
        const boxes = await page.evaluate((id) => {
          const row = document.getElementById(id);
          if (!row) throw new Error(`The numeric row ${id} did not mount.`);
          const entry =
            row.querySelector('sc-gimp-scale')?.shadowRoot?.querySelector('.spin-scale') ??
            row.querySelector('[data-numeric-entry]');
          const decrement = row.querySelector('[data-sc-number-decrement]');
          const increment = row.querySelector('[data-sc-number-increment]');
          if (!entry || !decrement || !increment) throw new Error('A numeric group did not mount.');
          return [entry, decrement, increment].map((element) => {
            const rect = element.getBoundingClientRect();
            return { top: rect.top, bottom: rect.bottom, height: rect.height };
          });
        }, id);
        for (const box of boxes) {
          expect(box.height).toBeCloseTo(height * 0.85, 1);
          expect(box.top).toBeCloseTo(boxes[0]!.top, 1);
          expect(box.bottom).toBeCloseTo(boxes[0]!.bottom, 1);
        }
      }
    }
  } finally {
    await disposePage(page);
  }
});

testInBrowsers(
  'presents Denoise as a visible slider with a native spinner in browser %s',
  async (page) => {
    try {
      await page.addScriptTag({ content: source });
      await page.addStyleTag({ content: gimpAppearanceCss(true) });
      expect(
        await page.evaluate(
          () => document.querySelector('#numeric-slider sc-gimp-slider') !== null,
        ),
      ).toBe(true);
      expect(await page.locator('#numeric-slider sc-gimp-scale').count()).toBe(0);
      expect(await page.locator('#numeric-slider [data-sc-number-input]').count()).toBe(1);
      expect(await page.locator('#numeric-slider [data-sc-number-decrement]').count()).toBe(1);
      expect(await page.locator('#numeric-slider [data-sc-number-increment]').count()).toBe(1);
      const range = page.locator('#numeric-slider sc-gimp-slider input');
      expect(await range.isVisible()).toBe(true);
      expect(await range.evaluate((input) => getComputedStyle(input).opacity)).toBe('1');
      await range.focus();
      await page.keyboard.press('ArrowRight');
      expect(Number(await range.inputValue())).toBeCloseTo(0.76);
      expect(
        Number(await page.locator('#numeric-slider [data-sc-number-input]').inputValue()),
      ).toBeCloseTo(0.76);
      const track = await range.boundingBox();
      if (!track) throw new Error('The visible slider track did not mount.');
      expect(track.width).toBeCloseTo(120, 1);
      await page.evaluate(() => {
        const node = document.getElementById('numeric-node');
        if (!node) throw new Error('The numeric fixture did not mount.');
        node.style.width = '480px';
      });
      const widerTrack = await range.boundingBox();
      expect(widerTrack?.width).toBeCloseTo(track.width, 1);
      await page.evaluate(() =>
        document.getElementById('numeric-node')?.style.removeProperty('width'),
      );
      await page.mouse.move(track.x + 10, track.y + track.height / 2);
      await page.mouse.down();
      await page.mouse.move(track.x + track.width / 2, track.y + track.height / 2, { steps: 5 });
      await page.mouse.up();
      expect(Number(await range.inputValue())).toBeCloseTo(0.5, 1);
      expect(
        Number(await page.locator('#numeric-slider [data-sc-number-input]').inputValue()),
      ).toBeCloseTo(0.5, 1);
    } finally {
      await disposePage(page);
    }
  },
);

testInBrowsers(
  'fills the entry continuously with GIMP materials at exact fractions in browser %s',
  async (page) => {
    try {
      await page.addScriptTag({ content: source });
      await page.locator('#scale').evaluate((element) => {
        element.style.width = '240px';
      });
      for (const dark of [true, false]) {
        await page.evaluate((tokens) => {
          document.body.style.cssText = tokens;
        }, gimpTokenDeclarations(dark));
        for (const fraction of [0, 0.25, 0.75, 1]) {
          await page.evaluate(
            (value) => window.gimpFixture.slider.synchronize(0, 1, 0.01, false, value),
            fraction,
          );
          const paint = await page.locator('#scale .spin-scale-progress').evaluate((fill) => {
            const entry = fill.parentElement;
            if (!entry) throw new Error('The actual spin-scale entry did not mount.');
            const style = getComputedStyle(fill);
            return {
              width: fill.getBoundingClientRect().width,
              entryWidth: entry.clientWidth,
              color: style.backgroundColor,
              image: style.backgroundImage,
              radius: style.borderTopRightRadius,
              entryColor: getComputedStyle(entry).backgroundColor,
            };
          });
          expect(paint.width).toBeCloseTo(paint.entryWidth * fraction, 0);
          expect(paint.color).toBe(dark ? 'rgb(100, 100, 100)' : 'rgb(220, 220, 220)');
          expect(paint.entryColor).toBe(dark ? 'rgb(42, 42, 42)' : 'rgb(250, 250, 250)');
          expect(paint.image).toBe('none');
          expect(paint.radius).toBe('0px');
        }
      }
    } finally {
      await disposePage(page);
    }
  },
);

testInBrowsers(
  'matches GTK panel geometry and switches palette without remounting in browser %s',
  async (page) => {
    try {
      await page.setContent(
        '<!doctype html><html><body><main class="sugarcubes-workflow-masonry" data-sc-style-surface><header class="sugarcubes-workflow-masonry__section"><h2>Workflow nodes</h2></header></main><article data-sc-style-node><header data-testid="node-header-preview"><span data-testid="node-title">GTK node</span></header></article></body></html>',
      );
      await page.addScriptTag({ content: source });
      await page.addStyleTag({ content: gimpAppearanceCss(true) });
      expect(
        await page.locator('h2').evaluate((element) => getComputedStyle(element).fontSize),
      ).toBe('12px');
      expect(
        await page
          .locator('[data-testid="node-header-preview"]')
          .evaluate((element) => element.getBoundingClientRect().height),
      ).toBe(32);
      expect(
        await page
          .locator('#search input')
          .evaluate((element) => getComputedStyle(element).backgroundColor),
      ).toBe('rgb(42, 42, 42)');
      await page.locator('#search input').focus();
      await page.evaluate((tokens) => {
        document.body.style.cssText = tokens;
      }, gimpTokenDeclarations(false));
      expect(
        await page
          .locator('#search input')
          .evaluate((element) => getComputedStyle(element).backgroundColor),
      ).toBe('rgb(220, 220, 220)');
      expect(await page.locator('#search input').inputValue()).toBe('retained query');
      expect(
        await page.locator('#search input').evaluate((element) => {
          const root = element.getRootNode();
          return root instanceof ShadowRoot && root.activeElement === element;
        }),
      ).toBe(true);
    } finally {
      await disposePage(page);
    }
  },
);
