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
/** Bind one browser document to negotiated host presentation and existing Comfy connectivity. */
import { PanelClientSession } from './PanelRuntime.js';
import { createUxpPanelTransport } from './UxpPanelTransport.js';
import { EmbeddedPanelLayout } from './EmbeddedPanelLayout.js';
/** Keep messaging active only while the owning SugarCubes document is alive. */
export class EmbeddedPanelHost {
    options;
    transport;
    #layout;
    #stop = null;
    #timer = null;
    /** Receive an identified adapter transport after Comfy and presentation setup have succeeded. */
    constructor(options, transport) {
        this.options = options;
        this.transport = transport;
        this.#layout = new EmbeddedPanelLayout(options.window);
    }
    /** Listen before announcing readiness so synchronous host responses cannot be lost. */
    setup() {
        if (this.#stop)
            return;
        const client = new PanelClientSession(this.transport.session, {
            send: (message) => this.transport.send(message),
            apply: (presentation) => {
                this.#layout.apply(presentation.resizeCornerSize);
                this.options.presentation.set(presentation);
                return true;
            },
            connected: this.options.connected,
            report: this.options.report,
        });
        this.#stop = this.transport.subscribe((value) => {
            try {
                client.receive(value);
            }
            catch (error) {
                this.options.report(error);
            }
        });
        const ready = () => {
            try {
                client.ready();
            }
            catch (error) {
                this.options.report(error);
            }
        };
        this.#timer = this.options.window.setInterval(ready, 1000);
        this.options.window.addEventListener('pagehide', this.dispose);
        ready();
    }
    /** Retire subscriptions and restore saved preferences when the embedded document leaves. */
    dispose = () => {
        this.#stop?.();
        this.#stop = null;
        if (this.#timer !== null)
            this.options.window.clearInterval(this.#timer);
        this.#timer = null;
        this.options.window.removeEventListener('pagehide', this.dispose);
        this.options.presentation.set(null);
        this.#layout.dispose();
    };
}
/** Activate embedded presentation only for an identified application WebView. */
export function startEmbeddedPanel(options) {
    const transport = createUxpPanelTransport(options.window);
    if (!transport)
        return null;
    const host = new EmbeddedPanelHost(options, transport);
    host.setup();
    return host;
}
