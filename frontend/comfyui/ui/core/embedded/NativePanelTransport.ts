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
/** Adapt a native viewer's injected transport without importing its runtime or editor SDK. */
import { isRecord } from '../../types/common.js';
import { panelNavigation, type BrowserPanelTransport } from './BrowserPanelTransport.js';

/** Read only the host's presentation preference, which grants no canvas or transport authority. */
export function prefersNativeOutputSurface(windowRef: Window): boolean {
  const bridge: unknown = Reflect.get(windowRef, 'sugarcubesPanelHost');
  return isRecord(bridge) && bridge.outputSurfacePreference === 'host';
}

/** Validate the injected identity; the native host independently authorizes every operation. */
export function createNativePanelTransport(windowRef: Window): BrowserPanelTransport | null {
  const bridge: unknown = Reflect.get(windowRef, 'sugarcubesPanelHost');
  if (!isRecord(bridge)) return null;
  const search = windowRef.location.search;
  let identity = panelNavigation(search);
  const query = new URLSearchParams(search);
  if (
    !identity &&
    !['sc-panel-host', 'sc-panel-session', 'sc-panel-transport'].some((key) => query.has(key)) &&
    typeof bridge.host === 'string' &&
    typeof bridge.session === 'string'
  ) {
    const injected = new URLSearchParams({
      'sc-panel-host': bridge.host,
      'sc-panel-session': bridge.session,
    });
    identity = panelNavigation(injected.toString());
  }
  if (
    !identity ||
    bridge.host !== identity.host ||
    bridge.session !== identity.session ||
    typeof bridge.send !== 'function' ||
    typeof bridge.subscribe !== 'function'
  )
    return null;
  const preferencePolicy: unknown = bridge.preferencePolicy;
  if (preferencePolicy !== undefined && preferencePolicy !== 'host' && preferencePolicy !== 'saved')
    return null;
  const outputSurfacePreference: unknown = bridge.outputSurfacePreference;
  if (outputSurfacePreference !== undefined && outputSurfacePreference !== 'host') return null;
  const send = bridge.send;
  const subscribe = bridge.subscribe;
  return {
    session: identity.session,
    preferencePolicy: preferencePolicy ?? 'host',
    memoryBudget: true,
    ...(outputSurfacePreference ? { outputSurfacePreference } : {}),
    send: (message) => {
      send.call(bridge, message);
    },
    subscribe: (receive) => {
      const stop: unknown = subscribe.call(bridge, receive);
      if (typeof stop !== 'function') throw new Error('Invalid native panel subscription.');
      return () => {
        stop();
      };
    },
  };
}
