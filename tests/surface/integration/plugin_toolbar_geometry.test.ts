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
/** Prove Plugin Mode keeps its toolbar contiguous and generation segments intact under PhotoPro. */
import { chromium, type Browser } from '@playwright/test';
import { pluginFrameCss } from '../../../frontend/comfyui/ui/surface/frame/PluginFrameStyles.js';
import { generationStripCss } from '../../../frontend/comfyui/ui/surface/frame/GenerationStripStyles.js';
import { photoProControlCss } from '../../../frontend/comfyui/ui/surface/appearance/PhotoProAppearanceStyles.js';
import { photoProTokenDeclarations } from '../../../frontend/comfyui/ui/surface/appearance/PhotoProThemeTokens.js';

let browser: Browser;

beforeAll(async () => {
  browser = await chromium.launch({ headless: true });
});

afterAll(async () => {
  await browser.close();
});

test.each(['dark-gray', 'medium-gray', 'light-gray'] as const)(
  'keeps one row and one segmented generation group in PhotoPro %s',
  async (palette) => {
    const page = await browser.newPage({ viewport: { width: 1200, height: 300 } });
    try {
      await page.setContent(`<!doctype html>
        <html data-sc-visual-style="photopro" data-sc-plugin-mode data-sc-substitute-frame>
        <body><div class="actionbar-container" data-sc-plugin-toolbar data-sc-theme="photopro">
          <div class="sc-plugin-toolbar-start"><button class="sc-plugin-settings">Settings</button></div>
          <button id="native-toggle">View</button>
          <div id="properties-toggle"><button aria-label="Toggle properties panel"><i class="icon-[lucide--panel-right]"></i></button></div>
          <div class="sc-generation-strip">
            <div class="sc-generation-batch"><input value="1"><button class="sc-generation-step" data-direction="up"></button></div>
            <div class="sc-generation-actions"><button class="sc-generation-action">Cancel</button><button class="sc-generation-action">Generate</button><button class="sc-generation-action">Skip</button><span class="sc-generation-queue-slot"></span></div>
          </div>
          <div class="sc-overrides-dock-slot" style="height:100px">Global Overrides</div>
        </div><button data-sc-generation-queue style="--sc-queue-left:0px;--sc-queue-top:0px">Queue</button></body></html>`);
      await page.addStyleTag({
        content: `
        .actionbar-container { display:flex; }
        [data-sc-theme="photopro"] { ${photoProTokenDeclarations(palette)} --sc-accent-color:#459cf0; }
        :where(#native-toggle) { height:32px; width:32px; }
        :where(#properties-toggle, #properties-toggle button) { height:32px; width:32px; }
        ${generationStripCss}${pluginFrameCss}${photoProControlCss()}
      `,
      });
      const geometry = await page.locator('.actionbar-container').evaluate((bar) => {
        const toggle = bar.querySelector('#native-toggle')!;
        const strip = bar.querySelector('.sc-generation-strip')!;
        const buttons = [...strip.querySelectorAll('button')];
        const segments = [...strip.querySelectorAll('.sc-generation-action')];
        return {
          overrides: {
            width: bar.querySelector('.sc-overrides-dock-slot')!.getBoundingClientRect().width,
            top: bar.querySelector('.sc-overrides-dock-slot')!.getBoundingClientRect().top,
          },
          barWidth: bar.getBoundingClientRect().width,
          controlBottom: strip.getBoundingClientRect().bottom,
          gap: strip.getBoundingClientRect().left - toggle.getBoundingClientRect().right,
          propertiesGap:
            bar.querySelector('#properties-toggle')!.getBoundingClientRect().left -
            strip.getBoundingClientRect().right,
          propertiesRight: bar.querySelector('#properties-toggle')!.getBoundingClientRect().right,
          controlRights: [...bar.children]
            .filter((child) => !child.matches('.sc-overrides-dock-slot'))
            .map((child) => child.getBoundingClientRect().right),
          centers: [
            bar.querySelector('.sc-plugin-settings')!,
            toggle,
            strip,
            strip.querySelector('.sc-generation-batch')!,
            strip.querySelector('.sc-generation-actions')!,
            bar.querySelector('#properties-toggle')!,
          ].map((node) => {
            const bounds = node.getBoundingClientRect();
            return bounds.top + bounds.height / 2;
          }),
          borders: buttons.map((button) => getComputedStyle(button).borderTopWidth),
          fills: segments.map((button) => getComputedStyle(button).backgroundColor),
          corners: segments.map((button) => getComputedStyle(button).borderTopLeftRadius),
          inputBorder: getComputedStyle(strip.querySelector('input')!).borderTopWidth,
          topCorners: [
            getComputedStyle(strip.querySelector('.sc-generation-batch')!).borderTopLeftRadius,
            getComputedStyle(strip.querySelector('.sc-generation-actions')!).borderTopLeftRadius,
            getComputedStyle(strip.querySelector('.sc-generation-actions')!).borderTopRightRadius,
            getComputedStyle(document.querySelector('[data-sc-generation-queue]')!)
              .borderTopRightRadius,
          ],
        };
      });
      expect(geometry.overrides.width).toBe(geometry.barWidth);
      expect(geometry.overrides.top).toBe(geometry.controlBottom + 4);
      expect(geometry.gap).toBeGreaterThanOrEqual(4);
      expect(geometry.propertiesGap).toBe(4);
      expect(geometry.propertiesRight).toBe(Math.max(...geometry.controlRights));
      expect(new Set(geometry.centers).size).toBe(1);
      expect(geometry.borders.every((border) => border === '0px')).toBe(true);
      expect(geometry.fills.every((fill) => fill === 'rgba(0, 0, 0, 0)')).toBe(true);
      expect(geometry.corners.every((corner) => corner === '0px')).toBe(true);
      expect(geometry.inputBorder).toBe('0px');
      expect(geometry.topCorners).toEqual(['0px', '0px', '0px', '0px']);
      await page.evaluate(() => {
        document.documentElement.removeAttribute('data-sc-plugin-mode');
        document
          .querySelector('[data-sc-plugin-toolbar]')!
          .removeAttribute('data-sc-plugin-toolbar');
      });
      const restored = await page.locator('.sc-generation-strip').evaluate((strip) => ({
        alignment: getComputedStyle(strip).alignItems,
        batch: getComputedStyle(strip.querySelector('.sc-generation-batch')!).borderTopLeftRadius,
        actions: getComputedStyle(strip.querySelector('.sc-generation-actions')!)
          .borderTopRightRadius,
        queue: getComputedStyle(document.querySelector('[data-sc-generation-queue]')!)
          .borderTopRightRadius,
        propertiesOrder: getComputedStyle(document.querySelector('#properties-toggle')!).order,
        propertiesBeforeGeneration:
          document.querySelector('#properties-toggle')!.getBoundingClientRect().right <=
          strip.getBoundingClientRect().left,
      }));
      expect(restored).toEqual({
        alignment: 'flex-start',
        batch: '0px',
        actions: '0px',
        queue: '0px',
        propertiesOrder: '0',
        propertiesBeforeGeneration: true,
      });
    } finally {
      await page.close();
    }
  },
);

test('fills the plugin panel to its bottom edge and restores the native splitter margin outside plugin mode', async () => {
  const page = await browser.newPage({ viewport: { width: 520, height: 1072 } });
  try {
    await page.setContent(`<!doctype html><html data-sc-plugin-mode><body>
      <div class="splitter-overlay-bottom"><div class="graph-canvas-panel"></div></div>
      </body></html>`);
    await page.addStyleTag({
      content: `
      html,body { height:100%; margin:0; }
      body { display:flex; flex-direction:column; }
      .splitter-overlay-bottom { flex:1; margin-bottom:4px; }
      .graph-canvas-panel { height:100%; }
      ${pluginFrameCss}
    `,
    });
    expect(
      await page
        .locator('.graph-canvas-panel')
        .evaluate((panel) => panel.getBoundingClientRect().bottom),
    ).toBe(1072);
    await page.evaluate(() => document.documentElement.removeAttribute('data-sc-plugin-mode'));
    expect(
      await page
        .locator('.graph-canvas-panel')
        .evaluate((panel) => panel.getBoundingClientRect().bottom),
    ).toBe(1068);
  } finally {
    await page.close();
  }
});
