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
/** Prove authoritative chrome reconstruction against Chromium's fractional nested layout. */
import { chromium, type Browser, type Page } from '@playwright/test';
import { build } from 'esbuild';
import { launchHeadlessBrowser } from '../../frontend_core/support/BrowserFixtureLifetime.js';

let browserReady: Promise<Browser>;
let pageReady: Promise<Page> | undefined;
let allocationScript: string;

beforeAll(async () => {
  browserReady = launchHeadlessBrowser(chromium);
  await browserReady;
  const output = await build({
    stdin: {
      contents:
        "import {ComfyCanvasFrameAllocation} from './frontend/comfyui/ui/output_canvas/infrastructure/ComfyCanvasFrameAllocation.ts';import {BrowserCanvasWorkspace} from './frontend/comfyui/ui/output_canvas/infrastructure/BrowserCanvasWorkspace.ts';import {CanvasWorkspaceLayout} from './frontend/comfyui/ui/output_canvas/application/CanvasWorkspaceLayout.ts';globalThis.ComfyCanvasFrameAllocation=ComfyCanvasFrameAllocation;globalThis.BrowserCanvasWorkspace=BrowserCanvasWorkspace;globalThis.CanvasWorkspaceLayout=CanvasWorkspaceLayout;",
      resolveDir: process.cwd(),
    },
    bundle: true,
    write: false,
    format: 'iife',
  });
  allocationScript = output.outputFiles[0]!.text;
});

/** Keep browser page acquisition under its own bounded hook, separate from layout observation. */
beforeEach(async () => {
  const browser = await browserReady;
  pageReady = browser.newPage({ viewport: { width: 1600, height: 1000 } });
  await pageReady;
});

/** Observe even a late page creation before releasing its native browser resources. */
afterEach(async () => {
  const pending = pageReady;
  pageReady = undefined;
  await (await pending)?.close();
});

test('observes the outer frame and realigns chrome before publishing the resized workspace', async () => {
  const page = await pageReady;
  if (!page) throw new Error('Browser page fixture is unavailable.');
  await page.setContent(`<!doctype html><style>
      * { box-sizing:border-box; }
      html,body { margin:0; }
      .comfyui-body { position:relative; width:1280px; height:800px; }
      #graph-canvas-container { position:relative; width:100%; height:100%; }
      #chrome { position:absolute; inset:0; }
      .graph-canvas-panel { position:absolute; left:63px; right:4px; top:94px; bottom:4px; }
    </style><div class="comfyui-body"><div id="graph-canvas-container"><div id="chrome"><div class="graph-canvas-panel"></div></div><canvas id="graph-canvas"></canvas></div></div>`);
  await page.addScriptTag({ content: allocationScript });
  const actual = await page.evaluate(async () => {
    const owner = globalThis as typeof globalThis & {
      BrowserCanvasWorkspace: new (
        root: HTMLElement,
        panel: HTMLElement,
        model: unknown,
        frames: { request(callback: () => void): () => void },
        actions: { width(): void; close(): void; changed(): void; report(): void },
      ) => { dispose(): void };
      CanvasWorkspaceLayout: new () => unknown;
    };
    const frame = document.querySelector<HTMLElement>('.comfyui-body')!;
    const root = document.querySelector<HTMLElement>('#graph-canvas-container')!;
    const chrome = document.querySelector<HTMLElement>('#chrome')!;
    const panel = document.querySelector<HTMLElement>('.graph-canvas-panel')!;
    const workspace = new owner.BrowserCanvasWorkspace(
      root,
      panel,
      new owner.CanvasWorkspaceLayout(),
      {
        request(callback) {
          const handle = requestAnimationFrame(callback);
          return () => cancelAnimationFrame(handle);
        },
      },
      { width() {}, close() {}, changed() {}, report() {} },
    );
    const initial = chrome.getBoundingClientRect().width;
    frame.style.width = '1484px';
    frame.style.height = '937px';
    for (let index = 0; index < 12; index++) await new Promise(requestAnimationFrame);
    const resized = {
      frame: frame.getBoundingClientRect(),
      chrome: chrome.getBoundingClientRect(),
      panel: panel.getBoundingClientRect(),
      pane: root.querySelector('.sc-output-workspace-pane')!.getBoundingClientRect(),
    };
    workspace.dispose();
    return {
      initial,
      frameWidth: resized.frame.width,
      chromeWidth: resized.chrome.width,
      panelRight: resized.panel.right,
      paneRight: resized.pane.right,
    };
  });
  expect(actual).toEqual({
    initial: 1280,
    frameWidth: 1484,
    chromeWidth: 1484,
    panelRight: 1480,
    paneRight: 1480,
  });
});

afterAll(async () => {
  await (await browserReady)?.close();
});

test('matches fractional outer-frame dimensions instead of losing an inverse-calc quantum', async () => {
  const page = await pageReady;
  if (!page) throw new Error('Browser page fixture is unavailable.');
  await page.setContent(`<!doctype html><style>
      * { box-sizing:border-box; }
      html,body { margin:0; }
      .comfyui-body { position:relative; width:1484px; height:937px; }
      #graph-canvas-container { position:relative; width:100%; height:100%; }
      #chrome { position:absolute; inset:0; }
      .graph-canvas-panel { position:absolute; left:63.25px; right:4.125px; top:78.5px; bottom:2.25px; }
    </style><div class="comfyui-body"><div id="graph-canvas-container"><div id="chrome"><div class="graph-canvas-panel"></div></div><canvas id="graph-canvas"></canvas></div></div>`);
  await page.addScriptTag({ content: allocationScript });
  const actual = await page.evaluate(() => {
    const owner = globalThis as typeof globalThis & {
      ComfyCanvasFrameAllocation: new (
        root: HTMLElement,
        panel: HTMLElement,
      ) => {
        apply(inset: number): void;
      };
    };
    const frame = document.querySelector<HTMLElement>('.comfyui-body')!;
    const root = document.querySelector<HTMLElement>('#graph-canvas-container')!;
    const chrome = document.querySelector<HTMLElement>('#chrome')!;
    const panel = document.querySelector<HTMLElement>('.graph-canvas-panel')!;
    const allocation = new owner.ComfyCanvasFrameAllocation(root, panel);
    allocation.apply(566.79375);
    const first = {
      frame: frame.getBoundingClientRect(),
      chrome: chrome.getBoundingClientRect(),
      root: root.getBoundingClientRect(),
      panel: panel.getBoundingClientRect(),
      chromeStyle: chrome.getAttribute('style'),
    };
    frame.style.width = '1484.375px';
    allocation.apply(566.79375);
    return {
      first: {
        frameWidth: first.frame.width,
        chromeWidth: first.chrome.width,
        rootX: first.root.x,
        panelX: first.panel.x,
        chromeStyle: first.chromeStyle,
      },
      resized: {
        frameWidth: frame.getBoundingClientRect().width,
        chromeWidth: chrome.getBoundingClientRect().width,
        chromeWidthStyle: chrome.style.width,
      },
    };
  });
  expect(actual).toEqual({
    first: {
      frameWidth: 1484,
      chromeWidth: 1484,
      rootX: 63.25,
      panelX: 63.25,
      chromeStyle: 'left: -63.25px; width: 1484px; top: -78.5px; height: 937px;',
    },
    resized: {
      frameWidth: 1484.375,
      chromeWidth: 1484.375,
      chromeWidthStyle: '1484.38px',
    },
  });
});
