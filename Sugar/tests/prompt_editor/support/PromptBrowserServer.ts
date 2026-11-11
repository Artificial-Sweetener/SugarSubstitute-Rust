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
/** Serve the bundled prompt browser proof to one headless Chromium for integration modules. */
import { createServer, type Server } from 'node:http';
import { build } from 'esbuild';
import { chromium, type Browser, type Page } from '@playwright/test';
import type {} from './BrowserProof.js';
import {
  closeBrowserFixture,
  launchHeadlessBrowser,
} from '../../frontend_core/support/BrowserFixtureLifetime.js';

/** One running fixture: its origin, its browser and the release of both. */
export interface PromptBrowserFixture {
  readonly address: string;
  readonly browser: Browser;
  close(): Promise<void>;
}

/** Bundle the real prompt host once, serve it with decoded proof media and launch the browser. */
export async function startPromptBrowser(): Promise<PromptBrowserFixture> {
  const result = await build({
    entryPoints: ['tests/prompt_editor/support/BrowserProof.ts'],
    bundle: true,
    format: 'esm',
    platform: 'browser',
    target: 'es2022',
    write: false,
    loader: { '.txt': 'text' },
    define: {
      'process.env.NODE_ENV': '"production"',
      __VUE_OPTIONS_API__: 'false',
      __VUE_PROD_DEVTOOLS__: 'false',
      __VUE_PROD_HYDRATION_MISMATCH_DETAILS__: 'false',
    },
  });
  const script = result.outputFiles[0]?.contents;
  if (!script) throw new Error('Missing prompt browser fixture');
  const server: Server = createServer((request, response) => {
    if (request.url === '/fixture.js') {
      response.writeHead(200, { 'Content-Type': 'application/javascript' });
      response.end(script);
    } else if (request.url?.startsWith('/proof/')) {
      response.writeHead(200, { 'Content-Type': 'image/svg+xml' });
      response.end(
        '<svg xmlns="http://www.w3.org/2000/svg" width="512" height="128"><rect width="512" height="128" fill="#3060a0"/><path d="M0 128L160 0 320 128Z" fill="#e09040"/></svg>',
      );
    } else {
      response.writeHead(200, { 'Content-Type': 'text/html' });
      response.end(
        '<!doctype html><html class="dark-theme" style="color-scheme:dark"><meta charset="utf-8"><style>body{margin:24px;background:#202020;color:#eee}textarea{width:700px}</style><script type="module" src="/fixture.js"></script></html>',
      );
    }
  });
  await new Promise<void>((resolve) => server.listen(0, '127.0.0.1', resolve));
  const listening = server.address();
  if (!listening || typeof listening === 'string') throw new Error('Missing fixture address');
  const browser = await launchHeadlessBrowser(chromium);
  return {
    address: `http://127.0.0.1:${String(listening.port)}`,
    browser,
    close: () => closeBrowserFixture(browser, server),
  };
}

/** Open a fresh page with one mounted prompt and wait for its editable surface. */
export async function mountPrompt(
  fixture: PromptBrowserFixture,
  renderer: 'nodes-1' | 'nodes-2',
  source: string,
  scale = 1,
): Promise<Page> {
  const page = await fixture.browser.newPage();
  await page.goto(fixture.address);
  await page.waitForFunction(() => !!window.promptBrowserProof);
  await page.evaluate(
    ({ renderer, scale, source }) => window.promptBrowserProof.mount(renderer, scale, source),
    { renderer, scale, source },
  );
  await page.locator('[data-sc-prompt-editor] .cm-content').waitFor({ state: 'visible' });
  return page;
}

/** Wait until the host widget publishes exactly this prompt source. */
export async function expectSource(page: Page, expected: string): Promise<void> {
  try {
    await page.waitForFunction(
      (value) => window.promptBrowserProof.read().source === value,
      expected,
      { timeout: 5_000 },
    );
  } catch (cause) {
    const actual = await page.evaluate(() => window.promptBrowserProof.read().source);
    throw new Error(`Expected source ${JSON.stringify(expected)}, got ${JSON.stringify(actual)}`, {
      cause,
    });
  }
}
