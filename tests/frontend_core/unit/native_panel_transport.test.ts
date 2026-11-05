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
import { panelNavigation } from '../../../frontend/comfyui/ui/core/embedded/BrowserPanelTransport.js';
import {
  createNativePanelTransport,
  prefersNativeOutputSurface,
} from '../../../frontend/comfyui/ui/core/embedded/NativePanelTransport.js';
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
      presentation: new EmbeddedPresentation(),
      connected: () => true,
      report: jest.fn(),
    }),
  ).toBeNull();
  expect(send).not.toHaveBeenCalled();
});

test('rejects an unrecognized native preference policy', () => {
  window.history.replaceState(
    null,
    '',
    `/?sc-panel-host=${hostId}&sc-panel-session=${session}&sc-panel-transport=native`,
  );
  Reflect.set(window, 'sugarcubesPanelHost', {
    host: hostId,
    session,
    preferencePolicy: 'unexpected',
    send: jest.fn(),
    subscribe: jest.fn(),
  });
  expect(
    startEmbeddedPanel({
      window,
      presentation: new EmbeddedPresentation(),
      connected: () => true,
      report: jest.fn(),
    }),
  ).toBeNull();
});

test('reads the optional host canvas preference before capability negotiation', () => {
  window.history.replaceState(
    null,
    '',
    `/?sc-panel-host=${hostId}&sc-panel-session=${session}&sc-panel-transport=native`,
  );
  for (const value of [undefined, 'host'] as const) {
    Reflect.set(window, 'sugarcubesPanelHost', {
      host: hostId,
      session,
      outputSurfacePreference: value,
      send: jest.fn(),
      subscribe: jest.fn(),
    });
    expect(createNativePanelTransport(window)?.outputSurfacePreference).toBe(value);
  }
});

test('rejects an invalid host canvas preference', () => {
  window.history.replaceState(
    null,
    '',
    `/?sc-panel-host=${hostId}&sc-panel-session=${session}&sc-panel-transport=native`,
  );
  Reflect.set(window, 'sugarcubesPanelHost', {
    host: hostId,
    session,
    outputSurfacePreference: 'invalid',
    send: jest.fn(),
    subscribe: jest.fn(),
  });
  expect(createNativePanelTransport(window)).toBeNull();
});

test('retains the presentation preference and injected transport after Comfy changes its route', () => {
  window.history.replaceState(
    null,
    '',
    `/?sc-panel-host=${hostId}&sc-panel-session=${session}&sc-panel-transport=native`,
  );
  Reflect.set(window, 'sugarcubesPanelHost', {
    host: hostId,
    session,
    outputSurfacePreference: 'host',
    send: jest.fn(),
    subscribe: jest.fn(),
  });
  expect(prefersNativeOutputSurface(window)).toBe(true);
  expect(createNativePanelTransport(window)).not.toBeNull();
  window.history.replaceState(null, '', '/#workflow');
  expect(createNativePanelTransport(window)?.session).toBe(session);
  expect(prefersNativeOutputSurface(window)).toBe(true);
});

test('uses browser output when no host presentation preference exists', () => {
  expect(prefersNativeOutputSurface(window)).toBe(false);
  Reflect.set(window, 'sugarcubesPanelHost', { outputSurfacePreference: 'invalid' });
  expect(prefersNativeOutputSurface(window)).toBe(false);
  Reflect.set(window, 'sugarcubesPanelHost', {});
  expect(prefersNativeOutputSurface(window)).toBe(false);
});

test('starts the native panel after a route change using the injected navigation identity', () => {
  window.history.replaceState(null, '', '/#workflow');
  const send = jest.fn();
  Reflect.set(window, 'sugarcubesPanelHost', {
    host: hostId,
    session,
    outputSurfacePreference: 'host',
    send,
    subscribe: () => () => undefined,
  });
  const panel = startEmbeddedPanel({
    window,
    presentation: new EmbeddedPresentation(),
    connected: () => true,
    report: jest.fn(),
  });
  expect(panel).not.toBeNull();
  expect(send).toHaveBeenCalled();
  panel?.dispose();
});

test('a presentation preference alone cannot create a native transport', () => {
  window.history.replaceState(null, '', '/#workflow');
  Reflect.set(window, 'sugarcubesPanelHost', { outputSurfacePreference: 'host' });
  expect(prefersNativeOutputSurface(window)).toBe(true);
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

test('native hosts can retain live saved preferences while completing the panel handshake', () => {
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
    preferencePolicy: 'saved',
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
  const owner = startEmbeddedPanel({ window, presentation, connected: () => true, report });
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

test('saved-preference hosts still offer their system accent as the default accent', () => {
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
    preferencePolicy: 'saved',
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
