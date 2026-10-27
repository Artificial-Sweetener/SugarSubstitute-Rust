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

/** Verify source-picker interaction refreshes capture-only previews without a maintenance control. */
import { createServer, type Server } from 'node:http';
import { mkdirSync } from 'node:fs';
import path from 'node:path';
import { build } from 'esbuild';
import { chromium, firefox } from '@playwright/test';
import { launchHeadlessBrowser } from '../../frontend_core/support/BrowserFixtureLifetime.js';
import type {} from '../support/CaptureOnlyBrowserProof.js';

let server: Server;
let address: string;
beforeAll(async () => {
  const output = await build({
    entryPoints: ['tests/input_canvas/support/CaptureOnlyBrowserProof.ts'],
    bundle: true,
    write: false,
    format: 'esm',
    platform: 'browser',
    target: 'es2022',
    define: {
      'process.env.NODE_ENV': '"production"',
      __VUE_OPTIONS_API__: 'false',
      __VUE_PROD_DEVTOOLS__: 'false',
      __VUE_PROD_HYDRATION_MISMATCH_DETAILS__: 'false',
    },
  });
  const script = output.outputFiles[0]?.text;
  if (!script) throw new Error('No capture-only browser fixture was compiled.');
  server = createServer((request, response) => {
    response.writeHead(200, {
      'Content-Type': request.url === '/fixture.js' ? 'application/javascript' : 'text/html',
    });
    response.end(
      request.url === '/fixture.js'
        ? script
        : '<!doctype html><html><meta charset="utf-8"><style>body{margin:20px;color:#eee;background:#3a3a3a;font:12px "Segoe UI",sans-serif}.sc-node-card{width:340px;background:#3a3a3a;border:1px solid #222;padding:8px}h1{font-size:14px}h2{font-size:13px;margin:0 0 10px}p{max-width:340px;color:#bbb}fieldset{margin-top:8px}</style><h1>Capture-only GIMP test adapter</h1><p>Shared SugarCubes input editor. Native GIMP pixels are replaced by a test boundary.</p><script type="module" src="/fixture.js"></script></html>',
    );
  });
  await new Promise<void>((resolve) => server.listen(0, '127.0.0.1', resolve));
  const endpoint = server.address();
  if (!endpoint || typeof endpoint === 'string') throw new Error('No browser fixture endpoint.');
  address = `http://127.0.0.1:${String(endpoint.port)}`;
}, 60_000);

afterAll(async () => {
  server.closeAllConnections();
  await new Promise<void>((resolve, reject) =>
    server.close((error) => (error ? reject(error) : resolve())),
  );
});

test.each(['chromium', 'firefox'] as const)(
  'refreshes capture-only previews in %s without background pixel polling',
  async (name) => {
    const browser = await launchHeadlessBrowser(name === 'chromium' ? chromium : firefox);
    try {
      const page = await browser.newPage({ viewport: { width: 520, height: 640 } });
      await page.goto(address);
      await page.waitForFunction(() => window.captureOnlyProof?.read().captures > 0);
      expect(
        await page.getByRole('button', { name: 'Refresh canvas sources', exact: true }).count(),
      ).toBe(0);
      expect(
        await page.getByText('Refresh previews after editing.', { exact: false }).count(),
      ).toBe(0);
      const initial = await page.evaluate(() => window.captureOnlyProof.read());
      await page.evaluate(() => window.captureOnlyProof.edit());
      await page.waitForTimeout(150);
      expect((await page.evaluate(() => window.captureOnlyProof.read())).captures).toBe(
        initial.captures,
      );
      await page.getByRole('combobox').click();
      await page.waitForFunction(
        (before) => window.captureOnlyProof.read().captures > before,
        initial.captures,
      );
      await page.waitForFunction(() => {
        const canvas = document.querySelector('canvas');
        const pixels = canvas
          ?.getContext('2d')
          ?.getImageData(canvas.width / 2, canvas.height / 2, 1, 1).data;
        return pixels?.[0] === 40 && pixels[2] === 180;
      });
      const refreshed = await page.evaluate(() => window.captureOnlyProof.read());
      expect(refreshed.authored).toBe(initial.authored);
      expect(refreshed.errors).toEqual([]);
      const artifacts = process.env.SUGARCUBES_CAPTURE_ARTIFACT_ROOT;
      if (artifacts) {
        mkdirSync(artifacts, { recursive: true });
        await page.screenshot({ path: path.join(artifacts, `capture-only-refresh-${name}.png`) });
      }
      await page.evaluate(() => window.captureOnlyProof.dispose());
    } finally {
      await browser.close();
    }
  },
  60_000,
);
