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
/** Prove a masonry card in Comfy's own style paints Python's translucent Fluent card fill once. */
import { chromium, type Browser } from '@playwright/test';
import { build } from 'esbuild';
import UPNG from 'upng-js';
import { launchHeadlessBrowser } from '../../frontend_core/support/BrowserFixtureLifetime.js';

describe('Chromium desktop masonry card in Comfy style', () => {
  let browser: Browser;
  let install: string;

  beforeAll(async () => {
    browser = await launchHeadlessBrowser(chromium);
    const output = await build({
      stdin: {
        contents: `
        import {ensureWorkflowMasonryStyles} from './frontend/comfyui/ui/surface/masonry/WorkflowMasonryStyles.ts';
        import {ensureNodeCardAppearanceStyles} from './frontend/comfyui/ui/surface/appearance/NodeCardAppearanceStyles.ts';
        import {desktopFrameCss} from './frontend/comfyui/ui/surface/frame/DesktopFrameStyles.ts';
        import {substituteFrameCss} from './frontend/comfyui/ui/surface/frame/SubstituteFrameStyles.ts';
        import {DesktopFramePresentation} from './frontend/comfyui/ui/surface/frame/DesktopFramePresentation.ts';
        ensureWorkflowMasonryStyles(document); ensureNodeCardAppearanceStyles(document);
        const style = document.createElement('style');
        style.textContent = substituteFrameCss + desktopFrameCss;
        document.head.append(style);
        const owner = new DesktopFramePresentation();
        owner.configure({schemaVersion:1,captionWidth:138,titlebarHeight:34,darkTheme:globalThis.darkTheme});
        owner.attach(document.querySelector('#frame'));
        globalThis.cardInstalled = true;
      `,
        resolveDir: process.cwd(),
      },
      bundle: true,
      write: false,
      format: 'esm',
    });
    install = output.outputFiles[0]!.text;
  });

  afterAll(async () => {
    await browser?.close();
  });

  /**
   * The card DOM and Comfy's own palette-driven layer colors, reproduced from the real Nodes 2.0
   * masonry card in Comfy's visual style (frontend 1.47.11, dark palette): an outer card on Comfy's
   * input background, a native-node mount, and an inner wrapper and body on the palette's node
   * surfaces.
   */
  const page = (
    dark: boolean,
  ): string => `<!doctype html><html data-sc-substitute-frame data-sc-visual-style="comfy"><body>
    <header id="frame"></header>
    <main id="graph-canvas-container" data-sugarcubes-masonry="true">
      <div class="sugarcubes-workflow-masonry__backdrop"></div>
      <section class="sugarcubes-workflow-masonry">
        <article class="sugarcubes-workflow-masonry__card sugarcubes-cube-face__node-card">
          <div class="sugarcubes-cube-face__native-card-mount sugarcubes-native-node-card" data-cube-face-native="nodes-2">
            <div class="lg-node" data-cube-face-presentation data-sc-style-node>
              <div data-testid="node-inner-wrapper" class="bg-node-component-header-surface">
                <div data-testid="node-header-1" class="bg-node-component-header-surface">Title</div>
                <div data-testid="node-body-1" class="bg-component-node-background">Body</div>
              </div>
            </div>
          </div>
        </article>
      </section>
    </main></body></html>
    <style>
      :root { --comfy-input-bg:${dark ? '#222' : '#ddd'}; --interface-stroke:${dark ? '#505050' : '#d0d0d0'};
        --node-component-header-surface:${dark ? '#202121' : '#f4f4f4'}; --component-node-background:${dark ? '#262729' : '#fafafa'}; }
      .bg-node-component-header-surface { background-color:var(--node-component-header-surface); }
      .bg-component-node-background { background-color:var(--component-node-background); }
      body { margin:0 } body::before { content:''; position:fixed; inset:0; background:rgb(80,120,160); z-index:-1 }
      main { position:relative; width:700px; height:300px }
      .sugarcubes-workflow-masonry { padding:20px }
      [data-testid^="node-header-"], [data-testid^="node-body-"] { height:60px }
    </style>`;

  test.each([true, false])('desktop theme dark=%s paints the card fill once', async (dark) => {
    const tab = await browser.newPage({ viewport: { width: 900, height: 400 } });
    try {
      await tab.setContent(page(dark));
      await tab.evaluate((value) => Object.assign(globalThis, { darkTheme: value }), dark);
      await tab.addScriptTag({ content: install, type: 'module' });
      await tab.waitForFunction(() => Reflect.get(globalThis, 'cardInstalled') === true);
      const fills = await tab.evaluate(() => {
        const fill = (selector: string): string =>
          getComputedStyle(document.querySelector(selector)!).backgroundColor;
        return {
          card: fill('.sugarcubes-workflow-masonry__card'),
          mount: fill('.sugarcubes-cube-face__native-card-mount'),
          node: fill('.lg-node'),
          wrapper: fill('[data-testid="node-inner-wrapper"]'),
          header: fill('[data-testid^="node-header-"]'),
          body: fill('[data-testid^="node-body-"]'),
        };
      });
      expect(fills.card).toBe(dark ? 'rgba(255, 255, 255, 0.05)' : 'rgba(255, 255, 255, 0.7)');
      for (const layer of [fills.mount, fills.node, fills.wrapper, fills.header, fills.body])
        expect(layer).toBe('rgba(0, 0, 0, 0)');
      const body = await tab.locator('[data-testid^="node-body-"]').boundingBox();
      if (!body) throw new Error('The card body has no painted bounds.');
      const png = await tab.screenshot({
        clip: { x: Math.floor(body.x) + 40, y: Math.floor(body.y) + 20, width: 1, height: 1 },
      });
      const painted = [
        ...new Uint8Array(UPNG.toRGBA8(UPNG.decode(Uint8Array.from(png).buffer))[0]!),
      ].slice(0, 3);
      const mica = [80, 120, 160];
      const wash = dark ? { value: 32, alpha: 150 / 255 } : { value: 251, alpha: 188 / 255 };
      const card = dark ? 13 / 255 : 179 / 255;
      const expected = mica.map((channel) => {
        const washed = wash.value * wash.alpha + channel * (1 - wash.alpha);
        return Math.round(255 * card + washed * (1 - card));
      });
      for (const [channel, value] of painted.entries())
        expect(Math.abs(value - expected[channel]!)).toBeLessThanOrEqual(1);
    } finally {
      await tab.close();
    }
  });
});
