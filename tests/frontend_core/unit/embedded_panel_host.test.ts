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
        if (value.protocol === 'sugarcubes.canvas.masks')
          throw new Error('Unexpected mask operation during presentation negotiation.');
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
                capabilities: { displayOutput: false, insertImageLayer: true },
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
