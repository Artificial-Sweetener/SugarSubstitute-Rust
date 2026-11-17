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
/** Prove a node pack's strip in Comfy's top area never displaces Substitute's title bar. */
import { chromium, type Browser } from '@playwright/test';
import {
  closeBrowserFixture,
  launchHeadlessBrowser,
} from '../../frontend_core/support/BrowserFixtureLifetime.js';
import { substituteFrameCss } from '../../../frontend/comfyui/ui/surface/frame/SubstituteFrameStyles.js';

let browser: Browser;
beforeAll(async () => {
  browser = await launchHeadlessBrowser(chromium);
});
afterAll(async () => {
  await closeBrowserFixture(browser, undefined);
});

/** Comfy's own page grid, as its frontend declares it: top area, workspace row, bottom area. */
const comfyGrid = `
  html,body{margin:0;height:100%}
  .comfyui-body{display:grid;width:100%;height:100%;overflow:hidden;grid-template-rows:auto 1fr auto;grid-template-columns:auto 1fr auto}
  .comfyui-body-top{z-index:1001;flex-direction:column;order:-5;grid-area:1 / 1 / auto / -1;display:flex}
  .comfyui-body-left{z-index:10;order:-4;grid-area:2 / 1;display:flex}
  .comfyui-body-right{z-index:10;order:-2;grid-area:2 / 3}
  .comfyui-body-bottom{z-index:1000;flex-direction:column;order:4;grid-area:3 / 1 / auto / -1;display:flex}
  .graph-canvas-container{position:relative;order:-3;grid-area:2 / 2}`;

interface Placement {
  readonly workspaceTop: number;
  readonly workspaceHeight: number;
  readonly stripTop: number;
  readonly stripHeight: number;
  readonly noticeTop: number;
}

/** Lay out Comfy's grid with one pack strip in each of its areas, with or without the frame. */
async function place(root: string): Promise<Placement> {
  const page = await browser.newPage({ viewport: { width: 1000, height: 600 } });
  try {
    await page.setContent(`<!doctype html><html ${root}><body><div class="comfyui-body">
      <div class="comfyui-body-top"><div id="strip" style="height:16px">Idle</div></div>
      <div class="comfyui-body-bottom"><div id="notice" style="height:10px">Notice</div></div>
      <div class="comfyui-body-left"></div><div class="comfyui-body-right"></div>
      <div class="graph-canvas-container" id="workspace"></div></div></body></html>`);
    await page.addStyleTag({ content: comfyGrid + substituteFrameCss });
    return await page.evaluate(() => {
      const box = (id: string): DOMRect => document.getElementById(id)!.getBoundingClientRect();
      return {
        workspaceTop: box('workspace').top,
        workspaceHeight: box('workspace').height,
        stripTop: box('strip').top,
        stripHeight: box('strip').height,
        noticeTop: box('notice').top,
      };
    });
  } finally {
    await page.close();
  }
}

test('Comfy alone puts a pack strip above the workspace', async () => {
  expect(await place('')).toEqual({
    workspaceTop: 16,
    workspaceHeight: 574,
    stripTop: 0,
    stripHeight: 16,
    noticeTop: 590,
  });
});

test("Substitute's frame keeps the workspace first and docks the strip visibly under it", async () => {
  expect(await place('data-sc-substitute-frame')).toEqual({
    workspaceTop: 0,
    workspaceHeight: 574,
    stripTop: 574,
    stripHeight: 16,
    noticeTop: 590,
  });
});
