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
/** Prove identified WebViews negotiate presentation while ordinary browser sessions remain untouched. */
import { EMPTY_CANVAS_SURFACE_CAPABILITIES } from '../../../integrations/shared/CanvasSurfaceDiscovery.js';
import { EmbeddedCanvasConnection } from '../../../frontend/comfyui/ui/core/embedded/EmbeddedCanvasConnection.js';
import { selectedPngCapabilities } from '../../editor_integrations/support/canvas_surface.js';
import { afterEach, expect, jest, test } from '@jest/globals';
import { EmbeddedPresentation } from '../../../frontend/comfyui/ui/core/embedded/EmbeddedPresentation.js';
import { EmbeddedPanelLayout } from '../../../frontend/comfyui/ui/core/embedded/EmbeddedPanelLayout.js';

test('publishes native resize clearance once per change and restores the exact local value', () => {
  const style = document.documentElement.style;
  style.setProperty('--sc-panel-resize-corner', '7px', 'important');
  const geometry = jest.fn();
  window.addEventListener('resize', geometry);
  const layout = new EmbeddedPanelLayout(window);
  try {
    layout.apply(24);
    layout.apply(24);
    expect(style.getPropertyValue('--sc-panel-resize-corner')).toBe('24px');
    expect(geometry).toHaveBeenCalledTimes(1);
    layout.apply(0);
    expect(style.getPropertyValue('--sc-panel-resize-corner')).toBe('0px');
    expect(geometry).toHaveBeenCalledTimes(2);
    layout.dispose();
    expect(style.getPropertyValue('--sc-panel-resize-corner')).toBe('7px');
    expect(style.getPropertyPriority('--sc-panel-resize-corner')).toBe('important');
    expect(geometry).toHaveBeenCalledTimes(3);
    layout.dispose();
    expect(geometry).toHaveBeenCalledTimes(3);
  } finally {
    layout.dispose();
    window.removeEventListener('resize', geometry);
    style.removeProperty('--sc-panel-resize-corner');
  }
});
import {
  EmbeddedPanelHost,
  startEmbeddedPanel,
} from '../../../frontend/comfyui/ui/core/embedded/EmbeddedPanelHost.js';
import { createUxpPanelTransport } from '../../../frontend/comfyui/ui/core/embedded/UxpPanelTransport.js';
import type { PanelMessage } from '../../../frontend/comfyui/ui/core/embedded/PanelRuntime.js';
import type { CanvasMessage } from '../../../frontend/comfyui/ui/core/embedded/CanvasRuntime.js';

const session = '00000000-0000-4000-8000-000000000001';
const hostId = 'fixture.editor.panel';

afterEach(() => {
  window.history.replaceState(null, '', '/');
  Reflect.deleteProperty(window, 'uxpHost');
  jest.useRealTimers();
});

test('query parameters alone never apply embedded presentation in an ordinary browser', () => {
  jest.useFakeTimers();
  window.history.replaceState(null, '', `/?sc-panel-host=${hostId}&sc-panel-session=${session}`);
  const presentation = new EmbeddedPresentation();
  const host = startEmbeddedPanel({
    window,
    presentation,
    connected: () => true,
    report: jest.fn(),
  });
  jest.advanceTimersByTime(5000);
  expect(presentation.read()).toBeNull();
  expect(document.documentElement.style.getPropertyValue('--sc-panel-resize-corner')).toBe('');
  host?.dispose();
});

test('negotiates after a delayed or replaced UXP bridge while preserving source validation', () => {
  jest.useFakeTimers();
  window.history.replaceState(null, '', `/?sc-panel-host=${hostId}&sc-panel-session=${session}`);
  const presentation = new EmbeddedPresentation();
  const host = startEmbeddedPanel({
    window,
    presentation,
    connected: () => true,
    report: jest.fn(),
  });
  const stale = { postMessage: jest.fn() };
  Object.defineProperty(window, 'uxpHost', { value: stale, configurable: true });
  jest.advanceTimersByTime(1000);
  expect(stale.postMessage).toHaveBeenCalledWith(expect.objectContaining({ type: 'ready' }));
  const current = { postMessage: jest.fn() };
  Object.defineProperty(window, 'uxpHost', { value: current, configurable: true });
  jest.advanceTimersByTime(1000);
  expect(current.postMessage).toHaveBeenCalledWith(expect.objectContaining({ type: 'ready' }));
  const configure = (source: unknown): void => {
    const event = new MessageEvent('message', {
      origin: hostId,
      data: {
        protocol: 'sugarcubes.panel',
        version: 3,
        session,
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
    Object.defineProperty(event, 'source', { value: source });
    window.dispatchEvent(event);
  };
  configure(stale);
  expect(presentation.read()).toBeNull();
  configure(current);
  expect(presentation.read()?.accentColor).toBe('#d6d6d6');
  host?.dispose();
});

test('filters plugin origin and source before delivering unknown payloads', () => {
  const bridge = { postMessage: jest.fn() };
  Object.defineProperty(window, 'uxpHost', { value: bridge, configurable: true });
  window.history.replaceState(null, '', `/?sc-panel-host=${hostId}&sc-panel-session=${session}`);
  const transport = createUxpPanelTransport(window);
  expect(transport).not.toBeNull();
  const receive = jest.fn();
  const stop = transport?.subscribe(receive);
  const dispatch = (origin: string, source: unknown): void => {
    const event = new MessageEvent<unknown>('message', {
      origin,
      data: { value: '<img src=x onerror=alert(1)>' },
    });
    Object.defineProperty(event, 'source', { value: source });
    window.dispatchEvent(event);
  };
  dispatch('untrusted.plugin', bridge);
  dispatch(hostId, window);
  expect(receive).not.toHaveBeenCalled();
  dispatch(hostId, bridge);
  expect(receive).toHaveBeenCalledTimes(1);
  transport?.send({ protocol: 'sugarcubes.panel', version: 3, session, type: 'ready' });
  expect(bridge.postMessage).toHaveBeenCalledTimes(1);
  stop?.();
  dispatch(hostId, bridge);
  expect(receive).toHaveBeenCalledTimes(1);
});

test('retries readiness, acknowledges applied presentation, reports connectivity and releases its lifetime', () => {
  jest.useFakeTimers();
  const presentation = new EmbeddedPresentation();
  const messages: (PanelMessage | CanvasMessage)[] = [];
  let receive: ((value: unknown) => void) | null = null;
  const stop = jest.fn();
  const host = new EmbeddedPanelHost(
    { window, presentation, connected: () => false, report: jest.fn() },
    {
      session,
      send: (value) => {
        if (value.protocol === 'sugarcubes.canvas.masks' || value.protocol === 'sugarcubes.memory')
          throw new Error('Unexpected mask or memory operation during presentation negotiation.');
        messages.push(value);
      },
      subscribe: (listener) => {
        receive = listener;
        return stop;
      },
    },
  );
  host.setup();
  host.setup();
  jest.advanceTimersByTime(1000);
  expect(messages.map((value) => value.type)).toEqual(['ready', 'ready']);
  const dispatch = (value: unknown): void => {
    receive?.(value);
  };
  dispatch({
    protocol: 'sugarcubes.panel',
    version: 3,
    session: 'stale-session-000000',
    type: 'configure',
    presentation: {
      pluginMode: true,
      visualStyle: 'photopro',
      photoProPalette: 'medium-gray',
      accentColor: null,
      resizeCornerSize: 24,
    },
  });
  expect(presentation.read()).toBeNull();
  dispatch({
    protocol: 'sugarcubes.panel',
    version: 3,
    session,
    type: 'configure',
    presentation: {
      pluginMode: true,
      visualStyle: 'photopro',
      photoProPalette: 'medium-gray',
      accentColor: null,
      resizeCornerSize: 24,
    },
  });
  expect(presentation.read()).toMatchObject({ pluginMode: true, visualStyle: 'photopro' });
  expect(document.documentElement.style.getPropertyValue('--sc-panel-resize-corner')).toBe('24px');
  expect(messages.at(-1)?.type).toBe('applied');
  jest.advanceTimersByTime(1000);
  expect(messages.at(-1)?.type).toBe('applied');
  dispatch({ protocol: 'sugarcubes.panel', version: 3, session, type: 'ping' });
  expect(messages.at(-1)).toMatchObject({ type: 'pong', connected: false });
  window.dispatchEvent(new Event('pagehide'));
  expect(presentation.read()).toBeNull();
  expect(document.documentElement.style.getPropertyValue('--sc-panel-resize-corner')).toBe('');
  expect(stop).toHaveBeenCalledTimes(1);
  expect(jest.getTimerCount()).toBe(0);
});

test('discovers a native canvas after configuration and releases correlated requests on teardown', async () => {
  const { EmbeddedCanvasConnection } = await import(
    '../../../frontend/comfyui/ui/core/embedded/EmbeddedCanvasConnection.js'
  );
  const listeners = new Set<(value: unknown) => void>();
  const sent: (PanelMessage | CanvasMessage)[] = [];
  const canvas = new EmbeddedCanvasConnection();
  const transport = {
    session,
    send: (message: PanelMessage | CanvasMessage) => {
      sent.push(message);
      if (message.protocol === 'sugarcubes.canvas' && message.type === 'context.request')
        for (const receive of listeners)
          receive({
            ...message,
            type: 'context.reply',
            context: {
              canvas: {
                id: 'fixture',
                label: 'Fixture editor',
                capabilities: {
                  displayOutput: false,
                  insertImageLayer: true,
                  positionImageLayer: false,
                },
              },
              document: null,
            },
          });
    },
    subscribe: (receive: (value: unknown) => void) => {
      listeners.add(receive);
      return () => {
        listeners.delete(receive);
      };
    },
  };
  expect(canvas.readCanvas()).toBeNull();
  canvas.connect(transport, window, (error) => {
    throw error;
  });
  await Promise.resolve();
  await Promise.resolve();
  expect(canvas.readCanvas()?.label).toBe('Fixture editor');
  canvas.connect(transport, window, (error) => {
    throw error;
  });
  expect(sent.filter((message) => message.type === 'context.request')).toHaveLength(1);
  canvas.dispose();
  expect(listeners.size).toBe(0);
  expect(canvas.readCanvas()).toBeNull();
});

/** Capability discovery completes before observers can mount a usable native surface. */
test('publishes canvas readiness only after delayed surface discovery completes', async () => {
  const { EmbeddedCanvasConnection } = await import(
    '../../../frontend/comfyui/ui/core/embedded/EmbeddedCanvasConnection.js'
  );
  const listeners = new Set<(value: unknown) => void>();
  const sent: CanvasMessage[] = [];
  const canvas = new EmbeddedCanvasConnection();
  const ready = jest.fn();
  document.addEventListener('sugarcubes-canvas-ready', ready);
  try {
    canvas.connect(
      {
        session,
        send: (message) => {
          if (message.protocol !== 'sugarcubes.canvas') throw new Error('Unexpected protocol.');
          sent.push(message);
          if (message.type === 'context.request')
            for (const receive of listeners)
              receive({
                ...message,
                type: 'context.reply',
                context: {
                  canvas: {
                    id: 'native',
                    label: 'Native',
                    capabilities: {
                      displayOutput: true,
                      insertImageLayer: false,
                      positionImageLayer: false,
                    },
                  },
                  document: null,
                },
              });
        },
        subscribe: (receive) => {
          listeners.add(receive);
          return () => {
            listeners.delete(receive);
          };
        },
      },
      window,
      (error) => {
        throw error;
      },
    );
    await Promise.resolve();
    await Promise.resolve();
    expect(sent.map(({ type }) => type)).toEqual([
      'context.request',
      'surface.capabilities.request',
    ]);
    expect(canvas.surface).toBeNull();
    expect(ready).not.toHaveBeenCalled();
    const request = sent[1];
    if (request?.type !== 'surface.capabilities.request')
      throw new Error('Missing discovery request.');
    for (const receive of listeners)
      receive({
        protocol: request.protocol,
        version: request.version,
        session: request.session,
        requestId: request.requestId,
        type: 'surface.capabilities.reply',
        status: 'available',
        capabilities: selectedPngCapabilities,
        message: '',
      });
    for (let turn = 0; turn < 8; turn++) await Promise.resolve();
    expect(canvas.surface?.capabilities.imageFormats).toEqual(['png']);
    expect(ready).toHaveBeenCalledTimes(1);
  } finally {
    document.removeEventListener('sugarcubes-canvas-ready', ready);
    canvas.dispose();
  }
});

/** Collect real client requests while controlling only the host transport. */
function discoveryFixture() {
  const listeners = new Set<(value: unknown) => void>();
  const messages: CanvasMessage[] = [];
  const canvas = new EmbeddedCanvasConnection();
  const report = jest.fn();
  const transport = {
    session,
    send: (message: PanelMessage | CanvasMessage) => {
      if (message.protocol === 'sugarcubes.canvas') messages.push(message);
    },
    subscribe: (receive: (value: unknown) => void) => {
      listeners.add(receive);
      return () => {
        listeners.delete(receive);
      };
    },
  };
  const receive = (value: unknown): void => {
    for (const listener of listeners) listener(value);
  };
  const context = (index: number): void =>
    receive({
      ...messages[index],
      type: 'context.reply',
      context: {
        canvas: {
          id: 'native',
          label: 'Native',
          capabilities: { displayOutput: true, insertImageLayer: false, positionImageLayer: false },
        },
        document: null,
      },
    });
  const capabilities = (index: number, available: boolean): void => {
    const request = messages[index];
    if (request?.type !== 'surface.capabilities.request')
      throw new Error('Missing discovery request.');
    receive({
      protocol: request.protocol,
      version: request.version,
      session: request.session,
      requestId: request.requestId,
      type: 'surface.capabilities.reply',
      status: available ? 'available' : 'unavailable',
      capabilities: available ? selectedPngCapabilities : EMPTY_CANVAS_SURFACE_CAPABILITIES,
      message: '',
    });
  };
  return { canvas, report, messages, transport, receive, context, capabilities, listeners };
}

/** A later public read owns the single readiness publication for its completed negotiation. */
test('a new discovery withdraws the previous completion until its own capabilities arrive', async () => {
  const f = discoveryFixture();
  try {
    f.canvas.connect(f.transport, window, f.report);
    f.context(0);
    await Promise.resolve();
    f.capabilities(1, true);
    for (let turn = 0; turn < 8; turn++) await Promise.resolve();
    expect(f.canvas.surfaceDiscovered).toBe(true);
    const current = f.canvas.readContext();
    void current.catch(() => {});
    expect(f.canvas.surface).toBeNull();
    expect(f.canvas.surfaceDiscovered).toBe(false);
    f.context(2);
    await Promise.resolve();
    expect(f.canvas.surfaceDiscovered).toBe(false);
    f.capabilities(3, true);
    await current;
    expect(f.canvas.surfaceDiscovered).toBe(true);
    expect(f.canvas.surface).not.toBeNull();
    expect(f.report).not.toHaveBeenCalled();
  } finally {
    f.canvas.dispose();
  }
});

test('backend disconnect revokes an accepted surface and a pending refresh cannot restore it', async () => {
  const f = discoveryFixture();
  try {
    f.canvas.connect(f.transport, window, f.report);
    f.context(0);
    await Promise.resolve();
    f.capabilities(1, true);
    for (let turn = 0; turn < 8; turn++) await Promise.resolve();
    const previous = f.canvas.surface;
    expect(previous).not.toBeNull();
    f.canvas.refresh();
    f.context(2);
    await Promise.resolve();
    expect(f.canvas.surface).toBe(previous);
    f.canvas.disconnected();
    expect(f.canvas.surface).toBeNull();
    expect(f.canvas.surfaceDiscovered).toBe(false);
    f.capabilities(3, true);
    for (let turn = 0; turn < 8; turn++) await Promise.resolve();
    expect(f.canvas.surface).toBeNull();
    expect(f.canvas.surfaceDiscovered).toBe(false);
  } finally {
    f.canvas.dispose();
  }
});

/** A later public read owns the single readiness publication for its completed negotiation. */
test('overlapping context reads cannot announce readiness before the current discovery completes', async () => {
  const f = discoveryFixture();
  const ready = jest.fn();
  document.addEventListener('sugarcubes-canvas-ready', ready);
  try {
    f.canvas.connect(f.transport, window, f.report);
    const second = f.canvas.readContext();
    f.context(0);
    for (let turn = 0; turn < 8; turn++) await Promise.resolve();
    expect(ready).not.toHaveBeenCalled();
    expect(f.messages.map(({ type }) => type)).toEqual(['context.request', 'context.request']);
    f.context(1);
    await Promise.resolve();
    expect(ready).not.toHaveBeenCalled();
    f.capabilities(2, true);
    await second;
    expect(f.canvas.surface?.capabilities.imageFormats).toEqual(['png']);
    expect(ready).toHaveBeenCalledTimes(1);
    expect(f.report).not.toHaveBeenCalled();
  } finally {
    document.removeEventListener('sugarcubes-canvas-ready', ready);
    f.canvas.dispose();
  }
});

/** Reuse actual Comfy connectivity transitions and the existing session instead of polling discovery. */
test('renegotiates once after reconnect during an unavailable discovery and releases the connection subscription', async () => {
  jest.useFakeTimers();
  const f = discoveryFixture();
  let connection: ((connected: boolean) => void) | null = null;
  const stopConnection = jest.fn();
  const host = new EmbeddedPanelHost(
    {
      window,
      presentation: new EmbeddedPresentation(),
      canvas: f.canvas,
      connected: () => true,
      report: f.report,
      subscribeConnection: (listener) => {
        connection = listener;
        return stopConnection;
      },
    },
    f.transport,
  );
  host.setup();
  f.receive({
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
  f.context(0);
  await Promise.resolve();
  const reconnect = (connected: boolean): void => {
    connection?.(connected);
  };
  reconnect(false);
  reconnect(true);
  reconnect(true);
  expect(f.messages.map(({ type }) => type)).toEqual([
    'context.request',
    'surface.capabilities.request',
    'context.request',
  ]);
  f.capabilities(1, false);
  for (let turn = 0; turn < 12; turn++) await Promise.resolve();
  expect(f.canvas.surface).toBeNull();
  expect(f.messages.map(({ type }) => type)).toEqual([
    'context.request',
    'surface.capabilities.request',
    'context.request',
  ]);
  f.context(2);
  await Promise.resolve();
  f.capabilities(3, true);
  for (let turn = 0; turn < 12; turn++) await Promise.resolve();
  expect(f.canvas.surface?.capabilities.imageFormats).toEqual(['png']);
  expect(f.report).not.toHaveBeenCalled();
  host.dispose();
  expect(stopConnection).toHaveBeenCalledTimes(1);
  expect(f.listeners.size).toBe(0);
  expect(jest.getTimerCount()).toBe(0);
  reconnect(true);
  expect(f.messages).toHaveLength(4);
});

/** A live navigation retains its only request sequence across capability refreshes. */
test('refreshes reuse the connected client and keep request identities increasing', async () => {
  const f = discoveryFixture();
  f.canvas.connect(f.transport, window, f.report);
  for (const start of [0, 2, 4]) {
    if (start > 0) f.canvas.refresh();
    f.context(start);
    await Promise.resolve();
    f.capabilities(start + 1, true);
    for (let turn = 0; turn < 8; turn++) await Promise.resolve();
    expect(f.canvas.surface?.capabilities.imageFormats).toEqual(['png']);
  }
  f.canvas.connect(f.transport, window, f.report);
  expect(f.messages.map(({ requestId }) => requestId)).toEqual([1, 2, 3, 4, 5, 6]);
  expect(f.listeners.size).toBe(1);
  expect(f.report).not.toHaveBeenCalled();
  f.canvas.dispose();
});

/** Page teardown must not create a second client under the same public panel session. */
test('a disposed embedded canvas refuses reconnection and cannot restart request identities', async () => {
  const f = discoveryFixture();
  f.canvas.connect(f.transport, window, f.report);
  f.canvas.dispose();
  f.canvas.dispose();
  expect(() => f.canvas.connect(f.transport, window, f.report)).toThrow('connection is closed');
  f.canvas.refresh();
  await expect(f.canvas.readContext()).rejects.toThrow('unavailable');
  expect(f.messages.map(({ type, requestId }) => ({ type, requestId }))).toEqual([
    { type: 'context.request', requestId: 1 },
    { type: 'cancel', requestId: 1 },
  ]);
  expect(f.listeners.size).toBe(0);
  expect(f.report).not.toHaveBeenCalled();
});
