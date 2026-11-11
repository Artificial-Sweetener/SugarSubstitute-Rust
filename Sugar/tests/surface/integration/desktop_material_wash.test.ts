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
/** Prove native material reaches each workspace surface through one translucent paint layer. */
import { chromium, firefox, type Browser } from '@playwright/test';
import { build } from 'esbuild';
import UPNG from 'upng-js';
import {
  BROWSER_FIXTURE_SETUP_TIMEOUT_MS,
  launchHeadlessBrowser,
} from '../../frontend_core/support/BrowserFixtureLifetime.js';

describe.each([
  ['Chromium', chromium],
  ['Firefox', firefox],
] as const)('%s desktop material', (_name, engine) => {
  let browser: Browser;
  let install: string;

  beforeAll(async () => {
    browser = await launchHeadlessBrowser(engine);
    const output = await build({
      stdin: {
        contents: `
        import {ensureCubeStackStyles} from './frontend/comfyui/ui/surface/stack/CubeStackStyles.ts';
        import {ensureWorkflowMasonryStyles} from './frontend/comfyui/ui/surface/masonry/WorkflowMasonryStyles.ts';
        import {ensureNodeCardAppearanceStyles} from './frontend/comfyui/ui/surface/appearance/NodeCardAppearanceStyles.ts';
        import {fluentAppearanceCss} from './frontend/comfyui/ui/surface/appearance/FluentAppearanceStyles.ts';
        import {desktopFrameCss} from './frontend/comfyui/ui/surface/frame/DesktopFrameStyles.ts';
        import {substituteFrameCss} from './frontend/comfyui/ui/surface/frame/SubstituteFrameStyles.ts';
        import {substituteToolbarCss} from './frontend/comfyui/ui/surface/frame/SubstituteToolbarStyles.ts';
        import {DesktopFramePresentation} from './frontend/comfyui/ui/surface/frame/DesktopFramePresentation.ts';
        import {CubeStackMaterialMotion} from './frontend/comfyui/ui/surface/stack/CubeStackMaterialMotion.ts';
        import {translucentGridPixels} from './frontend/comfyui/ui/surface/frame/DesktopGraphBackdrop.ts';
        ensureCubeStackStyles(document); ensureWorkflowMasonryStyles(document); ensureNodeCardAppearanceStyles(document);
        const style = document.createElement('style');
        style.textContent = fluentAppearanceCss(DARK) + substituteFrameCss + substituteToolbarCss + desktopFrameCss;
          document.head.append(style);
          document.head.append(document.querySelector('#sugarcubes-cube-stack-styles'));
        const owner = new DesktopFramePresentation();
        owner.configure({schemaVersion:1,captionWidth:138,titlebarHeight:34,darkTheme:DARK});
        owner.attach(document.querySelector('#frame'));
        const material = new CubeStackMaterialMotion(document.querySelector('main'), error => globalThis.materialError = error.message);
        material.set(false,224,false);
        document.addEventListener('stack-density', (event) => material.set(event.detail, event.detail ? 58 : 224));
        document.addEventListener('stack-dispose', () => material.dispose());
        document.addEventListener('paint-grid', () => {
          const canvas = document.querySelector('canvas'); canvas.width=900; canvas.height=400;
          const field = DARK ? [34,34,34,255] : [243,243,243,255];
          const line = DARK ? [53,53,53,255] : [220,220,220,255];
          const context = canvas.getContext('2d');
          const tile = context.createImageData(4,1);
          tile.data.set(translucentGridPixels(new Uint8ClampedArray([...field,...field,...field,...line]), DARK));
          context.putImageData(tile,400,350);
        });
        globalThis.washInstalled = true;
      `,
        resolveDir: process.cwd(),
      },
      bundle: true,
      write: false,
      format: 'esm',
      define: { DARK: 'globalThis.darkTheme' },
    });
    install = output.outputFiles[0]!.text;
  }, BROWSER_FIXTURE_SETUP_TIMEOUT_MS);

  afterAll(async () => {
    await browser?.close();
  });

  test.each([true, false])('desktop theme dark=%s paints each fill once', async (dark) => {
    const page = await browser.newPage();
    try {
      await page.setContent(`<!doctype html><html data-sc-substitute-frame data-sc-visual-style="fluent"><body>
      <header id="frame"></header>
      <div data-sc-substitute-toolbar-menu><div data-sc-substitute-toolbar-line></div></div>
      <main id="graph-canvas-container" data-sc-style-theme-root data-sugarcubes-masonry="true">
        <canvas id="graph-canvas"></canvas><div class="sugarcubes-workflow-masonry__backdrop"></div>
        <div class="sugarcubes-workflow-masonry" data-sc-style-surface>
          <div class="sugarcubes-cube-face__node-card"><div class="lg-node" data-sc-style-node>
            <div data-testid="node-inner-wrapper"><div data-testid="node-header-1"></div><div data-testid="node-body-1"></div></div>
          </div></div>
        </div>
        <aside class="sc-cube-stack-host" data-appearance="fluent"><nav class="sc-cube-stack" data-kind="rail" data-appearance="fluent"><li class="sc-cube-stack-row" data-selected></li></nav></aside>
      </main></body></html>`);
      await page.addStyleTag({
        content: `
      body{margin:0}body::before{content:'';position:fixed;inset:0;background:rgb(80,120,160);z-index:-1}
      main{position:relative;width:900px;height:400px}canvas{position:absolute;inset:0}
      [data-testid="node-inner-wrapper"]{background:var(--component-node-background)}
      [data-testid^="node-body-"]{background:var(--node-component-surface)}`,
      });
      await page.evaluate((value) => {
        Object.assign(globalThis, { darkTheme: value });
      }, dark);
      await page.addScriptTag({ content: install, type: 'module' });
      await page.waitForFunction(() => Reflect.get(globalThis, 'washInstalled') === true);
      const fills = await page.evaluate(() => {
        const fill = (selector: string): string =>
          getComputedStyle(document.querySelector(selector)!).backgroundColor;
        return {
          container: fill('main'),
          graph: fill('canvas'),
          backdrop: fill('.sugarcubes-workflow-masonry__backdrop'),
          card: fill('[data-testid="node-inner-wrapper"]'),
          body: fill('[data-testid="node-body-1"]'),
          outerCard: fill('.sugarcubes-cube-face__node-card'),
          stack: fill('aside'),
          selected: fill('.sc-cube-stack-row'),
          menu: fill('[data-sc-substitute-toolbar-menu]'),
          line: fill('[data-sc-substitute-toolbar-line]'),
        };
      });
      expect(fills.container).toBe('rgba(0, 0, 0, 0)');
      expect(fills.graph).toBe('rgba(0, 0, 0, 0)');
      expect(fills.card).toBe(dark ? 'rgba(255, 255, 255, 0.05)' : 'rgba(255, 255, 255, 0.7)');
      expect(fills.body).toBe('rgba(0, 0, 0, 0)');
      expect(fills.outerCard).toBe('rgba(0, 0, 0, 0)');
      expect(fills.stack).toBe('rgba(0, 0, 0, 0)');
      expect(fills.selected).toBe(fills.card);
      expect(fills.menu).toBe(dark ? 'rgba(44, 44, 44, 0.59)' : 'rgba(252, 252, 252, 0.77)');
      expect(fills.line).toBe('rgba(0, 0, 0, 0)');
      const bounds = await page.locator('main').boundingBox();
      if (!bounds) throw new Error('Workspace has no painted bounds.');
      const painted = async (x: number): Promise<number[]> => {
        const png = await page.screenshot({ clip: { x, y: bounds.y + 350, width: 1, height: 1 } });
        return [
          ...new Uint8Array(UPNG.toRGBA8(UPNG.decode(Uint8Array.from(png).buffer))[0]!),
        ].slice(0, 3);
      };
      const base = [80, 120, 160];
      const alpha = (dark ? 150 : 188) / 255;
      const washed = base.map((value) =>
        Math.round(value * (1 - alpha) + (dark ? 32 : 251) * alpha),
      );
      const editorFill = await painted(400);
      for (const [channel, value] of editorFill.entries())
        expect(Math.abs(value - washed[channel]!)).toBeLessThanOrEqual(1);
      for (const compact of [true, false, true, false]) {
        const samples = await page.evaluate((density) => {
          const main = document.querySelector('main')!;
          const node = document.querySelector('[data-sc-style-node]');
          document.dispatchEvent(new CustomEvent('stack-density', { detail: density }));
          const animation = main.getAnimations()[0]!;
          animation.pause();
          const values = [0, 55, 110, 165, 220].map((time) => {
            animation.currentTime = time;
            return Number(getComputedStyle(main).getPropertyValue('--sc-stack-wash-opacity'));
          });
          return {
            values,
            duration: animation.effect!.getTiming().duration,
            retained: node === document.querySelector('[data-sc-style-node]'),
          };
        }, compact);
        expect(samples.values).toEqual(
          compact ? [1, 0.9375, 0.5, 0.0625, 0] : [0, 0.0625, 0.5, 0.9375, 1],
        );
        expect(samples.duration).toBe(220);
        expect(samples.retained).toBe(true);
      }
      // Substitute faded the wash only over the stack's live geometry: the body the rail grows
      // into keeps its full wash, and the rail's edge and the aperture's edge travel together.
      const expandedAt = async (time: number): Promise<string> =>
        page.evaluate((at) => {
          const main = document.querySelector('main')!;
          document.dispatchEvent(new CustomEvent('stack-density', { detail: true }));
          main.getAnimations()[0]?.finish();
          document.dispatchEvent(new CustomEvent('stack-density', { detail: false }));
          const animation = main.getAnimations()[0]!;
          animation.pause();
          animation.currentTime = at;
          return getComputedStyle(document.querySelector('.sc-cube-stack')!).width;
        }, time);
      const halfWashed = base.map((value) =>
        Math.round(value * (1 - alpha / 2) + (dark ? 32 : 251) * (alpha / 2)),
      );
      const expectPainted = async (x: number, expected: number[]): Promise<void> => {
        for (const [channel, value] of (await painted(x)).entries())
          expect(Math.abs(value - expected[channel]!)).toBeLessThanOrEqual(1);
      };
      const startWidth = await expandedAt(0);
      expect(await painted(20)).toEqual(base);
      await expectPainted(150, washed);
      expect(startWidth).toBe('58px');
      const midpointWidth = await expandedAt(110);
      await expectPainted(100, halfWashed);
      await expectPainted(180, washed);
      expect(midpointWidth).toBe('141px');
      expect(await expandedAt(220)).toBe('224px');
      await expectPainted(150, washed);
      await expectPainted(20, washed);
      await page.evaluate(() => document.querySelector('main')!.getAnimations()[0]?.finish());
      expect(
        await page.evaluate(() => {
          const main = document.querySelector('main')!;
          document.dispatchEvent(new CustomEvent('stack-density', { detail: true }));
          const first = main.getAnimations()[0]!;
          first.pause();
          first.currentTime = 110;
          document.dispatchEvent(new CustomEvent('stack-density', { detail: false }));
          const second = main.getAnimations()[0]!;
          second.pause();
          const effect = second.effect;
          if (!(effect instanceof KeyframeEffect))
            throw new Error('Material keyframes are missing.');
          const start = effect.getKeyframes()[0]?.['--sc-stack-wash-opacity'];
          document.dispatchEvent(new CustomEvent('stack-density', { detail: false }));
          return {
            start,
            retained: main.getAnimations()[0] === second,
            opacity: getComputedStyle(main).opacity,
          };
        }),
      ).toEqual({ start: '0.5', retained: true, opacity: '1' });
      await page.emulateMedia({ reducedMotion: 'reduce' });
      expect(
        await page.evaluate(() => {
          document.dispatchEvent(new CustomEvent('stack-density', { detail: true }));
          return {
            opacity: getComputedStyle(document.querySelector('main')!).getPropertyValue(
              '--sc-stack-wash-opacity',
            ),
            count: document.querySelector('main')!.getAnimations().length,
          };
        }),
      ).toEqual({ opacity: '0', count: 0 });
      expect(await painted(20)).toEqual(base);
      for (const [channel, value] of (await painted(400)).entries())
        expect(Math.abs(value - washed[channel]!)).toBeLessThanOrEqual(1);
      await page.emulateMedia({ reducedMotion: 'no-preference' });
      await page.evaluate(() => {
        document.dispatchEvent(new CustomEvent('stack-density', { detail: false }));
        document.querySelector('main')!.getAnimations()[0]!.cancel();
      });
      await page.waitForFunction(
        () => Reflect.get(globalThis, 'materialError') === 'Cube stack material animation failed.',
      );
      expect(
        await page.evaluate(() => {
          document.dispatchEvent(new Event('stack-dispose'));
          const main = document.querySelector<HTMLElement>('main')!;
          return [
            main.style.getPropertyValue('--sc-stack-wash-opacity'),
            main.style.getPropertyValue('--sc-stack-material-width'),
            main.getAnimations().length,
          ];
        }),
      ).toEqual(['', '', 0]);
      await page
        .locator('main')
        .evaluate((main) => main.removeAttribute('data-sugarcubes-masonry'));
      expect(
        await page.locator('canvas').evaluate((canvas) => getComputedStyle(canvas).backgroundColor),
      ).toBe(dark ? 'rgba(32, 32, 32, 0.59)' : 'rgba(251, 251, 251, 0.737)');
      await page.evaluate(() => {
        for (const selector of [
          '.sugarcubes-workflow-masonry__backdrop',
          '.sugarcubes-workflow-masonry',
          'aside',
        ])
          document.querySelector<HTMLElement>(selector)!.hidden = true;
        document.dispatchEvent(new Event('paint-grid'));
      });
      const graphFill = await painted(400);
      const gridLine = await painted(403);
      for (const [channel, value] of graphFill.entries()) {
        expect(Math.abs(value - editorFill[channel]!)).toBeLessThanOrEqual(1);
        const contrast = dark ? gridLine[channel]! - value : value - gridLine[channel]!;
        expect(contrast).toBeGreaterThanOrEqual(12);
      }
    } finally {
      await page.close();
    }
  });
});
