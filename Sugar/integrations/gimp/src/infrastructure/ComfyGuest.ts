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
/** Install the native viewer's public transport before the existing ComfyUI application starts. */
import { TauriPanelTransport } from './TauriPanelTransport.js';

/** Expose public protocol delivery only for this native navigation, without impersonating UXP. */
export function installComfyGuest(windowRef: Window): void {
  const query = new URLSearchParams(windowRef.location.search);
  const host = query.get('sc-panel-host');
  const session = query.get('sc-panel-session');
  if (
    host !== 'ai.artificialsweetener.sugarcubes.gimp' ||
    !session ||
    query.get('sc-panel-transport') !== 'native'
  )
    return;
  const report = (error: unknown): void => {
    console.error('SugarCubes companion public transport failed', { error });
  };
  const transport = new TauriPanelTransport(windowRef, session, 'guest', report);
  Object.defineProperty(windowRef, 'sugarcubesPanelHost', {
    value: Object.freeze({
      host,
      session,
      send: transport.send.bind(transport),
      subscribe: transport.subscribe.bind(transport),
    }),
    configurable: false,
    writable: false,
  });
  windowRef.addEventListener(
    'pagehide',
    () => {
      transport.dispose();
    },
    { once: true },
  );
}

installComfyGuest(window);
