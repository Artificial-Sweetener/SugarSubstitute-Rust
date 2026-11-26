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
/** Prove projected native footer/body material stays continuous through Comfy's negative underlap. */
import { createServer, type Server } from 'node:http';
import { readFile } from 'node:fs/promises';
import path from 'node:path';
import { chromium, firefox, type Browser } from '@playwright/test';
import { transform } from 'esbuild';
import UPNG from 'upng-js';
import {
  nativeAdvancedFooterDom,
  nativeSubgraphFooterDom,
} from '../support/NativeAdvancedFooterDom.js';
import {
  closeBrowserFixture,
  launchHeadlessBrowser,
} from '../../frontend_core/support/BrowserFixtureLifetime.js';

let server: Server;
let origin: string;
beforeAll(async () => {
  const root = process.cwd();
  const css = await transform(
    await readFile(
      path.join(root, 'frontend/comfyui/ui/surface/frame/DesktopFrameStyles.ts'),
      'utf8',
    ),
    { loader: 'ts', format: 'esm' },
  );
  server = createServer((request, response) => {
    void (async () => {
      const name = new URL(request.url ?? '/', 'http://localhost').pathname;
      if (name === '/') {
        response.setHeader('Content-Type', 'text/html');
        response.end(
          '<!doctype html><html data-sc-visual-style="fluent"><head><link rel="icon" href="data:,"></head><body><header></header><main data-sc-style-theme-root><section></section></main></body></html>',
        );
        return;
      }
      const file = path.resolve(root, 'web', '.' + decodeURIComponent(name));
      if (!file.startsWith(path.resolve(root, 'web') + path.sep)) {
        response.writeHead(404).end();
        return;
      }
      response.setHeader('Content-Type', 'text/javascript');
      response.end(
        name === '/comfyui/ui/surface/frame/DesktopFrameStyles.js'
          ? css.code
          : await readFile(file),
      );
    })().catch((error: unknown) => {
      response.writeHead(500).end(String(error));
    });
  });
  await new Promise<void>((resolve) => server.listen(0, '127.0.0.1', resolve));
  const address = server.address();
  if (!address || typeof address === 'string') throw new Error('Missing footer fixture endpoint.');
  origin = 'http://127.0.0.1:' + String(address.port);
});
afterAll(async () => {
  await closeBrowserFixture(undefined, server);
});

describe.each([
  ['Chromium', chromium],
  ['Firefox', firefox],
] as const)('%s projected footer', (_name, engine) => {
  let browser: Browser;
  beforeAll(async () => {
    browser = await launchHeadlessBrowser(engine);
  });
  afterAll(async () => {
    await browser?.close();
  });
  test.each(
    [
      { dark: true, explicit: false },
      { dark: false, explicit: false },
      { dark: true, explicit: true },
      { dark: false, explicit: true },
    ].flatMap((variant) => [
      // A cube face hides "Enter subgraph" by design, so masonry shows only the advanced chin.
      { ...variant, chin: 'Show advanced inputs', tree: nativeAdvancedFooterDom, graph: false },
      { ...variant, chin: 'Show advanced inputs', tree: nativeAdvancedFooterDom, graph: true },
      { ...variant, chin: 'Enter subgraph', tree: nativeSubgraphFooterDom, graph: true },
    ]),
  )(
    '$chin (graph=$graph) paints one card layer, one divider and a hover below it with dark=$dark explicit=$explicit',
    async ({ dark, explicit, chin, tree, graph }) => {
      const page = await browser.newPage({
        viewport: { width: 500, height: 250 },
        reducedMotion: 'reduce',
      });
      try {
        await page.goto(origin);
        await page.addStyleTag({
          content: `
    body{margin:0}body::before{content:'';position:fixed;inset:0;background:rgb(40,20,20);z-index:-1}
    main{margin:16px;width:440px}header{height:0}*{box-sizing:border-box}
    .lg-node{display:flex;flex-direction:column}
    /* A generous corner radius makes the divider's curve measurable in whole pixels. */
    .lg-node[data-sc-style-node]{--sc-node-card-radius:12px!important}.bg-node-component-header-surface{background:var(--node-component-header-surface)}
    .bg-component-node-background{background:var(--component-node-background)}
    [data-testid^="node-body-"]{height:60px;display:flex;justify-content:space-between;padding:0 30px!important;align-items:center}
    [data-testid^="node-header-"]{height:49px!important;padding-left:40px!important}
    /* Native NodeFooter's actual Tailwind geometry puts twenty pixels beneath its sibling wrapper. */
    .lg-node>.isolate{isolation:isolate;z-index:-1;margin-top:-20px;display:flex;width:100%}
    .lg-node>.isolate>button{display:inline-flex;align-items:center;justify-content:center;width:100%;height:44px;padding:32px 0 12px;font-size:12px;border-width:0;transition:none!important}
   `,
        });
        const installer = `
    import {NativeNodeCardHost} from '/comfyui/ui/surface/NativeNodeCardHost.js';
    import {ensureCubeSurfaceStyles} from '/comfyui/ui/surface/CubeSurfaceStyles.js';
    import {fluentAppearanceCss} from '/comfyui/ui/surface/appearance/FluentAppearanceStyles.js';
    import {desktopFrameCss} from '/comfyui/ui/surface/frame/DesktopFrameStyles.js';
    import {DesktopFramePresentation} from '/comfyui/ui/surface/frame/DesktopFramePresentation.js';
    globalThis.mountFooter=(tree,dark,explicit,graph)=>{
     ensureCubeSurfaceStyles(document);const style=document.createElement('style');style.textContent=fluentAppearanceCss(dark)+desktopFrameCss;document.head.append(style);
     const frame=new DesktopFramePresentation();frame.configure({schemaVersion:1,captionWidth:138,titlebarHeight:34,darkTheme:dark});frame.attach(document.querySelector('header'));
     const element=(tag,attrs={},text='')=>{const node=document.createElement(tag);for(const [key,value]of Object.entries(attrs))node.setAttribute(key,value);if(text)node.textContent=text;return node;};
     const copy=(node)=>{if(typeof node==='string')return document.createTextNode(node);const result=element(node.tag,node.attributes);result.append(...node.children.map(copy));return result;};
     const card=(target,presentation)=>{
      const root=element('div',{class:'lg-node','data-sc-style-node':'','data-sc-style-card':'',...(presentation?{'data-cube-face-presentation':presentation}:{})});
      const wrapper=element('div',{'data-testid':'node-inner-wrapper',class:'bg-node-component-header-surface'});
      const title=element('div',{'data-testid':'node-header-1'},'VAE Decode');const body=element('div',{'data-testid':'node-body-1',class:'bg-component-node-background'},'Use Tiling');
      wrapper.append(title,body);root.append(wrapper,copy(tree));target.append(root);
      if(explicit){wrapper.style.backgroundColor='#704747';body.style.backgroundColor='#405060';root.querySelector('button').style.backgroundColor='#704747';}
      return root;
     };
     // The graph shows Comfy's node card as it is; masonry shows it inside a native card.
     if(graph){card(document.querySelector('section'),null);globalThis.footerReady=true;return;}
     const host=new NativeNodeCardHost({dispose(){},mount(target,node,options){
      target.classList.add('sugarcubes-native-node-card');target.dataset.cubeFaceNative='nodes-2';
      const root=card(target,String(options.presentationGeometry));
      return{refresh(){},unmount(){root.remove();}};
     }});
     host.mount(document.querySelector('section'),[{id:'1',node:{id:1,type:'VAEDecode',widgets:[]},label:'VAE Decode',visible:true,enabled:true,showActivationControl:false}],()=>{});
     globalThis.footerReady=true;
    };
   `;
        await page.addScriptTag({ content: installer, type: 'module' });
        await page.waitForFunction(
          () => typeof Reflect.get(globalThis, 'mountFooter') === 'function',
        );
        await page.evaluate(
          ({ tree, dark, explicit, graph }) => {
            const mount = Reflect.get(globalThis, 'mountFooter') as (
              tree: unknown,
              dark: boolean,
              explicit: boolean,
              graph: boolean,
            ) => void;
            mount(tree, dark, explicit, graph);
          },
          { tree, dark, explicit, graph },
        );
        const facts = await page.evaluate(() => {
          const root = document.querySelector('section .lg-node')!;
          const body = root.querySelector('[data-testid^="node-body-"]')!;
          const button = root.querySelector(
            ':scope > [data-testid="node-inner-wrapper"] + * > button',
          )!;
          const rect = (e: Element) => {
            const r = e.getBoundingClientRect();
            return { x: r.x, y: r.y, width: r.width, height: r.height };
          };
          return {
            cube: root.hasAttribute('data-sugarcube-node'),
            root: rect(root),
            body: rect(body),
            wrapper: rect(root.querySelector(':scope > [data-testid="node-inner-wrapper"]')!),
            radius: Number.parseFloat(getComputedStyle(button).borderBottomLeftRadius),
            button: rect(button),
            desktop: document.documentElement.hasAttribute('data-sc-desktop-frame-surface'),
          };
        });
        expect(facts.desktop).toBe(true);
        expect(facts.cube).toBe(false);
        // The divider is the wrapper's lower edge, under the body; the chin shows from it down and
        // tucks up behind the body only by the corner radius, which fills the curve's corners.
        const divider = Math.round(facts.wrapper.y + facts.wrapper.height) - 1;
        expect(facts.radius).toBeGreaterThan(2);
        expect(
          Math.abs(facts.button.y + facts.radius - (facts.wrapper.y + facts.wrapper.height)),
        ).toBeLessThanOrEqual(1);
        expect(
          Math.abs(facts.wrapper.y + facts.wrapper.height - 1 - (facts.body.y + facts.body.height)),
        ).toBeLessThanOrEqual(1);
        const encoded = Uint8Array.from(await page.screenshot());
        const image = UPNG.decode(encoded.buffer);
        const data = new Uint8Array(UPNG.toRGBA8(image)[0]!);
        const pixel = (x: number, y: number) => [
          ...data.slice(
            (Math.floor(y) * image.width + Math.floor(x)) * 4,
            (Math.floor(y) * image.width + Math.floor(x)) * 4 + 4,
          ),
        ];
        const x = facts.root.x + 8;
        const body = pixel(x, facts.body.y + 12);
        const overlap = pixel(x, facts.body.y + facts.body.height - 8);
        const footer = pixel(x, facts.button.y + facts.button.height - 6);
        if (explicit) expect(body).toEqual([64, 80, 96, 255]);
        else {
          // On the desktop frame a node card is the translucent card fill: the page behind it
          // (rgb 40,20,20 here) shows through, so it is neither the style's opaque fill nor bare.
          expect(body).not.toEqual(dark ? [43, 43, 43, 255] : [251, 251, 251, 255]);
          expect(body).not.toEqual([40, 20, 20, 255]);
        }
        // One card layer: the footer and the body's lower edge paint the same pixel as the body.
        expect(overlap).toEqual(body);
        expect(footer).toEqual(explicit ? [112, 71, 71, 255] : body);
        // The divider curves: just above it at the card's edge the corner lies outside the body's
        // rounding and shows the chin, while the same height further in is still body.
        if (explicit) {
          expect(pixel(facts.root.x + 1, divider - 1)).toEqual([112, 71, 71, 255]);
          expect(pixel(facts.root.x + facts.radius + 4, divider - 2)).toEqual([64, 80, 96, 255]);
        }
        expect(await page.getByRole('button', { name: chin, exact: true }).isVisible()).toBe(true);
        // One colour per pixel row near the card's right edge, clear of every label.
        const column = (pixels: Uint8Array, width: number, y: number) => {
          const sum = [0, 0, 0];
          for (
            let at = facts.root.x + facts.root.width - 30;
            at < facts.root.x + facts.root.width - 10;
            at++
          )
            for (let channel = 0; channel < 3; channel++)
              sum[channel]! += pixels[(Math.floor(y) * width + Math.floor(at)) * 4 + channel] ?? 0;
          return sum.map((value) => Math.round(value / 20));
        };
        const differs = (left: number[], right: number[]) =>
          left.some((value, index) => Math.abs(value - (right[index] ?? 0)) > 2);
        const top = Math.ceil(facts.body.y) + 12;
        const bottom = Math.floor(facts.button.y + facts.button.height) - 2;
        const rest = Array.from({ length: bottom - top }, (_, index) =>
          column(data, image.width, top + index),
        );
        // Every change of colour from one row to the next is the one divider: the line drawn
        // where the body ends (two edges, a pixel apart), or the step to an explicitly coloured chin.
        const edges = rest.flatMap((row, index) =>
          index > 0 && differs(row, rest[index - 1]!) ? [top + index] : [],
        );
        expect(edges.length).toBeGreaterThan(0);
        for (const edge of edges) expect(edge - divider).toBeGreaterThanOrEqual(0);
        for (const edge of edges) expect(edge - divider).toBeLessThanOrEqual(1);
        // A real pointer over the chin: its hover shows, and none of it reaches above the divider.
        await page.mouse.move(
          facts.button.x + facts.button.width / 2,
          facts.button.y + facts.button.height / 2,
        );
        await page.waitForTimeout(200);
        const hovered = UPNG.decode(Uint8Array.from(await page.screenshot()).buffer);
        const hoveredData = new Uint8Array(UPNG.toRGBA8(hovered)[0]!);
        const changed = rest.flatMap((row, index) =>
          differs(column(hoveredData, hovered.width, top + index), row) ? [top + index] : [],
        );
        // An explicit node colour is inline, as in Comfy, so no hover paints over it.
        if (!explicit) expect(changed.length).toBeGreaterThan(8);
        for (const row of changed) expect(row).toBeGreaterThan(divider);
      } finally {
        await page.close();
      }
    },
  );
});
