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
/**
 * Prove the actual local shell renders and retries without a running ComfyUI server.
 */
import { createServer, type Server } from 'node:http';
import { chromium, firefox, type Browser, type Page } from '@playwright/test';
import { companionAssets } from '../../../scripts/gimp/build.js';

let server: Server;
const engines = ['chromium', 'firefox'] as const;
type Engine = (typeof engines)[number];
const browsers = new Map<Engine, Browser>();
let address: string;

beforeAll(async () => {
  const assets = await companionAssets(process.cwd());
  server = createServer((request, response) => {
    const name = request.url === '/' ? 'index.html' : request.url?.slice(1);
    const asset = name ? assets.get(name) : undefined;
    if (!asset) {
      response.writeHead(404);
      response.end();
      return;
    }
    response.writeHead(200, {
      'Content-Type': name?.endsWith('.js')
        ? 'application/javascript'
        : name?.endsWith('.css')
          ? 'text/css'
          : 'text/html',
    });
    response.end(asset);
  });
  await new Promise<void>((resolve) => {
    server.listen(0, '127.0.0.1', resolve);
  });
  const endpoint = server.address();
  if (!endpoint || typeof endpoint === 'string')
    throw new Error('Missing companion fixture endpoint.');
  address = `http://127.0.0.1:${String(endpoint.port)}`;
  const launches = await Promise.allSettled(
    engines.map(async (engine) => {
      browsers.set(
        engine,
        await (engine === 'chromium' ? chromium : firefox).launch({ headless: true }),
      );
    }),
  );
  const failure = launches.find((result) => result.status === 'rejected');
  if (failure?.status === 'rejected') throw failure.reason;
}, 60_000);

afterAll(async () => {
  await Promise.all([...browsers.values()].map((browser) => browser.close()));
  if (server?.listening)
    await new Promise<void>((resolve, reject) => {
      server.close((error) => {
        if (error) reject(error);
        else resolve();
      });
    });
});

/** Replace only Tauri's native operation while exercising the compiled product shell. */
async function page(native = false, engine: Engine = 'chromium'): Promise<Page> {
  const browser = browsers.get(engine);
  if (!browser) throw new Error('Missing companion proof browser.');
  const page = await browser.newPage();
  await page.addInitScript((native) => {
    let calls = 0;
    let current: { onmessage: (message: unknown) => void } | undefined;
    let session = '';
    class Channel {
      onmessage: (message: unknown) => void = () => {};
    }
    const receive = (message: Readonly<Record<string, unknown>>): void => {
      current?.onmessage({ protocol: 'sugarcubes.panel', version: 3, session, ...message });
    };
    Reflect.set(globalThis, '__TAURI__', {
      core: {
        Channel,
        invoke: (
          command: unknown,
          args?: { channel?: Channel; session?: string; message?: { type: string } },
        ) => {
          if (command === 'panel_open') {
            current = args?.channel;
            session = args?.session ?? '';
            return Promise.resolve(null);
          }
          if (command === 'panel_visible') return Promise.resolve(null);
          if (command === 'panel_send') {
            if (args?.message?.type === 'configure') receive({ type: 'applied' });
            if (args?.message?.type === 'ping') receive({ type: 'pong', connected: true });
            return Promise.resolve(null);
          }
          if (command === 'native_open') return Promise.resolve(native ? 1 : null);
          if (command === 'native_submit') return Promise.resolve(7);
          if (command === 'native_wait')
            return Promise.resolve({
              kind: 'context',
              hostId: 'fixture-host',
              document: {
                imageId: 1,
                revision: 'geometry',
                width: 8,
                height: 6,
                name: '<img src=x onerror=boom> Native GIMP image',
              },
              layers: [],
              primaryLayer: null,
              rgb: true,
            });
          if (command === 'native_close' || command === 'native_cancel')
            return Promise.resolve(null);
          if (command !== 'connect_comfy')
            return Promise.reject(new Error('Unexpected native command.'));
          calls++;
          Reflect.set(globalThis, 'nativeConnectionCalls', calls);
          if (calls > 1)
            queueMicrotask(() => {
              receive({ type: 'ready' });
            });
          return Promise.resolve({
            phase: calls === 1 ? 'disconnected' : 'connected',
            address: 'http://127.0.0.1:8226/',
            message:
              calls === 1
                ? '<img src=x onerror=boom> Start ComfyUI, then retry.'
                : 'Connected to ComfyUI.',
          });
        },
      },
    });
  }, native);
  await page.goto(address);
  return page;
}

test.each(engines)(
  'shows unavailable-server feedback literally and retries through the native port: %s',
  async (engine) => {
    const view = await page(false, engine);
    try {
      await view.getByRole('heading', { name: 'ComfyUI is not connected' }).waitFor();
      expect(await view.locator('#connection-detail').textContent()).toContain(
        '<img src=x onerror=boom>',
      );
      expect(await view.locator('img').count()).toBe(0);
      expect(await view.getByRole('button', { name: 'Retry' }).isEnabled()).toBe(true);
      await view.getByRole('button', { name: 'Retry' }).click();
      await view.getByRole('heading', { name: 'Connected', exact: true }).waitFor();
      expect(await view.evaluate(() => Reflect.get(globalThis, 'nativeConnectionCalls'))).toBe(2);
      expect(await view.locator('#connection-address').textContent()).toBe(
        'http://127.0.0.1:8226/',
      );
    } finally {
      await view.close();
    }
  },
);

test.each(engines)(
  'shows the native launch image literally while ComfyUI is disconnected: %s',
  async (engine) => {
    const view = await page(true, engine);
    try {
      await view.getByRole('heading', { name: 'ComfyUI is not connected' }).waitFor();
      await view.locator('#native-document').waitFor({ state: 'visible' });
      expect(await view.locator('#native-document').textContent()).toBe(
        '<img src=x onerror=boom> Native GIMP image',
      );
      expect(await view.locator('img').count()).toBe(0);
      expect(await view.getByRole('button', { name: 'Retry' }).isEnabled()).toBe(true);
    } finally {
      await view.close();
    }
  },
);
