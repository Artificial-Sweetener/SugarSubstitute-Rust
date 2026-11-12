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
/** Prove authoritative chrome reconstruction and graph-layer clipping against Chromium layout. */
import { chromium, type Browser, type Page } from '@playwright/test';
import { build } from 'esbuild';
import UPNG from 'upng-js';
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
        "import {ComfyCanvasFrameAllocation} from './frontend/comfyui/ui/output_canvas/infrastructure/ComfyCanvasFrameAllocation.ts';import {BrowserCanvasWorkspace} from './frontend/comfyui/ui/output_canvas/infrastructure/BrowserCanvasWorkspace.ts';import {CanvasWorkspaceLayout} from './frontend/comfyui/ui/output_canvas/application/CanvasWorkspaceLayout.ts';import {ensureCanvasViewportStyles} from './frontend/comfyui/ui/output_canvas/infrastructure/CanvasViewportStyles.ts';globalThis.ComfyCanvasFrameAllocation=ComfyCanvasFrameAllocation;globalThis.BrowserCanvasWorkspace=BrowserCanvasWorkspace;globalThis.CanvasWorkspaceLayout=CanvasWorkspaceLayout;globalThis.ensureCanvasViewportStyles=ensureCanvasViewportStyles;",
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
        actions: { editorWidth(): void; close(): void; changed(): void; report(): void },
      ) => { dispose(): void };
      CanvasWorkspaceLayout: new (defaultEditorWidth: number) => unknown;
    };
    const frame = document.querySelector<HTMLElement>('.comfyui-body')!;
    const root = document.querySelector<HTMLElement>('#graph-canvas-container')!;
    const chrome = document.querySelector<HTMLElement>('#chrome')!;
    const panel = document.querySelector<HTMLElement>('.graph-canvas-panel')!;
    const workspace = new owner.BrowserCanvasWorkspace(
      root,
      panel,
      new owner.CanvasWorkspaceLayout(840),
      {
        request(callback) {
          const handle = requestAnimationFrame(callback);
          return () => cancelAnimationFrame(handle);
        },
      },
      { editorWidth() {}, close() {}, changed() {}, report() {} },
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
      chromeStyle: 'position: fixed; left: 0px; top: 0px; width: 1484px; height: 937px;',
    },
    resized: {
      frameWidth: 1484.375,
      chromeWidth: 1484.375,
      chromeWidthStyle: '1484.38px',
    },
  });
});

/** Read one opaque screenshot pixel in CSS pixels at the fixture's unit device scale. */
function pixel(image: UPNG.Image, x: number, y: number): readonly number[] {
  const rgba = new Uint8Array(UPNG.toRGBA8(image)[0]!);
  const offset = (y * image.width + x) * 4;
  return [...rgba.slice(offset, offset + 3)];
}

test('keeps Comfy graph layers out of a transparent docked strip while chrome spans the frame', async () => {
  const page = await pageReady;
  if (!page) throw new Error('Browser page fixture is unavailable.');
  // Comfy 1.47.11 graph root: full-frame chrome, the LiteGraph canvas, the transformed Vue node
  // pane and the Nodes 1.0 DOM-widget layer, whose widgets are fixed in client coordinates.
  await page.setContent(`<!doctype html><style>
      * { box-sizing:border-box; }
      html,body { margin:0; background:rgb(0,0,0); }
      .comfyui-body { display:grid; grid-template-rows:auto 1fr auto; grid-template-columns:auto 1fr auto;
        width:1200px; height:800px; overflow:hidden; }
      #graph-canvas-container { grid-area:2/2; position:relative; width:100%; height:100%; overflow:clip; }
      #chrome { position:absolute; top:0; left:0; z-index:999; display:flex; flex-direction:column;
        width:100%; height:100%; pointer-events:none; }
      #topbar { height:60px; flex-shrink:0; background:rgb(0,255,0); pointer-events:auto; }
      .graph-canvas-panel { position:relative; flex:1; }
      #graph-canvas { position:absolute; inset:0; width:100%; height:100%; background:rgb(40,40,40); }
      [data-testid="transform-pane"] { position:absolute; inset:0; pointer-events:none;
        transform-origin:0 0; transform:scale3d(1.5,1.5,1.5) translate3d(100px,0px,0); }
      .node { position:absolute; left:300px; top:100px; width:600px; height:200px; background:rgb(0,0,255); }
      .isolate { isolation:isolate; }
      .dom-widget { position:fixed; left:500px; top:540px; width:690px; height:100px; background:rgb(255,0,0); }
    </style><div class="comfyui-body"><div id="graph-canvas-container"><div id="chrome"><div id="topbar"></div><div class="graph-canvas-panel"></div></div><canvas id="graph-canvas"></canvas><div data-testid="transform-pane"><div class="node"></div></div><div class="isolate"><div class="dom-widget"></div></div></div></div>`);
  await page.addScriptTag({ content: allocationScript });
  const layout = await page.evaluate(async () => {
    const owner = globalThis as typeof globalThis & {
      BrowserCanvasWorkspace: new (
        root: HTMLElement,
        panel: HTMLElement,
        model: unknown,
        frames: { request(callback: () => void): () => void },
        actions: { editorWidth(): void; close(): void; changed(): void; report(): void },
      ) => {
        readonly canvas: HTMLElement;
        preferences(value: { visible: boolean; editorWidth: number }): void;
      };
      CanvasWorkspaceLayout: new (defaultEditorWidth: number) => unknown;
      ensureCanvasViewportStyles(document: Document): void;
    };
    const root = document.querySelector<HTMLElement>('#graph-canvas-container')!;
    const panel = document.querySelector<HTMLElement>('.graph-canvas-panel')!;
    const workspace = new owner.BrowserCanvasWorkspace(
      root,
      panel,
      new owner.CanvasWorkspaceLayout(840),
      {
        request(callback) {
          const handle = requestAnimationFrame(callback);
          return () => cancelAnimationFrame(handle);
        },
      },
      { editorWidth() {}, close() {}, changed() {}, report() {} },
    );
    workspace.preferences({ visible: true, editorWidth: 800 });
    // A native host presents beneath the page, which leaves the docked pane transparent.
    owner.ensureCanvasViewportStyles(document);
    const viewport = document.createElement('div');
    viewport.className = 'sc-output-viewport sc-output-native';
    workspace.canvas.append(viewport);
    for (let index = 0; index < 4; index++) await new Promise(requestAnimationFrame);
    const pane = root.querySelector<HTMLElement>('.sc-output-workspace-pane')!;
    const strip = pane.getBoundingClientRect();
    return {
      root: root.getBoundingClientRect().toJSON() as unknown,
      strip: strip.toJSON() as unknown,
      paneBackground: getComputedStyle(pane).backgroundColor,
      stripTarget: pane.contains(document.elementFromPoint(1000, 400)),
      widgetTarget: document.elementFromPoint(1100, 590)?.className,
      graphWidgetTarget: document.elementFromPoint(600, 590)?.className,
    };
  });
  expect(layout).toEqual({
    root: { x: 0, y: 60, width: 800, height: 740, top: 60, right: 800, bottom: 800, left: 0 },
    strip: { x: 800, y: 60, width: 400, height: 740, top: 60, right: 1200, bottom: 800, left: 800 },
    paneBackground: 'rgba(0, 0, 0, 0)',
    stripTarget: true,
    widgetTarget: 'sc-output-viewport sc-output-native',
    graphWidgetTarget: 'dom-widget',
  });
  const image = UPNG.decode((await page.screenshot()).buffer as ArrayBuffer);
  // Graph layers render inside the allocated rectangle and never inside the transparent strip.
  expect(pixel(image, 700, 300)).toEqual([0, 0, 255]);
  expect(pixel(image, 600, 590)).toEqual([255, 0, 0]);
  expect(pixel(image, 1000, 300)).toEqual([0, 0, 0]);
  expect(pixel(image, 1100, 590)).toEqual([0, 0, 0]);
  // Chrome keeps its full-frame row above the strip.
  expect(pixel(image, 1100, 30)).toEqual([0, 255, 0]);
});
