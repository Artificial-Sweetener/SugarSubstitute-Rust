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
/** Prove that only Compact masonry or Plugin Mode packs masonry and the Cube stack edge to edge. */
import { chromium, type Browser } from '@playwright/test';
import { build } from 'esbuild';
import {
  closeBrowserFixture,
  launchHeadlessBrowser,
} from '../../frontend_core/support/BrowserFixtureLifetime.js';
import { substituteFrameCss } from '../../../frontend/comfyui/ui/surface/frame/SubstituteFrameStyles.js';
import { pluginFrameCss } from '../../../frontend/comfyui/ui/surface/frame/PluginFrameStyles.js';

let browser: Browser;
let installStyles: string;
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
  installStyles = output.outputFiles[0]!.text;
});
afterAll(async () => {
  await closeBrowserFixture(browser, undefined);
});

interface Presentation {
  readonly hostLeft: number;
  readonly hostTop: number;
  readonly hostRadius: string;
  readonly stackWidth: number;
  readonly rowLeft: number;
  readonly rowRadius: string;
  readonly rowsPadding: string;
  readonly gap: string;
  readonly masonryPadding: string;
  readonly labelDisplay: string;
}

/** Render the real rail and masonry rules under the given root presentation attributes. */
async function present(root: string, compactStack: boolean): Promise<Presentation> {
  const page = await browser.newPage({ viewport: { width: 1000, height: 600 } });
  try {
    await page.setContent(`<!doctype html><html ${root}><body><div id="workspace">
      <div class="sugarcubes-workflow-masonry" style="--sc-masonry-stack-width:58px"><div class="sugarcubes-workflow-masonry__content">Cards</div></div>
      <aside class="sc-cube-stack-host"><nav class="sc-cube-stack" data-kind="rail" ${compactStack ? 'data-compact' : ''}>
        <ul class="sc-cube-stack__rows" style="height:96px">
          <li class="sc-cube-stack-row" data-selected data-severity="none"><button class="sc-cube-stack-row__activate"><span class="sc-cube-stack-row__icon">One</span><span class="sc-cube-stack-row__text"><span class="sc-cube-stack-row__label">One</span></span></button></li>
        </ul></nav></aside></div></body></html>`);
    await page.addStyleTag({
      content: `*{box-sizing:border-box}html,body{margin:0;height:100%}#workspace{position:relative;height:100%}${substituteFrameCss}${pluginFrameCss}`,
    });
    await page.addScriptTag({ content: installStyles });
    return await page.evaluate(() => {
      const host = document.querySelector('.sc-cube-stack-host')!;
      const stack = document.querySelector('.sc-cube-stack')!;
      const row = document.querySelector<HTMLElement>('.sc-cube-stack-row')!;
      const masonry = document.querySelector('.sugarcubes-workflow-masonry')!;
      return {
        hostLeft: host.getBoundingClientRect().left,
        hostTop: host.getBoundingClientRect().top,
        hostRadius: getComputedStyle(host).borderRadius,
        stackWidth: stack.getBoundingClientRect().width,
        rowLeft: row.getBoundingClientRect().left,
        rowRadius: getComputedStyle(row).borderRadius,
        rowsPadding: getComputedStyle(document.querySelector('.sc-cube-stack__rows')!).padding,
        gap: getComputedStyle(masonry).getPropertyValue('--sugarcubes-masonry-gap').trim(),
        masonryPadding: getComputedStyle(masonry).padding,
        labelDisplay: getComputedStyle(row.querySelector('.sc-cube-stack-row__text')!).display,
      };
    });
  } finally {
    await page.close();
  }
}

test.each([
  ['plain ComfyUI', '', 12, 12, '8px', '5px'],
  ['the Substitute frame', 'data-sc-substitute-frame', 0, 6, '0px', '4px'],
])(
  'a compact Cube stack in %s only hides labels and keeps masonry and rail spacing',
  async (_name, root, hostLeft, hostTop, hostRadius, rowRadius) => {
    const actual = await present(root, true);
    expect(actual.labelDisplay).toBe('none');
    expect(actual.stackWidth).toBe(58);
    expect(actual.hostLeft).toBe(hostLeft);
    expect(actual.hostTop).toBe(hostTop);
    expect(actual.hostRadius).toBe(hostRadius);
    expect(actual.rowRadius).toBe(rowRadius);
    expect(actual.rowsPadding).toBe('0px 6px');
    expect(actual.rowLeft).toBe(hostLeft + 6);
    expect(actual.gap).not.toBe('0');
    expect(actual.masonryPadding).toBe('12px 20px 96px 78px');
  },
);

test.each([
  ['Compact masonry in plain ComfyUI', 'data-sc-compact-masonry'],
  ['Compact masonry in the Substitute frame', 'data-sc-compact-masonry data-sc-substitute-frame'],
  ['Plugin Mode', 'data-sc-plugin-mode data-sc-substitute-frame'],
])('%s packs masonry and the Cube stack edge to edge', async (_name, root) => {
  for (const compactStack of [false, true]) {
    const actual = await present(root, compactStack);
    expect(actual.labelDisplay).toBe(compactStack ? 'none' : 'flex');
    expect(actual.stackWidth).toBe(compactStack ? 46 : 224);
    expect(actual.hostLeft).toBe(0);
    expect(actual.hostTop).toBe(0);
    expect(actual.hostRadius).toBe('0px');
    expect(actual.rowRadius).toBe('0px');
    expect(actual.rowsPadding).toBe('0px');
    expect(actual.rowLeft).toBe(0);
    expect(actual.gap).toBe('0');
    expect(actual.masonryPadding).toBe('0px 0px 0px 58px');
  }
});
