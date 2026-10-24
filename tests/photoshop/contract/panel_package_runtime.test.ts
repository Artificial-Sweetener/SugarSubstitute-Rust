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
/** Exercise the actual generated UXP package through a deterministic native-host boundary. */
import { copyFileSync, cpSync, mkdirSync, mkdtempSync, readFileSync, rmSync } from 'node:fs';
import path from 'node:path';
import { runInNewContext } from 'node:vm';
import { buildPhotoshop } from '../../../scripts/photoshop/build.js';
import type { PanelMessage } from '../../../integrations/contracts/PanelProtocol.generated.js';

type Listener = (event: unknown) => void;
interface Lifecycle {
  create(): void;
  show(): void;
  hide(): void;
  destroy(): void;
}

/** Build the authored package in a private repository snapshot without racing developer output. */
async function packageSource(): Promise<string> {
  const repository = path.resolve('.');
  const parent = path.join(repository, '.generated/photoshop-native-tests');
  mkdirSync(parent, { recursive: true });
  const fixture = mkdtempSync(path.join(parent, 'package-'));
  if (!fixture.startsWith(`${parent}${path.sep}`))
    throw new Error('Unsafe native package fixture.');
  try {
    for (const directory of ['photoshop', 'contracts', 'shared'])
      cpSync(
        path.join(repository, 'integrations', directory),
        path.join(fixture, 'integrations', directory),
        { recursive: true },
      );
    for (const name of [
      'tsconfig.photoshop.json',
      'tsconfig.integrations.json',
      'tsconfig.json',
      'LICENSE',
    ])
      copyFileSync(path.join(repository, name), path.join(fixture, name));
    expect(await buildPhotoshop(fixture, false)).toBe(true);
    return readFileSync(path.join(fixture, 'build/photoshop/main.js'), 'utf8');
  } finally {
    rmSync(fixture, { recursive: true, force: true });
  }
}

/** Model only native element behavior used by the package boundary. */
class NativeElement {
  textContent: string | null = '';
  readonly style = { display: '' };
  readonly listeners = new Map<string, Set<Listener>>();
  src = '';
  readonly sent: { message: PanelMessage; origin: string }[] = [];
  /** Reflect the native display attribute used by the panel's owned status elements. */
  setAttribute(name: string, value: string): void {
    if (name === 'style') this.style.display = value.replace('display:', '');
  }
  /** Register one listener with DOM-compatible identity semantics. */
  addEventListener(name: string, listener: Listener): void {
    const group = this.listeners.get(name) ?? new Set<Listener>();
    group.add(listener);
    this.listeners.set(name, group);
  }
  /** Release exactly the registered callback. */
  removeEventListener(name: string, listener: Listener): void {
    this.listeners.get(name)?.delete(listener);
  }
  /** Deliver a native event without interpreting its untrusted payload. */
  emit(name: string, value: unknown): void {
    for (const listener of this.listeners.get(name) ?? []) listener(value);
  }
  /** Record the origin restriction passed by the native transport. */
  postMessage(message: PanelMessage, origin: string): void {
    this.sent.push({ message, origin });
  }
}

test('packaged entrypoint filters messages, acknowledges presentation, preserves hide/show, and cleans up', async () => {
  const source = await packageSource();
  const nodes = new Map(
    ['comfyui', 'connection', 'connection-title', 'connection-message', 'retry'].map((id) => [
      id,
      new NativeElement(),
    ]),
  );
  const nativeWindow = new NativeElement();
  const timers = new Map<number, () => void>();
  const lifecycles: Lifecycle[] = [];
  const errors: unknown[] = [];
  const grippers: unknown[] = [];
  const themeListeners = new Set<() => void>();
  const storedValues = new Map<string, string>();
  let nativeTheme = 'darkest';
  let nextSession = 0;
  let nextTimer = 0;
  runInNewContext(
    source,
    {
      module: { exports: {} },
      require: (name: string): unknown => {
        if (name === 'photoshop')
          return {
            core: {
              suppressResizeGripper: async (value: unknown) => {
                grippers.push(value);
              },
            },
          };
        if (name !== 'uxp') throw new Error(`Unexpected native module: ${name}`);
        return {
          entrypoints: {
            setup: (value: {
              plugin: { create(): void; destroy(): void };
              panels: { sugarcubes: Lifecycle };
            }): void => {
              expect(typeof value.plugin.create).toBe('function');
              expect(typeof value.plugin.destroy).toBe('function');
              value.plugin.create();
              lifecycles.push(value.panels.sugarcubes);
            },
          },
        };
      },
      HTMLElement: NativeElement,
      document: {
        getElementById: (id: string) => nodes.get(id) ?? null,
        theme: {
          getCurrent: () => nativeTheme,
          onUpdated: {
            addListener: (callback: () => void) => themeListeners.add(callback),
            removeListener: (callback: () => void) => themeListeners.delete(callback),
          },
        },
      },
      window: nativeWindow,
      localStorage: {
        getItem: (key: string) => storedValues.get(key) ?? null,
        setItem: (key: string, value: string) => storedValues.set(key, value),
        removeItem: (key: string) => storedValues.delete(key),
      },
      crypto: {
        randomUUID: () => `00000000-0000-4000-8000-${String(++nextSession).padStart(12, '0')}`,
      },
      console: { info: () => undefined, error: (...args: unknown[]) => errors.push(args) },
      setInterval: (tick: () => void) => {
        const id = ++nextTimer;
        timers.set(id, tick);
        return id;
      },
      clearInterval: (id: number) => timers.delete(id),
      encodeURIComponent,
      URL,
    },
    { timeout: 1000 },
  );
  const lifecycle = lifecycles[0];
  expect(lifecycle).toBeDefined();
  lifecycle?.create();
  const webview = nodes.get('comfyui');
  expect(webview).toBeDefined();
  const url = new URL(webview?.src ?? '');
  const session = url.searchParams.get('sc-panel-session') ?? '';
  expect(url.origin).toBe('http://127.0.0.1:8188');
  expect(url.searchParams.get('sc-panel-host')).toBe('ai.artificialsweetener.sugarcubes.photoshop');
  expect(timers.size).toBe(1);
  expect(grippers).toEqual([{ type: 'panel', target: 'sugarcubes', value: true }]);
  expect(themeListeners.size).toBe(1);
  webview?.emit('loadstart', { url: 'about:blank' });
  expect(errors).toHaveLength(0);
  const ready: PanelMessage = { protocol: 'sugarcubes.panel', version: 3, session, type: 'ready' };
  for (const event of [
    { origin: 'https://untrusted.invalid', source: webview, data: ready },
    { origin: `${url.origin}.untrusted.invalid/`, source: webview, data: ready },
    { origin: `${url.origin}@untrusted.invalid/`, source: webview, data: ready },
    { origin: 'malformed-origin', source: webview, data: ready },
    { origin: url.href, source: {}, data: ready },
    { origin: url.href, source: webview, data: { ...ready, session: 'stale-session-000000' } },
  ])
    nativeWindow.emit('message', event);
  expect(webview?.sent).toEqual([]);
  nativeWindow.emit('message', { origin: url.href, source: webview, data: ready });
  expect(webview?.sent[0]).toMatchObject({
    origin: url.origin,
    message: {
      type: 'configure',
      presentation: {
        pluginMode: true,
        visualStyle: 'photopro',
        photoProPalette: 'dark-gray',
        accentColor: '#d6d6d6',
        resizeCornerSize: 0,
      },
    },
  });
  expect(nodes.get('connection')?.style.display).toBe('block');
  nativeWindow.emit('message', {
    origin: url.href,
    source: webview,
    data: { ...ready, type: 'applied' },
  });
  expect(webview?.sent.at(-1)?.message.type).toBe('ping');
  nativeWindow.emit('message', {
    origin: `${url.origin}/#workflow-selected-by-comfy`,
    source: webview,
    data: { ...ready, type: 'pong', connected: true },
  });
  expect(nodes.get('connection')?.style.display).toBe('none');
  nativeTheme = 'light';
  for (const changed of themeListeners) changed();
  expect(webview?.sent.at(-1)?.message).toMatchObject({
    type: 'configure',
    session,
    presentation: {
      photoProPalette: 'light-gray',
      accentColor: '#d6d6d6',
      resizeCornerSize: 0,
    },
  });
  expect(webview?.src).toBe(url.href);
  for (const [theme, palette] of [
    ['dark', 'medium-gray'],
    ['lightest', 'light-gray'],
    ['darkest', 'dark-gray'],
    ['light', 'light-gray'],
  ]) {
    nativeTheme = theme ?? '';
    for (const changed of themeListeners) changed();
    expect(webview?.sent.at(-1)?.message).toMatchObject({
      type: 'configure',
      presentation: {
        photoProPalette: palette,
        accentColor: '#d6d6d6',
        resizeCornerSize: 0,
      },
    });
    expect(webview?.src).toBe(url.href);
  }
  const messageCount = webview?.sent.length;
  for (const changed of themeListeners) changed();
  expect(webview?.sent).toHaveLength(messageCount ?? 0);
  nativeTheme = 'unsupported';
  for (const changed of themeListeners) changed();
  expect(errors).toHaveLength(1);
  expect(webview?.sent).toHaveLength(messageCount ?? 0);
  nativeTheme = 'light';
  errors.length = 0;
  lifecycle?.hide();
  expect(timers.size).toBe(0);
  lifecycle?.show();
  expect(timers.size).toBe(1);
  expect(webview?.src).toBe(url.href);
  webview?.emit('loaderror', {});
  expect(nodes.get('connection')?.style.display).toBe('block');
  expect(nodes.get('retry')?.style.display).toBe('block');
  expect(errors).toHaveLength(1);
  nodes.get('retry')?.emit('click', {});
  expect(webview?.src).not.toBe(url.href);
  expect(timers.size).toBe(1);
  lifecycle?.destroy();
  expect(timers.size).toBe(0);
  expect(nativeWindow.listeners.get('message')?.size).toBe(0);
  expect(webview?.listeners.get('loaderror')?.size).toBe(0);
  expect(webview?.src).toBe('about:blank');
  expect(themeListeners.size).toBe(0);
});
