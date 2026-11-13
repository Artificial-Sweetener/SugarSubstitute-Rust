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
/** Adapt the dedicated native workspace transport independently of canvas message capacity. */
import { isSessionScope, sessionScope, sessionUuid, sessionMessageError, sameSessionFence, parseSessionJson, isSessionBinding, } from '../../core/embedded/SessionRuntime.js';
import { isRecord } from '../../types/common.js';
/** Validate injected facts; the native host independently admits origin, page lifetime and backend grant. */
export function createNativeDesktopSessionTransport(window) {
    const bridge = Reflect.get(window, 'sugarcubesSessionHost');
    if (bridge === undefined)
        return null;
    if (!isRecord(bridge) ||
        !isSessionScope(bridge.scope) ||
        !sessionScope(bridge.scope) ||
        typeof bridge.origin !== 'string' ||
        bridge.origin !== window.location.origin ||
        typeof bridge.webview !== 'string' ||
        !sessionUuid(bridge.webview) ||
        typeof bridge.send !== 'function' ||
        typeof bridge.subscribe !== 'function')
        throw new TypeError('Native desktop session bootstrap is invalid.');
    const scope = Object.freeze({ ...bridge.scope });
    const origin = bridge.origin;
    const webview = bridge.webview;
    const send = bridge.send;
    const subscribe = bridge.subscribe;
    const media = bridge.media;
    if (media !== undefined &&
        (!isRecord(media) || media.version !== 1 || typeof media.assetUrl !== 'function'))
        throw new TypeError('Native desktop session media capability is invalid.');
    return Object.freeze({
        scope,
        origin,
        webview,
        async send(message) {
            const error = sessionMessageError(message, scope, origin);
            if (error)
                throw new TypeError(`Native desktop session message was refused: ${error}.`);
            await send.call(bridge, message);
        },
        subscribe(receive) {
            const stop = subscribe.call(bridge, receive);
            if (typeof stop !== 'function')
                throw new TypeError('Native desktop session subscription is invalid.');
            return () => {
                stop();
            };
        },
        ...(isRecord(media) && typeof media.assetUrl === 'function'
            ? {
                assetUrl(outputId, binding) {
                    if (!sessionUuid(outputId))
                        throw new TypeError('Retained output identity is invalid.');
                    const method = media.assetUrl;
                    if (typeof method !== 'function')
                        throw new TypeError('Native session media capability retired.');
                    const value = method.call(media, outputId, binding);
                    if (value === null)
                        return null;
                    if (typeof value !== 'string')
                        throw new TypeError('Native retained output URL is invalid.');
                    const url = new URL(value, origin);
                    const encoded = url.searchParams.get('binding');
                    const decoded = encoded ? parseSessionJson(encoded, 4096) : null;
                    if (url.origin !== origin ||
                        value.length > 4096 ||
                        [...url.searchParams.keys()].length !== 1 ||
                        url.searchParams.getAll('binding').length !== 1 ||
                        url.hash ||
                        !isSessionBinding(decoded) ||
                        !sameSessionFence({ webview, revision: 0, bindings: [decoded] }, { webview, revision: 0, bindings: [binding] }) ||
                        !new RegExp(`^/__sugarsubstitute/session-media/v1/[0-9a-f]{32}/${outputId}$`).test(url.pathname))
                        throw new TypeError('Native retained output URL escaped its admitted origin or identity.');
                    return url.href;
                },
            }
            : {}),
    });
}
