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
/** Adapt UXP WebView messaging into an explicitly identified browser transport. */
import { isRecord } from '../../types/common.js';
import { panelNavigation, type BrowserPanelTransport } from './BrowserPanelTransport.js';

/** Retain the navigation identity while allowing UXP to inject its bridge after frontend startup. */
export function createUxpPanelTransport(windowRef: Window): BrowserPanelTransport | null {
  const identity = panelNavigation(windowRef.location.search);
  if (!identity) return null;
  const { host, session } = identity;
  return {
    session,
    send: (message) => {
      const bridge: unknown = Reflect.get(windowRef, 'uxpHost');
      if (isRecord(bridge) && typeof bridge.postMessage === 'function')
        bridge.postMessage.call(bridge, message);
    },
    subscribe: (receive) => {
      const listener = (event: MessageEvent<unknown>): void => {
        const source: unknown = event.source;
        const bridge: unknown = Reflect.get(windowRef, 'uxpHost');
        if (
          isRecord(bridge) &&
          typeof bridge.postMessage === 'function' &&
          event.origin === host &&
          source === bridge
        )
          receive(event.data);
      };
      windowRef.addEventListener('message', listener);
      return () => windowRef.removeEventListener('message', listener);
    },
  };
}
