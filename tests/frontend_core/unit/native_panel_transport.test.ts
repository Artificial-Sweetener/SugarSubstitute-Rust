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
import { panelNavigation } from '../../../frontend/comfyui/ui/core/embedded/BrowserPanelTransport.js';

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
