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
/**
 * Follow the ComfyUI a host runs through the `sugarcubes.backend` protocol
 * (`integrations/contracts/backend.schema.json`), and ask the host to start it again.
 *
 * The page sends `watch` once the host lists the protocol; the host answers with a state and sends
 * a new one whenever it changes. Every host message is admitted through the generated guard before
 * it is read. Starting ComfyUI again is the host's decision: the page only asks, naming the state
 * it was shown.
 */
import { isBackendStateMessage, } from '../embedded/BackendRuntime.js';
/** Protocol identifier shared with desktop hosts. */
export const BACKEND_PROTOCOL = 'sugarcubes.backend';
/** Keep one watcher informed of the host's backend, newest state only. */
export class HostBackendLink {
    transport;
    watcher;
    report;
    #stop = null;
    #revision = -1;
    #disposed = false;
    constructor(transport, watcher, report) {
        this.transport = transport;
        this.watcher = watcher;
        this.report = report;
    }
    /** Listen before asking the host to watch, so its synchronous answer cannot be lost. */
    start() {
        if (this.#stop || this.#disposed)
            return;
        this.#stop = this.transport.subscribe((value) => this.#receive(value));
        this.#send({ ...this.#envelope(), type: 'watch' });
    }
    /** Stop listening. */
    dispose() {
        this.#disposed = true;
        this.#stop?.();
        this.#stop = null;
    }
    #envelope() {
        return { protocol: BACKEND_PROTOCOL, version: 1, session: this.transport.session };
    }
    #receive(value) {
        if (!isBackendStateMessage(value) || value.session !== this.transport.session)
            return;
        // An answer to a repeated watch repeats a state already heard.
        if (value.revision <= this.#revision)
            return;
        this.#revision = value.revision;
        const { revision, status } = value;
        try {
            this.watcher.state({ status, revision }, () => {
                if (revision > 0)
                    this.#send({ ...this.#envelope(), type: 'retry', revision });
            });
        }
        catch (error) {
            this.report(error);
        }
    }
    /** Send one message; a transport failure retires the link and is reported once. */
    #send(message) {
        if (this.#disposed)
            return;
        try {
            this.transport.send(message);
        }
        catch (error) {
            this.report(error);
            this.dispose();
        }
    }
}
