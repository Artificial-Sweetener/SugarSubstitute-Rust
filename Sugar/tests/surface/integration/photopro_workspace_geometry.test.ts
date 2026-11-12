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
/** Prove compact workspace edges and Photoshop tool states across panel widths. */
import { chromium, type Browser } from '@playwright/test';
import { launchHeadlessBrowser } from '../../frontend_core/support/BrowserFixtureLifetime.js';
import { build } from 'esbuild';
import { photoProAppearanceCss } from '../../../frontend/comfyui/ui/surface/appearance/PhotoProAppearanceStyles.js';
import { photoProTokenDeclarations } from '../../../frontend/comfyui/ui/surface/appearance/PhotoProThemeTokens.js';
import { pluginFrameCss } from '../../../frontend/comfyui/ui/surface/frame/PluginFrameStyles.js';
import { substituteFrameCss } from '../../../frontend/comfyui/ui/surface/frame/SubstituteFrameStyles.js';
import { substituteToolbarCss } from '../../../frontend/comfyui/ui/surface/frame/SubstituteToolbarStyles.js';
import { generationStripCss } from '../../../frontend/comfyui/ui/surface/frame/GenerationStripStyles.js';

let browser: Browser;
let stackStyles: string;
beforeAll(async () => {
  browser = await launchHeadlessBrowser(chromium);
  const output = await build({
    stdin: {
      contents:
        "import {ensureCubeStackStyles} from './frontend/comfyui/ui/surface/stack/CubeStackStyles.ts';import {ensureWorkflowMasonryStyles} from './frontend/comfyui/ui/surface/masonry/WorkflowMasonryStyles.ts';ensureCubeStackStyles(document);ensureWorkflowMasonryStyles(document);",
      resolveDir: process.cwd(),
    },
    bundle: true,
    write: false,
    format: 'iife',
  });
  stackStyles = output.outputFiles[0]!.text;
});
afterAll(async () => {
  await browser.close();
});

test.each([280, 420, 1200])(
  'removes compact padding and right-aligns square generation controls at %spx',
  async (width) => {
    const page = await browser.newPage({ viewport: { width, height: 800 } });
    try {
      await page.setContent(`<!doctype html><html data-sc-visual-style="photopro" data-sc-photopro-palette="dark-gray" data-sc-plugin-mode data-sc-substitute-frame><body data-sc-theme="photopro" data-sc-style-theme-root>
      <div class="p-splitter" id="toolbar-frame"><div data-sc-substitute-toolbar-line><div data-sc-substitute-toolbar data-sc-plugin-toolbar>
        <div class="sc-plugin-toolbar-start"><button class="sc-plugin-settings">Gear</button></div><button id="view">View</button>
        <div id="properties"><button aria-label="Toggle properties panel"><i class="icon-[lucide--panel-right]"></i></button></div>
        <div class="sc-generation-strip"><div class="sc-generation-batch"><input value="1"></div><div class="sc-generation-actions"><button class="sc-generation-action">Cancel</button><button class="sc-generation-action">Run</button><button class="sc-generation-action">Skip</button><span class="sc-generation-queue-slot"></span></div></div>
      </div></div>
      <div id="workspace"><div class="sugarcubes-workflow-masonry" style="--sc-masonry-stack-width:46px"><div class="sugarcubes-workflow-masonry__content">Cards</div></div>
        <aside class="sc-cube-stack-host"><nav class="sc-cube-stack" data-kind="rail" data-compact><ul class="sc-cube-stack__rows" style="height:92px">
          <li class="sc-cube-stack-row" data-selected data-severity="none"><button class="sc-cube-stack-row__activate"><span class="sc-cube-stack-row__icon">One</span></button></li>
          <li class="sc-cube-stack-row" data-severity="none" style="transform:translateY(46px)"><button class="sc-cube-stack-row__activate"><span class="sc-cube-stack-row__icon">Two</span></button></li>
        </ul><button class="sc-cube-stack__add">+</button></nav></aside>
      </div></div></body></html>`);
      await page.addStyleTag({
        content: `*{box-sizing:border-box}html,body{margin:0;height:100%}body{display:flex;flex-direction:column}.p-splitter{border-radius:6px;overflow:hidden;display:flex;flex:1;flex-direction:column}#workspace{position:relative;flex:1}.sugarcubes-workflow-masonry{position:absolute;inset:0;padding:12px 20px 96px 66px}.sc-generation-strip{--sc-accent-color:#d6d6d6}[data-sc-plugin-toolbar]{display:flex}:where(#properties,#properties button,#view){width:32px;height:32px;flex:none} [data-sc-theme="photopro"]{${photoProTokenDeclarations('dark-gray')}}${substituteFrameCss}${substituteToolbarCss}${generationStripCss}${pluginFrameCss}${photoProAppearanceCss()}`,
      });
      await page.addScriptTag({ content: stackStyles });
      const actual = await page.evaluate(() => {
        const bar = document.querySelector('[data-sc-plugin-toolbar]')!,
          host = document.querySelector('.sc-cube-stack-host')!,
          row = document.querySelector('.sc-cube-stack-row')!,
          normal = document.querySelectorAll('.sc-cube-stack-row')[1]!,
          masonry = document.querySelector('.sugarcubes-workflow-masonry')!,
          generation = document.querySelector('.sc-generation-strip')!,
          properties = document.querySelector('#properties')!;
        const rows = document.querySelector('.sc-cube-stack__rows')!,
          button = row.querySelector('button')!,
          icon = row.querySelector('.sc-cube-stack-row__icon')!;
        return {
          toolbarHeight: bar.getBoundingClientRect().height,
          controlHeights: [
            document.querySelector('.sc-plugin-settings')!,
            document.querySelector('#view')!,
            properties.querySelector('button')!,
            generation,
            ...generation.querySelectorAll(
              '.sc-generation-batch,.sc-generation-actions,.sc-generation-action,.sc-generation-queue-slot',
            ),
          ].map((element) => element.getBoundingClientRect().height),
          frameTopCorners: [
            getComputedStyle(document.querySelector('#toolbar-frame')!).borderTopLeftRadius,
            getComputedStyle(document.querySelector('#toolbar-frame')!).borderTopRightRadius,
          ],
          frameBottomCorners: [
            getComputedStyle(document.querySelector('#toolbar-frame')!).borderBottomLeftRadius,
            getComputedStyle(document.querySelector('#toolbar-frame')!).borderBottomRightRadius,
          ],
          hostLeft: host.getBoundingClientRect().left,
          hostTop: host.getBoundingClientRect().top,
          bodyTop: document.querySelector('#workspace')!.getBoundingClientRect().top,
          hostWidth: host.getBoundingClientRect().width,
          rowLeft: row.getBoundingClientRect().left,
          rowsPadding: getComputedStyle(rows).padding,
          buttonPadding: getComputedStyle(button).padding,
          iconWidth: icon.getBoundingClientRect().width,
          cardLeft: document
            .querySelector('.sugarcubes-workflow-masonry__content')!
            .getBoundingClientRect().left,
          masonryPadding: getComputedStyle(masonry).padding,
          masonryFill: getComputedStyle(masonry).backgroundColor,
          toolbarFill: getComputedStyle(bar).backgroundColor,
          divider: getComputedStyle(bar).borderBottomWidth,
          selectedFill: getComputedStyle(row).backgroundColor,
          selectedBorder: getComputedStyle(row).borderColor,
          normalFill: getComputedStyle(normal).backgroundColor,
          toolFill: getComputedStyle(document.querySelector('.sc-plugin-settings')!)
            .backgroundColor,
          toolRadius: getComputedStyle(document.querySelector('.sc-plugin-settings')!).borderRadius,
          generationCorners: [
            ...document.querySelectorAll('.sc-generation-batch,.sc-generation-actions'),
          ].map((e) => getComputedStyle(e).borderRadius),
          right: properties.getBoundingClientRect().right,
          barRight:
            bar.getBoundingClientRect().right - parseFloat(getComputedStyle(bar).paddingRight),
          rightGap:
            properties.getBoundingClientRect().left - generation.getBoundingClientRect().right,
        };
      });
      expect(actual.hostLeft).toBe(0);
      expect(actual.toolbarHeight).toBe(width === 280 ? 65 : 35);
      expect(actual.controlHeights.every((height) => height === 28)).toBe(true);
      expect(actual.frameTopCorners).toEqual(['0px', '0px']);
      expect(actual.frameBottomCorners).toEqual(['6px', '6px']);
      expect(actual.hostTop).toBe(actual.bodyTop);
      expect(actual.hostWidth).toBe(46);
      expect(actual.rowLeft).toBe(0);
      expect(actual.rowsPadding).toBe('0px');
      expect(actual.buttonPadding).toBe('5px');
      expect(actual.iconWidth).toBe(34);
      expect(actual.cardLeft).toBe(46);
      expect(actual.masonryPadding).toBe('0px 0px 0px 46px');
      expect(actual.masonryFill).toBe('rgb(50, 50, 50)');
      expect(actual.toolbarFill).toBe(actual.masonryFill);
      expect(actual.divider).toBe('1px');
      expect(actual.selectedFill).toBe('rgb(31, 31, 31)');
      expect(actual.selectedBorder).toBe('rgb(62, 62, 62)');
      expect(actual.normalFill).toBe('rgba(0, 0, 0, 0)');
      expect(actual.toolFill).toBe('rgba(0, 0, 0, 0)');
      expect(actual.toolRadius).toBe('0px');
      expect(actual.generationCorners).toEqual(['0px', '0px']);
      expect(actual.right).toBe(actual.barRight);
      expect(actual.rightGap).toBe(2);
      expect(
        await page
          .locator('[data-sc-plugin-toolbar]')
          .evaluate((e) => getComputedStyle(e).paddingInline),
      ).toBe('4px');
      await page
        .locator('.sc-cube-stack-row')
        .nth(1)
        .evaluate((e) => e.setAttribute('data-severity', 'error'));
      const warningFill = await page
        .locator('.sc-cube-stack-row')
        .nth(1)
        .evaluate((e) => getComputedStyle(e).backgroundColor);
      expect(warningFill).not.toBe('rgba(0, 0, 0, 0)');
      expect(warningFill).not.toBe('rgb(31, 31, 31)');
      await page
        .locator('.sc-cube-stack-row')
        .nth(1)
        .evaluate((e) => e.setAttribute('data-severity', 'none'));
      await page.locator('.sc-cube-stack-row').first().hover();
      expect(
        await page
          .locator('.sc-cube-stack-row')
          .first()
          .evaluate((e) => getComputedStyle(e).backgroundColor),
      ).toBe('rgb(31, 31, 31)');
      await page.locator('.sc-plugin-settings').hover();
      expect(
        await page.locator('.sc-plugin-settings').evaluate((e) => getComputedStyle(e).borderColor),
      ).toBe('rgb(71, 71, 71)');
      await page.locator('.sc-cube-stack-row').nth(1).hover();
      expect(
        await page
          .locator('.sc-cube-stack-row')
          .nth(1)
          .evaluate((e) => getComputedStyle(e).borderColor),
      ).toBe('rgb(71, 71, 71)');
      await page.locator('html').evaluate((e) => e.removeAttribute('data-sc-visual-style'));
      expect(
        await page.locator('#toolbar-frame').evaluate((e) => getComputedStyle(e).borderRadius),
      ).toBe('6px');
    } finally {
      await page.close();
    }
  },
);

test.each(['black', 'dark-gray', 'medium-gray', 'light-gray'] as const)(
  'matches disabled generation segments to the spinner fill in PhotoPro %s',
  async (palette) => {
    const page = await browser.newPage();
    try {
      await page.setContent(
        `<!doctype html><html data-sc-visual-style="photopro" data-sc-photopro-palette="${palette}"><body data-sc-theme="photopro"><div class="sc-generation-strip" ${palette === 'light-gray' ? 'data-sc-color-scheme="light"' : ''}><div class="sc-generation-batch"></div><div class="sc-generation-actions"><button class="sc-generation-action" disabled>Stop</button><button class="sc-generation-action">Run</button></div></div></body></html>`,
      );
      await page.addStyleTag({
        content: `[data-sc-theme="photopro"]{${photoProTokenDeclarations(palette)}--sc-accent-color:#d6d6d6}${generationStripCss}${photoProAppearanceCss()}`,
      });
      const colors = await page.evaluate(() => {
        const batch = document.querySelector('.sc-generation-batch')!,
          disabled = document.querySelector('button:disabled')!,
          enabled = document.querySelector('button:not(:disabled)')!,
          actions = document.querySelector('.sc-generation-actions')!;
        const context = document.createElement('canvas').getContext('2d')!;
        context.fillStyle = getComputedStyle(disabled).backgroundColor;
        context.fillRect(0, 0, 1, 1);
        return {
          batch: getComputedStyle(batch).backgroundColor,
          disabled: getComputedStyle(disabled).backgroundColor,
          disabledPixel: [...context.getImageData(0, 0, 1, 1).data],
          enabled: getComputedStyle(enabled).backgroundColor,
          accent: getComputedStyle(actions).backgroundColor,
        };
      });
      expect(colors.disabled).toBe(colors.batch);
      expect(colors.disabledPixel[3]).toBe(255);
      if (palette === 'dark-gray') expect(colors.disabledPixel).toEqual([39, 39, 39, 255]);
      expect(colors.enabled).toBe('rgba(0, 0, 0, 0)');
      expect(colors.accent).toBe('rgb(214, 214, 214)');
      await page.locator('button:disabled').hover();
      expect(
        await page.locator('button:disabled').evaluate((e) => getComputedStyle(e).backgroundColor),
      ).toBe(colors.batch);
      await page.locator('button:disabled').evaluate((e) => e.removeAttribute('disabled'));
      await page.mouse.move(300, 100);
      expect(
        await page
          .locator('button')
          .first()
          .evaluate((e) => getComputedStyle(e).backgroundColor),
      ).toBe('rgba(0, 0, 0, 0)');
      await page.locator('html').evaluate((e) => e.removeAttribute('data-sc-visual-style'));
      await page
        .locator('button')
        .first()
        .evaluate((e) => e.setAttribute('disabled', ''));
      expect(
        await page
          .locator('button')
          .first()
          .evaluate((e) => getComputedStyle(e).backgroundColor),
      ).toBe(palette === 'light-gray' ? 'rgb(205, 205, 205)' : 'rgb(52, 52, 52)');
    } finally {
      await page.close();
    }
  },
);

test('shares square header tools and a centered, selected-only advanced disclosure', async () => {
  const page = await browser.newPage();
  try {
    await page.setContent(
      `<!doctype html><html data-sc-visual-style="photopro" data-sc-photopro-palette="dark-gray"><body data-sc-theme="photopro"><header class="sugarcubes-workflow-masonry__section"><div class="sugarcubes-workflow-masonry__actions"><button class="sugarcubes-workflow-masonry__icon-action" id="editor">Editor</button><button class="sugarcubes-workflow-masonry__icon-action" id="eye" aria-expanded="false">Eye</button></div></header><div class="sugarcubes-cube-face__node-card"><div data-sc-style-node><button data-testid="advanced-inputs-button" aria-expanded="false"><i>Native</i></button></div></div></body></html>`,
    );
    await page.addStyleTag({
      content: `*{transition:none!important}.sugarcubes-workflow-masonry__section{top:0}.sugarcubes-cube-face__node-card{margin-top:60px}[data-sc-theme="photopro"]{${photoProTokenDeclarations('dark-gray')}}${photoProAppearanceCss()}`,
    });
    await page.addScriptTag({ content: stackStyles });
    const appearance = (selector: string) =>
      page.locator(selector).evaluate((e) => {
        const s = getComputedStyle(e);
        return {
          fill: s.backgroundColor,
          border: s.borderTopColor,
          radius: s.borderRadius,
          height: e.getBoundingClientRect().height,
        };
      });
    for (const selector of ['#editor', '#eye', '[data-testid="advanced-inputs-button"]']) {
      const initial = await appearance(selector);
      expect(initial.fill).toBe('rgba(0, 0, 0, 0)');
      expect(initial.radius).toBe('0px');
      await page.locator(selector).hover();
      expect((await appearance(selector)).border).toBe('rgb(71, 71, 71)');
      expect((await appearance(selector)).fill).toBe('rgb(41, 41, 41)');
      expect((await appearance(selector)).radius).toBe('4px');
      await page.mouse.move(700, 500);
    }
    expect(
      await page.locator('.sugarcubes-workflow-masonry__actions').evaluate((e) => ({
        gap: getComputedStyle(e).gap,
        padding: getComputedStyle(e).paddingRight,
      })),
    ).toEqual({ gap: '2px', padding: '4px' });
    const advanced = '[data-testid="advanced-inputs-button"]';
    expect((await appearance(advanced)).height).toBe(18);
    const glyph = await page.locator(advanced).evaluate((e) => ({
      align: getComputedStyle(e).alignItems,
      height: getComputedStyle(e, '::after').height,
      width: getComputedStyle(e, '::after').width,
    }));
    expect(glyph).toEqual({ align: 'center', height: '2px', width: '12px' });
    for (const selector of ['#eye', advanced]) {
      await page.locator(selector).evaluate((e) => e.setAttribute('aria-expanded', 'true'));
      expect((await appearance(selector)).fill).toBe('rgb(31, 31, 31)');
      await page.locator(selector).hover();
      expect((await appearance(selector)).fill).toBe('rgb(31, 31, 31)');
      await page.mouse.move(700, 500);
      await page.locator(selector).evaluate((e) => e.setAttribute('aria-expanded', 'false'));
      expect((await appearance(selector)).fill).toBe('rgba(0, 0, 0, 0)');
    }
  } finally {
    await page.close();
  }
});

test('name-sized PhotoPro tabs preserve the complete title and reserve the close-button slot', async () => {
  const page = await browser.newPage();
  try {
    await page.setContent(
      '<html data-sc-visual-style="photopro"><body><header class="sc-substitute-frame"><div data-sc-workflow-tabs><div data-sc-workflow-tab><span class="sc-workflow-title">Anima (2)</span><button class="sc-workflow-close">×</button></div></div></header></body></html>',
    );
    await page.addStyleTag({ content: substituteFrameCss + photoProAppearanceCss() });
    const result = await page.locator('[data-sc-workflow-tab]').evaluate((tab) => {
      const title = tab.querySelector<HTMLElement>('.sc-workflow-title')!,
        close = tab.querySelector('.sc-workflow-close')!;
      const range = document.createRange();
      range.selectNodeContents(title);
      const text = range.getBoundingClientRect(),
        label = title.getBoundingClientRect(),
        button = close.getBoundingClientRect(),
        bounds = tab.getBoundingClientRect();
      return {
        mask: getComputedStyle(title).maskImage,
        text: text.width,
        label: label.width,
        gap: button.left - label.right,
        close: button.width,
        remaining: bounds.right - button.right,
      };
    });
    expect(result.mask).toBe('none');
    expect(result.label).toBeGreaterThanOrEqual(result.text - 0.1);
    expect(result.gap).toBeGreaterThanOrEqual(6);
    expect(result.close).toBe(14);
    expect(result.remaining).toBeGreaterThanOrEqual(8);
  } finally {
    await page.close();
  }
});

test('PhotoPro retains the tab content extent inside a narrow scrolling viewport', async () => {
  const page = await browser.newPage();
  try {
    await page.setContent(
      '<html data-sc-visual-style="photopro"><body><header class="sc-substitute-frame"><div class="sc-workflow-strip" style="width:100px"><div data-sc-workflow-tabs style="--sc-workflow-tabs-width:400px"><div data-sc-workflow-tab><span class="sc-workflow-title">A longer workflow title</span><button class="sc-workflow-close">×</button></div><div data-sc-workflow-tab><span class="sc-workflow-title">Another workflow title</span><button class="sc-workflow-close">×</button></div></div></div></header></body></html>',
    );
    await page.addStyleTag({ content: substituteFrameCss + photoProAppearanceCss() });
    const extent = await page.locator('.sc-workflow-strip').evaluate((viewport) => {
      viewport.scrollLeft = 100;
      return {
        width: viewport.clientWidth,
        content: viewport.scrollWidth,
        position: viewport.scrollLeft,
      };
    });
    expect(extent).toEqual({ width: 100, content: 400, position: 100 });
  } finally {
    await page.close();
  }
});
