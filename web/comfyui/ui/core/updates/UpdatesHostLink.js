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
 * Present a host's own updater to the page through the `sugarcubes.updates` protocol, version 1.
 *
 * `integrations/contracts/updates.schema.json` owns the wire shapes; every host message is
 * admitted through the generated guard before it is read.
 */
import { isUpdatesHostMessage, } from '../embedded/UpdatesRuntime.js';
/** Protocol identifier shared with desktop hosts. */
export const UPDATES_PROTOCOL = 'sugarcubes.updates';
/** Protocol version; it advances independently of the panel transport. */
export const UPDATES_PROTOCOL_VERSION = 1;
/** Translate the host's wire state into the snapshot every update source shares. */
function snapshot(state) {
    return Object.freeze({
        currentVersion: state.currentVersion,
        channel: state.channel,
        lastSuccessfulCheck: state.lastSuccessfulCheck?.completedAt ?? null,
        status: state.status,
        report: state.report ?? null,
        previousVersionRunning: state.previousVersionRunning,
        workflowRunning: state.workflowRunning,
        restartAfterWorkflow: state.scheduledRestart === 'after-workflow',
    });
}
/**
 * Watch the host's update state and forward the page's check and restart requests.
 *
 * Each request carries its own identity and settles on the host's answer naming it. A state
 * with a revision no higher than the last one is stale and ignored.
 */
export class UpdatesHostLink {
    transport;
    report;
    applies = 'host-restart';
    #listeners = new Set();
    #pending = new Map();
    #snapshot = null;
    #revision = 0;
    #requests = 0;
    #stop = null;
    #disposed = false;
    constructor(transport, report) {
        this.transport = transport;
        this.report = report;
    }
    /** Listen before asking the host to watch, so its synchronous first state cannot be lost. */
    start() {
        if (this.#stop || this.#disposed)
            return;
        this.#stop = this.transport.subscribe((value) => this.#receive(value));
        this.#send({ ...this.#envelope(), type: 'watch' });
    }
    read() {
        return this.#snapshot;
    }
    subscribe(changed) {
        this.#listeners.add(changed);
        return () => this.#listeners.delete(changed);
    }
    check() {
        return this.#request('check', (request) => ({ ...this.#envelope(), type: 'check', request }));
    }
    apply(when) {
        return this.#request('restart', (request) => ({
            ...this.#envelope(),
            type: 'restart',
            request,
            when,
        }));
    }
    /** Stop watching; requests still waiting for an answer settle as unavailable. */
    dispose() {
        this.#disposed = true;
        this.#stop?.();
        this.#stop = null;
        this.#settleAll({ accepted: false, reason: 'unavailable' });
        this.#listeners.clear();
    }
    #envelope() {
        return {
            protocol: UPDATES_PROTOCOL,
            version: UPDATES_PROTOCOL_VERSION,
            session: this.transport.session,
        };
    }
    #request(kind, message) {
        if (this.#disposed || !this.#stop)
            return Promise.resolve({ accepted: false, reason: 'unavailable' });
        const request = `${kind}-${String(++this.#requests)}`;
        return new Promise((resolve) => {
            this.#pending.set(request, resolve);
            if (!this.#send(message(request)))
                this.#settle(request, { accepted: false, reason: 'unavailable' });
        });
    }
    #receive(value) {
        if (!isUpdatesHostMessage(value) || value.session !== this.transport.session)
            return;
        if (value.type === 'accepted')
            this.#settle(value.request, { accepted: true });
        else if (value.type === 'rejected')
            this.#settle(value.request, { accepted: false, reason: value.reason });
        else if (value.revision > this.#revision) {
            this.#revision = value.revision;
            this.#snapshot = snapshot(value.update);
            for (const changed of [...this.#listeners])
                changed();
        }
    }
    #settle(request, outcome) {
        const resolve = this.#pending.get(request);
        if (!resolve)
            return;
        this.#pending.delete(request);
        resolve(outcome);
    }
    #settleAll(outcome) {
        for (const request of [...this.#pending.keys()])
            this.#settle(request, outcome);
    }
    /** Send one message; a transport failure retires the link and is reported once. */
    #send(message) {
        if (this.#disposed)
            return false;
        try {
            this.transport.send(message);
            return true;
        }
        catch (error) {
            this.report(error);
            this.dispose();
            return false;
        }
    }
}
