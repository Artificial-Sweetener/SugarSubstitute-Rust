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
 * Feed the Updates entry from SugarCubes' own updater when no host updates SugarCubes.
 *
 * The backend checks and stages releases in the background and pushes every state change over
 * ComfyUI's websocket as `sugarcubes.self_update`. Choosing to apply records the choice; the
 * files are replaced the next time ComfyUI starts, before SugarCubes loads, so this source never
 * restarts anything itself.
 */
import { HOST_PROTOCOLS } from '../embedded/PanelRuntime.js';
import { isRecord } from '../../types/common.js';
/** The ComfyUI websocket event that carries each updater state change. */
export const SELF_UPDATE_EVENT = 'sugarcubes.self_update';
const STATE_PATH = '/sugarcubes/self-update';
const UNMANAGED = {
    'git-checkout': 'git-checkout',
    'changed-files': 'local-changes',
    'linked-folder': 'linked-folder',
    'linked-content': 'linked-folder',
    'not-registry-install': 'not-registry-install',
    'invalid-tracking': 'not-registry-install',
    missing: 'not-registry-install',
};
const VERSION = /^[0-9]{1,9}\.[0-9]{1,9}\.[0-9]{1,9}$/;
function record(value) {
    return isRecord(value) ? value : null;
}
function versionOrNull(value) {
    if (value === null)
        return null;
    return typeof value === 'string' && VERSION.test(value) ? value : undefined;
}
function decodeNotice(value) {
    if (value === null)
        return null;
    const notice = record(value);
    const kind = notice?.kind;
    if (!notice ||
        (kind !== 'installed' && kind !== 'restored' && kind !== 'not-installed') ||
        typeof notice.reason !== 'string' ||
        typeof notice.version !== 'string' ||
        typeof notice.message !== 'string' ||
        notice.message.length === 0 ||
        notice.message.length > 1024)
        return undefined;
    return { kind, reason: notice.reason, version: notice.version, message: notice.message };
}
/** Validate one backend state object; anything unexpected is rejected whole. */
export function decodeSelfUpdateState(value) {
    const state = record(value);
    if (!state || state.schema !== 1)
        return null;
    const currentVersion = versionOrNull(state.currentVersion);
    const availableVersion = versionOrNull(state.availableVersion);
    const notice = decodeNotice(state.notice);
    const uncheckedSources = decodeUncheckedSources(state.uncheckedSources);
    const failure = state.failure;
    const lastCheckedAt = state.lastCheckedAt;
    if (typeof currentVersion !== 'string' ||
        availableVersion === undefined ||
        notice === undefined ||
        uncheckedSources === null ||
        typeof state.state !== 'string' ||
        !(failure === null || failure === 'check' || failure === 'download' || failure === 'verify') ||
        !(state.notManaged === null || typeof state.notManaged === 'string') ||
        !(lastCheckedAt === null ||
            (Number.isSafeInteger(lastCheckedAt) && Number(lastCheckedAt) >= 0)) ||
        !Number.isSafeInteger(state.revision) ||
        Number(state.revision) < 1)
        return null;
    return {
        currentVersion,
        state: state.state,
        availableVersion,
        failure,
        notManaged: typeof state.notManaged === 'string' ? state.notManaged : null,
        lastCheckedAt: typeof lastCheckedAt === 'number' ? lastCheckedAt : null,
        revision: Number(state.revision),
        notice,
        uncheckedSources,
    };
}
function statusOf(state) {
    const version = state.availableVersion;
    switch (state.state) {
        case 'idle':
        case 'checking':
            return { state: 'checking' };
        case 'up-to-date':
            return { state: 'up-to-date' };
        case 'downloading':
            return version ? { state: 'available', version } : null;
        case 'ready':
            return version ? { state: 'ready', version } : null;
        case 'scheduled':
            return version ? { state: 'restart-comfyui', version } : null;
        case 'failed':
            return { state: 'failed', during: state.failure === 'check' ? 'check' : 'update' };
        case 'not-managed': {
            const reason = state.notManaged ? UNMANAGED[state.notManaged] : undefined;
            return reason ? { state: 'unmanaged', reason } : null;
        }
        default:
            return null;
    }
}
function decodeUncheckedSources(value) {
    if (!Array.isArray(value) || value.length > 2)
        return null;
    const messages = [];
    for (const item of value) {
        const entry = record(item);
        const message = entry?.message;
        if (typeof entry?.source !== 'string' || typeof message !== 'string')
            return null;
        if (message.length === 0 || message.length > 1024)
            return null;
        messages.push(message);
    }
    return messages;
}
function reportOf(state) {
    const notice = state.notice;
    if (!notice || notice.kind === 'installed' || !VERSION.test(notice.version)) {
        if (state.uncheckedSources.length === 0)
            return null;
        return {
            version: state.currentVersion,
            phase: 'check',
            causes: state.uncheckedSources.map((message) => ({ message })),
        };
    }
    return {
        version: notice.version,
        phase: notice.kind === 'restored' && notice.reason === 'did-not-start' ? 'start' : 'install',
        causes: [{ message: notice.message }],
    };
}
/** Translate one validated backend state into the entry's snapshot, or null if it cannot be shown. */
export function selfUpdateSnapshot(state) {
    const status = statusOf(state);
    if (!status)
        return null;
    return {
        currentVersion: state.currentVersion,
        channel: 'stable',
        lastSuccessfulCheck: state.lastCheckedAt,
        status,
        report: reportOf(state),
        previousVersionRunning: state.notice?.kind === 'restored',
        workflowRunning: false,
        restartAfterWorkflow: false,
    };
}
/** SugarCubes' own updater as the page's update source. */
export class SelfUpdateSource {
    transport;
    report;
    applies = 'comfyui-restart';
    #snapshot = null;
    #state = null;
    #retired = false;
    #listeners = new Set();
    #releases = [];
    constructor(transport, report) {
        this.transport = transport;
        this.report = report;
    }
    /** Follow pushed state, read the current one, and ask for a background check. */
    start() {
        this.#releases.push(this.transport.subscribe((payload) => this.#accept(payload)));
        this.#releases.push(this.transport.subscribeReconnect(() => void this.#refresh()));
        void this.#refresh().then(() => this.#send('POST', `${STATE_PATH}/check`, { force: false }));
    }
    /** Stop following the backend; pending requests settle as unavailable. */
    retire() {
        this.#retired = true;
        for (const release of this.#releases.splice(0))
            release();
    }
    read() {
        return this.#snapshot;
    }
    subscribe(changed) {
        this.#listeners.add(changed);
        return () => this.#listeners.delete(changed);
    }
    check() {
        return this.#send('POST', `${STATE_PATH}/check`, { force: true });
    }
    apply() {
        const version = this.#state?.state === 'ready' ? this.#state.availableVersion : null;
        if (!version)
            return Promise.resolve({ accepted: false, reason: 'not-ready' });
        return this.#send('POST', `${STATE_PATH}/install`, { version });
    }
    async #refresh() {
        await this.#send('GET', STATE_PATH);
    }
    async #send(method, path, body) {
        if (this.#retired)
            return { accepted: false, reason: 'unavailable' };
        let reply;
        try {
            reply = await this.transport.request(method, path, body);
        }
        catch (error) {
            this.report(error);
            return { accepted: false, reason: 'unavailable' };
        }
        if (this.#retired)
            return { accepted: false, reason: 'unavailable' };
        if (reply.status === 409)
            return { accepted: false, reason: 'not-ready' };
        if (reply.status < 200 || reply.status >= 300 || !this.#accept(reply.body))
            return { accepted: false, reason: 'unavailable' };
        return { accepted: true };
    }
    /** Keep only newer valid states; returns whether the payload was a valid state. */
    #accept(payload) {
        const state = decodeSelfUpdateState(payload);
        if (!state)
            return false;
        if (this.#state && state.revision <= this.#state.revision)
            return true;
        const snapshot = selfUpdateSnapshot(state);
        if (!snapshot)
            return false;
        this.#state = state;
        this.#snapshot = snapshot;
        for (const changed of [...this.#listeners])
            changed();
        return true;
    }
}
/**
 * Attach SugarCubes' own updater once the host description has settled without a host updater.
 *
 * @returns The release that stops waiting, or retires the attached source.
 */
export function attachSelfUpdateWhenUnhosted(options) {
    let release = null;
    let stopWaiting = null;
    const settle = () => {
        const declared = options.host.read();
        if (!declared)
            return false;
        stopWaiting?.();
        stopWaiting = null;
        if (declared.serves(HOST_PROTOCOLS.updates) || options.updates.read())
            return true;
        const source = new SelfUpdateSource(options.transport, options.report);
        const detach = options.updates.attach(source);
        source.start();
        release = () => {
            source.retire();
            detach();
        };
        return true;
    };
    if (!settle())
        stopWaiting = options.host.subscribe(() => void settle());
    return () => {
        stopWaiting?.();
        release?.();
    };
}
/** Adapt ComfyUI's `api` object (fetchApi and its websocket events) to the transport port. */
export function comfySelfUpdateTransport(api) {
    const comfy = record(api);
    const fetchApi = comfy?.fetchApi;
    const addEventListener = comfy?.addEventListener;
    const removeEventListener = comfy?.removeEventListener;
    if (!comfy ||
        typeof fetchApi !== 'function' ||
        typeof addEventListener !== 'function' ||
        typeof removeEventListener !== 'function')
        return null;
    const listen = (type, listener) => {
        addEventListener.call(comfy, type, listener);
        return () => removeEventListener.call(comfy, type, listener);
    };
    return {
        async request(method, path, body) {
            const init = body === undefined
                ? { method }
                : { method, headers: { 'Content-Type': 'application/json' }, body: JSON.stringify(body) };
            const response = await fetchApi.call(comfy, path, init);
            const reply = record(response);
            const status = reply?.status;
            const json = reply?.json;
            if (typeof status !== 'number' || typeof json !== 'function')
                throw new TypeError('ComfyUI returned an unreadable reply.');
            const decoded = await Promise.resolve(json.call(response)).catch(() => null);
            return { status, body: decoded };
        },
        subscribe: (receive) => listen(SELF_UPDATE_EVENT, (event) => receive(record(event)?.detail ?? null)),
        subscribeReconnect: (reconnected) => listen('reconnected', () => reconnected()),
    };
}
