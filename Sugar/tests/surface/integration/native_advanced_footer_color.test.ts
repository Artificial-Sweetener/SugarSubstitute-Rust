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
import { nativeAdvancedFooterDom } from '../support/NativeAdvancedFooterDom.js';
import {
  closeBrowserFixture,
  launchHeadlessBrowser,
  BROWSER_FIXTURE_SETUP_TIMEOUT_MS,
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
}, BROWSER_FIXTURE_SETUP_TIMEOUT_MS);
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
  }, BROWSER_FIXTURE_SETUP_TIMEOUT_MS);
  afterAll(async () => {
    await browser?.close();
  });
  test.each([
    { dark: true, explicit: false },
    { dark: false, explicit: false },
    { dark: true, explicit: true },
    { dark: false, explicit: true },
  ])('paints one card layer with dark=$dark explicit=$explicit', async ({ dark, explicit }) => {
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
    .lg-node{display:flex;flex-direction:column}.bg-node-component-header-surface{background:var(--node-component-header-surface)}
    .bg-component-node-background{background:var(--component-node-background)}
    [data-testid^="node-body-"]{height:60px;display:flex;justify-content:space-between;padding:0 30px!important;align-items:center}
    [data-testid^="node-header-"]{height:49px!important;padding-left:40px!important}
    /* Native NodeFooter's actual Tailwind geometry puts twenty pixels beneath its sibling wrapper. */
    .lg-node>.isolate{isolation:isolate;z-index:-1;margin-top:-20px;display:flex;width:100%}
    [data-testid="advanced-inputs-button"]{display:inline-flex;align-items:center;justify-content:center;width:100%;height:44px;padding:32px 0 12px;font-size:12px;border-width:0;transition:none!important}
   `,
      });
      const installer = `
    import {NativeNodeCardHost} from '/comfyui/ui/surface/NativeNodeCardHost.js';
    import {ensureCubeSurfaceStyles} from '/comfyui/ui/surface/CubeSurfaceStyles.js';
    import {fluentAppearanceCss} from '/comfyui/ui/surface/appearance/FluentAppearanceStyles.js';
    import {desktopFrameCss} from '/comfyui/ui/surface/frame/DesktopFrameStyles.js';
    import {DesktopFramePresentation} from '/comfyui/ui/surface/frame/DesktopFramePresentation.js';
    globalThis.mountFooter=(tree,dark,explicit)=>{
     ensureCubeSurfaceStyles(document);const style=document.createElement('style');style.textContent=fluentAppearanceCss(dark)+desktopFrameCss;document.head.append(style);
     const frame=new DesktopFramePresentation();frame.configure({schemaVersion:1,captionWidth:138,titlebarHeight:34,darkTheme:dark});frame.attach(document.querySelector('header'));
     const element=(tag,attrs={},text='')=>{const node=document.createElement(tag);for(const [key,value]of Object.entries(attrs))node.setAttribute(key,value);if(text)node.textContent=text;return node;};
     const copy=(node)=>{if(typeof node==='string')return document.createTextNode(node);const result=element(node.tag,node.attributes);result.append(...node.children.map(copy));return result;};
     const host=new NativeNodeCardHost({dispose(){},mount(target,node,options){
      target.classList.add('sugarcubes-native-node-card');target.dataset.cubeFaceNative='nodes-2';
      const root=element('div',{class:'lg-node','data-sc-style-node':'','data-cube-face-presentation':String(options.presentationGeometry)});
      const wrapper=element('div',{'data-testid':'node-inner-wrapper',class:'bg-node-component-header-surface'});
      const title=element('div',{'data-testid':'node-header-1'},'VAE Decode');const body=element('div',{'data-testid':'node-body-1',class:'bg-component-node-background'},'Use Tiling');
      wrapper.append(title,body);root.append(wrapper,copy(tree));target.append(root);
      if(explicit){wrapper.style.backgroundColor='#704747';body.style.backgroundColor='#405060';root.querySelector('button').style.backgroundColor='#704747';}
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
        ({ tree, dark, explicit }) => {
          const mount = Reflect.get(globalThis, 'mountFooter') as (
            tree: unknown,
            dark: boolean,
            explicit: boolean,
          ) => void;
          mount(tree, dark, explicit);
        },
        { tree: nativeAdvancedFooterDom, dark, explicit },
      );
      const facts = await page.evaluate(() => {
        const root = document.querySelector('.sugarcubes-native-node-card>.lg-node')!;
        const body = root.querySelector('[data-testid^="node-body-"]')!;
        const button = root.querySelector('[data-testid="advanced-inputs-button"]')!;
        const rect = (e: Element) => {
          const r = e.getBoundingClientRect();
          return { x: r.x, y: r.y, width: r.width, height: r.height };
        };
        return {
          cube: root.hasAttribute('data-sugarcube-node'),
          root: rect(root),
          body: rect(body),
          button: rect(button),
          desktop: document.documentElement.hasAttribute('data-sc-desktop-frame-surface'),
        };
      });
      expect(facts.desktop).toBe(true);
      expect(facts.cube).toBe(false);
      expect(facts.button.y).toBeLessThan(facts.body.y + facts.body.height);
      expect(facts.button.y + facts.button.height).toBeGreaterThan(
        facts.body.y + facts.body.height,
      );
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
      expect(body).toEqual(
        explicit ? [64, 80, 96, 255] : dark ? [50, 31, 31, 255] : [191, 185, 185, 255],
      );
      expect(overlap).toEqual(body);
      expect(footer).toEqual(explicit ? [112, 71, 71, 255] : body);
      expect(
        await page.getByRole('button', { name: 'Show advanced inputs', exact: true }).isVisible(),
      ).toBe(true);
    } finally {
      await page.close();
    }
  });
});
