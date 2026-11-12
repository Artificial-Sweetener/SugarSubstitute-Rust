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
/** Prove native viewer injection uses the shared embedded lifecycle without weakening UXP admission. */
import { afterEach, expect, jest, test } from '@jest/globals';
import { startEmbeddedPanel } from '../../../frontend/comfyui/ui/core/embedded/EmbeddedPanelHost.js';
import { EmbeddedPresentation } from '../../../frontend/comfyui/ui/core/embedded/EmbeddedPresentation.js';
import { HostAccent } from '../../../frontend/comfyui/ui/core/embedded/HostAccent.js';
import { HostDescriptionState } from '../../../frontend/comfyui/ui/core/embedded/HostDescriptionState.js';
import { panelNavigation } from '../../../frontend/comfyui/ui/core/embedded/BrowserPanelTransport.js';
import { createNativePanelTransport } from '../../../frontend/comfyui/ui/core/embedded/NativePanelTransport.js';
import {
  registerVisualStylePreference,
  VISUAL_STYLE_SETTING,
} from '../../../frontend/comfyui/ui/surface/appearance/VisualStylePreference.js';
import {
  registerPluginModePreference,
  PLUGIN_MODE_SETTING,
} from '../../../frontend/comfyui/ui/surface/frame/PluginModePreference.js';

const session = 'fixture-session-0001';
const hostId = 'fixture.editor.panel';

/** Describe a fixture host serving only the panel, with the given capabilities. */
function description(capabilities: readonly string[]) {
  return {
    protocol: 'sugarcubes.host',
    version: 1,
    session,
    type: 'description',
    protocols: [{ name: 'sugarcubes.panel', version: 3 }],
    capabilities: capabilities.map((name) => ({ name })),
  };
}

afterEach(() => {
  window.history.replaceState(null, '', '/');
  Reflect.deleteProperty(window, 'sugarcubesPanelHost');
  Reflect.deleteProperty(window, 'uxpHost');
  jest.useRealTimers();
});

test.each([
  'sc-panel-host=x&sc-panel-host=y&sc-panel-session=fixture-session-0001',
  'sc-panel-host=x%0A&sc-panel-session=fixture-session-0001',
  'sc-panel-host=x&sc-panel-session=fixture-session-0001%0A',
])('rejects duplicate and partially matched navigation identities: %s', (search) => {
  expect(panelNavigation(search)).toBeNull();
});

test('requires the native injected identity and never falls back to an unrelated UXP bridge', () => {
  window.history.replaceState(
    null,
    '',
    `/?sc-panel-host=${hostId}&sc-panel-session=${session}&sc-panel-transport=native`,
  );
  const send = jest.fn();
  Reflect.set(window, 'uxpHost', { postMessage: send });
  Reflect.set(window, 'sugarcubesPanelHost', {
    host: hostId,
    session: 'stale-session-0001',
    send,
    subscribe: jest.fn(),
  });
  expect(
    startEmbeddedPanel({
      window,
      host: new HostDescriptionState(),
      presentation: new EmbeddedPresentation(),
      connected: () => true,
      report: jest.fn(),
    }),
  ).toBeNull();
  expect(send).not.toHaveBeenCalled();
});

test('retains the injected transport after Comfy changes its route', () => {
  window.history.replaceState(
    null,
    '',
    `/?sc-panel-host=${hostId}&sc-panel-session=${session}&sc-panel-transport=native`,
  );
  Reflect.set(window, 'sugarcubesPanelHost', {
    host: hostId,
    session,
    send: jest.fn(),
    subscribe: jest.fn(),
  });
  expect(createNativePanelTransport(window)).not.toBeNull();
  window.history.replaceState(null, '', '/#workflow');
  expect(createNativePanelTransport(window)?.session).toBe(session);
});

test('starts the native panel after a route change using the injected navigation identity', () => {
  window.history.replaceState(null, '', '/#workflow');
  const send = jest.fn();
  Reflect.set(window, 'sugarcubesPanelHost', {
    host: hostId,
    session,
    send,
    subscribe: () => () => undefined,
  });
  const panel = startEmbeddedPanel({
    window,
    host: new HostDescriptionState(),
    presentation: new EmbeddedPresentation(),
    connected: () => true,
    report: jest.fn(),
  });
  expect(panel).not.toBeNull();
  expect(send).toHaveBeenCalled();
  panel?.dispose();
});

test('a bridge without an identity and message port cannot create a native transport', () => {
  window.history.replaceState(null, '', '/#workflow');
  Reflect.set(window, 'sugarcubesPanelHost', {});
  expect(createNativePanelTransport(window)).toBeNull();
});

test('rejects partial native navigation identities even with an injected bridge', () => {
  window.history.replaceState(null, '', '/?sc-panel-transport=native');
  Reflect.set(window, 'sugarcubesPanelHost', {
    host: hostId,
    session,
    send: jest.fn(),
    subscribe: jest.fn(),
  });
  expect(createNativePanelTransport(window)).toBeNull();
});

test('a host that does not declare host styles keeps live saved preferences through the handshake', () => {
  jest.useFakeTimers();
  window.history.replaceState(
    null,
    '',
    `/?sc-panel-host=${hostId}&sc-panel-session=${session}&sc-panel-transport=native`,
  );
  let receive: ((value: unknown) => void) | undefined;
  const send = jest.fn();
  Reflect.set(window, 'sugarcubesPanelHost', {
    host: hostId,
    session,
    send,
    subscribe: (listener: (value: unknown) => void) => {
      receive = listener;
      return () => undefined;
    },
  });
  const changes = new Map<string, (value: unknown) => void>();
  const writes = jest.fn();
  const app = {
    ui: {
      settings: {
        getSettingValue: (id: string): unknown =>
          id === VISUAL_STYLE_SETTING ? 'fluent' : id === PLUGIN_MODE_SETTING,
        addSetting: (value: { id: string; onChange(value: unknown): void }): void => {
          changes.set(value.id, value.onChange);
        },
        setSettingValue: writes,
      },
    },
  };
  const presentation = new EmbeddedPresentation();
  const style = jest.fn();
  const mode = jest.fn();
  const report = jest.fn();
  const stopStyle = registerVisualStylePreference(app, style, report, presentation);
  const stopMode = registerPluginModePreference(app, document, mode, report, presentation);
  const owner = startEmbeddedPanel({
    window,
    host: new HostDescriptionState(),
    presentation,
    connected: () => true,
    report,
  });
  receive?.(description([]));
  receive?.({
    protocol: 'sugarcubes.panel',
    version: 3,
    session,
    type: 'configure',
    presentation: {
      pluginMode: false,
      visualStyle: 'comfy',
      photoProPalette: 'dark-gray',
      accentColor: null,
      resizeCornerSize: 0,
    },
  });
  expect(send).toHaveBeenLastCalledWith({
    protocol: 'sugarcubes.panel',
    version: 3,
    session,
    type: 'applied',
  });
  expect(presentation.read()).toBeNull();
  expect(style).toHaveBeenLastCalledWith('fluent');
  expect(mode).toHaveBeenLastCalledWith(true);
  changes.get(VISUAL_STYLE_SETTING)?.('spectrum');
  changes.get(PLUGIN_MODE_SETTING)?.(false);
  expect(style).toHaveBeenLastCalledWith('spectrum');
  expect(mode).toHaveBeenLastCalledWith(false);
  expect(writes).not.toHaveBeenCalled();
  expect(report).not.toHaveBeenCalled();
  owner?.dispose();
  stopStyle();
  stopMode();
});

test('negotiates GIMP presentation over native transport and releases its subscriptions', () => {
  jest.useFakeTimers();
  window.history.replaceState(
    null,
    '',
    `/?sc-panel-host=${hostId}&sc-panel-session=${session}&sc-panel-transport=native`,
  );
  let receive: ((value: unknown) => void) | undefined;
  const stop = jest.fn();
  const send = jest.fn();
  Reflect.set(window, 'sugarcubesPanelHost', {
    host: hostId,
    session,
    send,
    subscribe: (listener: (value: unknown) => void) => {
      receive = listener;
      return stop;
    },
  });
  const presentation = new EmbeddedPresentation();
  const owner = startEmbeddedPanel({
    window,
    host: new HostDescriptionState(),
    presentation,
    connected: () => false,
    report: jest.fn(),
  });
  expect(send).toHaveBeenLastCalledWith({
    protocol: 'sugarcubes.panel',
    version: 3,
    session,
    type: 'ready',
  });
  receive?.(description(['presentation.host-styles']));
  receive?.({
    protocol: 'sugarcubes.panel',
    version: 3,
    session,
    type: 'configure',
    presentation: {
      pluginMode: true,
      visualStyle: 'gimp',
      photoProPalette: 'dark-gray',
      accentColor: null,
      resizeCornerSize: 0,
    },
  });
  expect(presentation.read()?.visualStyle).toBe('gimp');
  receive?.({ protocol: 'sugarcubes.panel', version: 3, session, type: 'ping' });
  expect(send).toHaveBeenLastCalledWith({
    protocol: 'sugarcubes.panel',
    version: 3,
    session,
    type: 'pong',
    connected: false,
  });
  owner?.dispose();
  expect(presentation.read()).toBeNull();
  expect(stop).toHaveBeenCalledTimes(1);
  expect(jest.getTimerCount()).toBe(0);
});

test('hosts without host styles still offer their accent as the default accent', () => {
  jest.useFakeTimers();
  window.history.replaceState(
    null,
    '',
    `/?sc-panel-host=${hostId}&sc-panel-session=${session}&sc-panel-transport=native`,
  );
  let receive: ((value: unknown) => void) | undefined;
  Reflect.set(window, 'sugarcubesPanelHost', {
    host: hostId,
    session,
    send: jest.fn(),
    subscribe: (listener: (value: unknown) => void) => {
      receive = listener;
      return () => undefined;
    },
  });
  const presentation = new EmbeddedPresentation();
  const hostAccent = new HostAccent();
  const owner = startEmbeddedPanel({
    window,
    host: new HostDescriptionState(),
    presentation,
    hostAccent,
    connected: () => true,
    report: jest.fn(),
  });
  const configure = (accentColor: string | null): void =>
    receive?.({
      protocol: 'sugarcubes.panel',
      version: 3,
      session,
      type: 'configure',
      presentation: {
        pluginMode: false,
        visualStyle: 'comfy',
        photoProPalette: 'dark-gray',
        accentColor,
        resizeCornerSize: 0,
      },
    });
  configure('#0078d4');
  expect(presentation.read()).toBeNull();
  expect(hostAccent.read()).toBe('#0078d4');
  configure('#107c10');
  expect(hostAccent.read()).toBe('#107c10');
  configure(null);
  expect(hostAccent.read()).toBeNull();
  configure('#0078d4');
  owner?.dispose();
  expect(hostAccent.read()).toBeNull();
});

/** A bridge that queues at most eight canvas requests and refuses the rest as the native host does. */
function boundedBridge() {
  const queued: { requestId: number; type: string }[] = [];
  const delivered: string[] = [];
  const listeners = new Set<(value: unknown) => void>();
  Reflect.set(window, 'sugarcubesPanelHost', {
    host: hostId,
    session,
    send: (message: { protocol: string; requestId?: number; type: string }) => {
      if (message.protocol === 'sugarcubes.canvas' && message.type !== 'cancel') {
        if (queued.length >= 8)
          throw new TypeError('', { cause: { code: 'panel.transport.capacity' } });
        queued.push({ requestId: message.requestId ?? 0, type: message.type });
      }
      delivered.push(`${message.type}:${message.requestId ?? '-'}`);
    },
    subscribe: (listener: (value: unknown) => void) => {
      listeners.add(listener);
      return () => listeners.delete(listener);
    },
  });
  return {
    delivered,
    queued,
    /** The host finishes its oldest request: capacity frees and its reply reaches the page. */
    reply(): void {
      const done = queued.shift();
      for (const listener of listeners) listener({ type: 'reply', requestId: done?.requestId });
    },
    /** The host frees capacity with no message for the page. */
    free(): void {
      queued.shift();
    },
  };
}

/** Compose a canvas protocol message the bridge admits on its request lane. */
function canvasRequest(requestId: number, type = 'surface.geometry') {
  return { protocol: 'sugarcubes.canvas', version: 3, session, requestId, type } as const;
}

test('canvas requests beyond the bridge window are held and delivered in order as the host replies', () => {
  window.history.replaceState(
    null,
    '',
    `/?sc-panel-host=${hostId}&sc-panel-session=${session}&sc-panel-transport=native`,
  );
  const bridge = boundedBridge();
  const transport = createNativePanelTransport(window);
  if (!transport) throw new Error('Missing native transport.');
  const replies: unknown[] = [];
  transport.subscribe((value) => replies.push(value));
  for (let id = 1; id <= 12; id++)
    expect(() => transport.send(canvasRequest(id) as never)).not.toThrow();
  expect(bridge.delivered).toEqual(
    Array.from({ length: 8 }, (_, index) => `surface.geometry:${index + 1}`),
  );
  for (let id = 9; id <= 12; id++) bridge.reply();
  expect(bridge.delivered.slice(8)).toEqual(
    Array.from({ length: 4 }, (_, index) => `surface.geometry:${index + 9}`),
  );
  expect(replies).toHaveLength(4);
});

test('a cancellation never overtakes the held request it cancels, and control messages are not delayed', () => {
  window.history.replaceState(
    null,
    '',
    `/?sc-panel-host=${hostId}&sc-panel-session=${session}&sc-panel-transport=native`,
  );
  const bridge = boundedBridge();
  const transport = createNativePanelTransport(window);
  if (!transport) throw new Error('Missing native transport.');
  transport.subscribe(() => undefined);
  for (let id = 1; id <= 9; id++) transport.send(canvasRequest(id) as never);
  transport.send(canvasRequest(9, 'cancel') as never);
  transport.send({ protocol: 'sugarcubes.panel', version: 3, session, type: 'pong' } as never);
  expect(bridge.delivered.at(-1)).toBe('pong:-');
  expect(bridge.delivered).not.toContain('cancel:9');
  bridge.reply();
  expect(bridge.delivered.slice(-2)).toEqual(['surface.geometry:9', 'cancel:9']);
});

test('held requests retry on their own when the host frees capacity without a message for the page', () => {
  jest.useFakeTimers();
  window.history.replaceState(
    null,
    '',
    `/?sc-panel-host=${hostId}&sc-panel-session=${session}&sc-panel-transport=native`,
  );
  const bridge = boundedBridge();
  const transport = createNativePanelTransport(window);
  if (!transport) throw new Error('Missing native transport.');
  for (let id = 1; id <= 9; id++) transport.send(canvasRequest(id) as never);
  expect(bridge.delivered).toHaveLength(8);
  bridge.free();
  jest.advanceTimersByTime(25);
  expect(bridge.delivered.at(-1)).toBe('surface.geometry:9');
});

test('a bridge failure other than capacity is raised, never swallowed or retried', () => {
  window.history.replaceState(
    null,
    '',
    `/?sc-panel-host=${hostId}&sc-panel-session=${session}&sc-panel-transport=native`,
  );
  let closed = false;
  Reflect.set(window, 'sugarcubesPanelHost', {
    host: hostId,
    session,
    send: () => {
      if (closed) throw new TypeError('', { cause: { code: 'panel.transport.closed' } });
    },
    subscribe: () => () => undefined,
  });
  const transport = createNativePanelTransport(window);
  if (!transport) throw new Error('Missing native transport.');
  transport.send(canvasRequest(1) as never);
  closed = true;
  expect(() => transport.send(canvasRequest(2) as never)).toThrow(TypeError);
});
