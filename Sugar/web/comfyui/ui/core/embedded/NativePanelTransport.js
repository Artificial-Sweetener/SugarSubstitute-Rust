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
import { panelNavigation } from './BrowserPanelTransport.js';
import { NativePanelSendQueue } from './NativePanelSendQueue.js';
/** Validate the injected identity; the native host independently authorizes every operation. */
export function createNativePanelTransport(windowRef) {
    const bridge = Reflect.get(windowRef, 'sugarcubesPanelHost');
    if (!isRecord(bridge))
        return null;
    const search = windowRef.location.search;
    let identity = panelNavigation(search);
    const query = new URLSearchParams(search);
    if (!identity &&
        !['sc-panel-host', 'sc-panel-session', 'sc-panel-transport'].some((key) => query.has(key)) &&
        typeof bridge.host === 'string' &&
        typeof bridge.session === 'string') {
        const injected = new URLSearchParams({
            'sc-panel-host': bridge.host,
            'sc-panel-session': bridge.session,
        });
        identity = panelNavigation(injected.toString());
    }
    if (!identity ||
        bridge.host !== identity.host ||
        bridge.session !== identity.session ||
        typeof bridge.send !== 'function' ||
        typeof bridge.subscribe !== 'function')
        return null;
    const send = bridge.send;
    const subscribe = bridge.subscribe;
    const outbound = new NativePanelSendQueue({
        send: (message) => {
            send.call(bridge, message);
        },
        later: (callback, milliseconds) => {
            const timer = windowRef.setTimeout(callback, milliseconds);
            return () => windowRef.clearTimeout(timer);
        },
    });
    return {
        session: identity.session,
        send: (message) => {
            outbound.send(message);
        },
        subscribe: (receive) => {
            let listening = hosts.get(bridge);
            if (!listening) {
                listening = new SharedSubscription((deliver) => {
                    const stop = subscribe.call(bridge, deliver);
                    if (typeof stop !== 'function')
                        throw new Error('Invalid native panel subscription.');
                    return () => {
                        stop();
                    };
                });
                hosts.set(bridge, listening);
            }
            return listening.add((value) => {
                try {
                    receive(value);
                }
                finally {
                    outbound.progressed();
                }
            });
        },
    };
}
/** The one subscription held on each injected bridge. */
const hosts = new WeakMap();
/**
 * Hear one host bridge once and pass what it says to every listener in the page.
 *
 * The host admits only a few listeners per page, and each of SugarCubes' links to it (canvas,
 * backend, updates, release notes, file drags and more) listens on its own. Subscribing each
 * one natively ran past that limit, and the host refused the last link to start. They share
 * one native subscription here, opened by the first listener and closed by the last.
 */
class SharedSubscription {
    open;
    #receivers = new Set();
    #stop = null;
    constructor(open) {
        this.open = open;
    }
    /** Add a listener; the returned function removes exactly that one. */
    add(receive) {
        // A fresh function per call, so one listener added twice is two subscriptions.
        const receiver = (value) => receive(value);
        this.#receivers.add(receiver);
        if (!this.#stop) {
            try {
                this.#stop = this.open((value) => this.#deliver(value));
            }
            catch (error) {
                this.#receivers.delete(receiver);
                throw error;
            }
        }
        return () => {
            if (!this.#receivers.delete(receiver) || this.#receivers.size > 0)
                return;
            const stop = this.#stop;
            this.#stop = null;
            stop?.();
        };
    }
    /** Give `value` to every current listener; one that throws does not starve the others. */
    #deliver(value) {
        let failure = null;
        for (const receiver of [...this.#receivers]) {
            if (!this.#receivers.has(receiver))
                continue;
            try {
                receiver(value);
            }
            catch (error) {
                failure ??= { error };
            }
        }
        if (failure)
            throw failure.error;
    }
}
