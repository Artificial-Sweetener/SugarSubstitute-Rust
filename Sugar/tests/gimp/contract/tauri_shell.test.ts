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
import {
  closeBrowserFixture,
  launchHeadlessBrowser,
} from '../../frontend_core/support/BrowserFixtureLifetime.js';
import { companionAssets } from '../../../scripts/gimp/build.js';

let server: Server;
const engines = ['chromium', 'firefox'] as const;
type Engine = (typeof engines)[number];
const scenarios = ['unavailable', 'native-document', 'panel-admission'] as const;
type Scenario = (typeof scenarios)[number];
const browsers = new Map<Engine, Browser>();
const launchFailures = new Map<Engine, unknown>();
const pages = new Map<Scenario, Map<Engine, Page>>();
interface ActivePage {
  page: Page;
  closing?: Promise<void>;
}
let activePage: ActivePage | undefined;
let address: string;

/** Claim one preallocated page exactly once before any case setup can block. */
function claimPage(scenario: Scenario, engine: Engine): Page {
  if (activePage) throw new Error('A Tauri shell page is already active.');
  if (launchFailures.has(engine))
    throw new Error(`The ${engine} browser fixture did not launch.`, {
      cause: launchFailures.get(engine),
    });
  const candidates = pages.get(scenario);
  const page = candidates?.get(engine);
  if (!candidates || !page) throw new Error(`Missing ${scenario} ${engine} page fixture.`);
  candidates.delete(engine);
  if (candidates.size === 0) pages.delete(scenario);
  activePage = { page };
  return page;
}

/** Retire only the currently claimed page, leaving a later case's custody untouched. */
async function closeClaimedPage(): Promise<void> {
  const claimed = activePage;
  if (!claimed) return;
  claimed.closing ??= claimed.page.isClosed() ? Promise.resolve() : claimed.page.close();
  await claimed.closing;
  if (activePage === claimed) activePage = undefined;
}

/** Close every browser even when another browser reports a teardown failure. */
async function closeBrowsers(): Promise<void> {
  const outcomes = await Promise.allSettled(
    [...browsers.values()].map(async (browser) => browser.close()),
  );
  const causes = outcomes.flatMap((outcome): unknown[] =>
    outcome.status === 'rejected' ? [outcome.reason] : [],
  );
  if (causes.length) throw new AggregateError(causes, 'Tauri shell browser cleanup failed');
}

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
  // A browser that cannot launch fails only its own variants when they claim a page.
  await Promise.all(
    engines.map(async (engine) => {
      try {
        browsers.set(
          engine,
          await launchHeadlessBrowser(engine === 'chromium' ? chromium : firefox),
        );
      } catch (cause) {
        launchFailures.set(engine, cause);
      }
    }),
  );
  for (const scenario of scenarios) {
    const candidates = new Map<Engine, Page>();
    pages.set(scenario, candidates);
    for (const [engine, browser] of browsers) {
      const page = await browser.newPage();
      candidates.set(engine, page);
      await configurePage(page, scenario === 'native-document', scenario === 'panel-admission');
    }
  }
});

afterAll(async () => {
  await closeBrowserFixture({ close: closeBrowsers }, server);
});

afterEach(async () => {
  await closeClaimedPage();
});

/** Replace only Tauri's native operation before exercising the compiled product shell. */
async function configurePage(page: Page, native = false, admissionFailure = false): Promise<void> {
  await page.addInitScript(
    ({ native, admissionFailure }) => {
      let calls = 0;
      let openings = 0;
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
              openings++;
              Reflect.set(globalThis, 'nativePanelCalls', openings);
              if (admissionFailure && openings === 1)
                return Promise.reject(new Error('Native panel admission failed.'));
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
            if (calls > 1 || admissionFailure)
              queueMicrotask(() => {
                receive({ type: 'ready' });
              });
            return Promise.resolve({
              phase: calls === 1 && !admissionFailure ? 'disconnected' : 'connected',
              address: 'http://127.0.0.1:8226/',
              message:
                calls === 1 && !admissionFailure
                  ? '<img src=x onerror=boom> Start ComfyUI, then retry.'
                  : 'Connected to ComfyUI.',
            });
          },
        },
      });
    },
    { native, admissionFailure },
  );
  await page.goto(address);
}

test.each(engines)(
  'shows unavailable-server feedback literally and retries through the native port: %s',
  async (engine) => {
    const view = claimPage('unavailable', engine);
    await view.getByRole('heading', { name: 'ComfyUI is not connected' }).waitFor();
    expect(await view.locator('#connection-detail').textContent()).toContain(
      '<img src=x onerror=boom>',
    );
    expect(await view.locator('img').count()).toBe(0);
    expect(await view.getByRole('button', { name: 'Retry' }).isEnabled()).toBe(true);
    await view.getByRole('button', { name: 'Retry' }).click();
    await view.getByRole('heading', { name: 'Connected', exact: true }).waitFor();
    expect(await view.evaluate(() => Reflect.get(globalThis, 'nativeConnectionCalls'))).toBe(2);
    expect(await view.locator('#connection-address').textContent()).toBe('http://127.0.0.1:8226/');
  },
);

test.each(engines)(
  'shows the native launch image literally while ComfyUI is disconnected: %s',
  async (engine) => {
    const view = claimPage('native-document', engine);
    await view.getByRole('heading', { name: 'ComfyUI is not connected' }).waitFor();
    await view.locator('#native-document').waitFor({ state: 'visible' });
    expect(await view.locator('#native-document').textContent()).toBe(
      '<img src=x onerror=boom> Native GIMP image',
    );
    expect(await view.locator('img').count()).toBe(0);
    expect(await view.getByRole('button', { name: 'Retry' }).isEnabled()).toBe(true);
  },
);

test.each(engines)(
  'retries native panel admission immediately through the compiled shell: %s',
  async (engine) => {
    const view = claimPage('panel-admission', engine);
    await view.getByRole('heading', { name: 'ComfyUI is not connected' }).waitFor();
    expect(await view.evaluate(() => Reflect.get(globalThis, 'nativePanelCalls'))).toBe(1);
    await view.getByRole('button', { name: 'Retry' }).click();
    await view.getByRole('heading', { name: 'Connected', exact: true }).waitFor();
    expect(await view.evaluate(() => Reflect.get(globalThis, 'nativePanelCalls'))).toBe(2);
    expect(await view.evaluate(() => Reflect.get(globalThis, 'nativeConnectionCalls'))).toBe(1);
  },
);
