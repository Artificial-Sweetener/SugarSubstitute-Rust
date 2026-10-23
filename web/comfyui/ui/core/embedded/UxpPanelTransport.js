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
/** Retain the navigation identity while allowing UXP to inject its bridge after frontend startup. */
export function createUxpPanelTransport(windowRef) {
    const query = new URLSearchParams(windowRef.location.search);
    const host = query.get('sc-panel-host');
    const session = query.get('sc-panel-session');
    if (!host ||
        !session ||
        !/^[a-zA-Z0-9._-]{1,128}$/.test(host) ||
        !/^[a-zA-Z0-9-]{16,80}$/.test(session) ||
        query.getAll('sc-panel-host').length !== 1 ||
        query.getAll('sc-panel-session').length !== 1)
        return null;
    return {
        session,
        send: (message) => {
            const bridge = Reflect.get(windowRef, 'uxpHost');
            if (isRecord(bridge) && typeof bridge.postMessage === 'function')
                bridge.postMessage.call(bridge, message);
        },
        subscribe: (receive) => {
            const listener = (event) => {
                const source = event.source;
                const bridge = Reflect.get(windowRef, 'uxpHost');
                if (isRecord(bridge) &&
                    typeof bridge.postMessage === 'function' &&
                    event.origin === host &&
                    source === bridge)
                    receive(event.data);
            };
            windowRef.addEventListener('message', listener);
            return () => windowRef.removeEventListener('message', listener);
        },
    };
}
