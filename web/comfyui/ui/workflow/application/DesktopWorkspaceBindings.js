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
/** Bind durable document occurrences to fresh Comfy sessions and immutable observation fences. */
import { sessionUuid } from '../../core/embedded/SessionRuntime.js';
/** Keep document identity independent of paths, labels, graph IDs, and browser object lifetimes. */
export class DesktopWorkspaceBindings {
    webview;
    nextId;
    #entries = new Map();
    #listeners = new Set();
    #revision = 1;
    #active = null;
    #loading = true;
    /** Receive only independently granted page identity and an adapter-owned UUID source. */
    constructor(webview, nextId) {
        this.webview = webview;
        this.nextId = nextId;
        if (!sessionUuid(webview))
            throw new TypeError('Desktop session webview identity is invalid.');
    }
    /** Reflect the authoritative open order while keeping renames on the same occurrence. */
    synchronize(snapshot) {
        if (snapshot.open.length > 256)
            throw new RangeError('Desktop workspace has too many occurrences.');
        const open = new Set(snapshot.open.map((context) => context.sessionId));
        if (open.size !== snapshot.open.length)
            throw new TypeError('Desktop runtime workflow identity was reused.');
        let changed = this.#active !== (snapshot.active?.sessionId ?? null) ||
            this.#loading !== snapshot.loading ||
            [...this.#entries.keys()].some((id) => !open.has(id));
        const ordered = new Map();
        for (const context of snapshot.open) {
            let entry = this.#entries.get(context.sessionId);
            if (!entry) {
                const entryId = this.nextId();
                if (!sessionUuid(entryId) ||
                    [...this.#entries.values(), ...ordered.values()].some((e) => e.entryId === entryId))
                    throw new TypeError('Desktop occurrence identity was invalid or reused.');
                entry = {
                    entryId,
                    context,
                    graphRevision: 0,
                    sourceRevision: 0,
                    navigationRevision: 0,
                    signatures: new Map(),
                };
                changed = true;
            }
            else if (entry.context.generation !== context.generation ||
                entry.context.title !== context.title ||
                entry.context.path !== context.path) {
                entry = {
                    ...entry,
                    context,
                    signatures: entry.context.generation !== context.generation ? new Map() : entry.signatures,
                };
                changed = true;
            }
            ordered.set(context.sessionId, entry);
        }
        if ([...ordered.keys()].some((id, index) => [...this.#entries.keys()][index] !== id))
            changed = true;
        this.#entries.clear();
        for (const [id, entry] of ordered)
            this.#entries.set(id, entry);
        this.#active = snapshot.active?.sessionId ?? null;
        this.#loading = snapshot.loading;
        if (changed)
            this.changed();
    }
    /** Adopt the host's durable identity only for the exact freshly materialized runtime binding. */
    bind(binding) {
        const entry = this.#require(binding.runtimeSession);
        if (!sessionUuid(binding.entryId) ||
            binding.generation !== entry.context.generation ||
            [...this.#entries.values()].some((e) => e !== entry && e.entryId === binding.entryId))
            throw new TypeError('Restored desktop occurrence binding is invalid.');
        this.#entries.set(entry.context.sessionId, {
            ...entry,
            entryId: binding.entryId,
            graphRevision: binding.graphRevision,
            sourceRevision: binding.sourceRevision,
            navigationRevision: binding.navigationRevision,
        });
        this.changed();
    }
    /** Advance only the owner whose immutable semantic snapshot actually changed. */
    observe(session, owner, signature) {
        const entry = this.#require(session);
        if (entry.signatures.get(owner) === signature)
            return;
        entry.signatures.set(owner, signature);
        if (owner === 'graph')
            entry.graphRevision++;
        else if (owner === 'source')
            entry.sourceRevision++;
        else
            entry.navigationRevision++;
        this.changed();
    }
    /** Establish imported semantic baselines without treating the restore itself as a new authored edit. */
    prime(session, owner, signature) {
        this.#require(session).signatures.set(owner, signature);
    }
    /** Return a detached binding; consumers cannot mutate accepted runtime or revision facts. */
    binding(session) {
        const entry = this.#require(session);
        return Object.freeze({
            entryId: entry.entryId,
            runtimeSession: entry.context.sessionId,
            generation: entry.context.generation,
            graphRevision: entry.graphRevision,
            sourceRevision: entry.sourceRevision,
            navigationRevision: entry.navigationRevision,
        });
    }
    /** Capture the complete immutable observation set, distinct from host durable checkpoint revision. */
    fence() {
        return Object.freeze({
            webview: this.webview,
            revision: this.#revision,
            bindings: Object.freeze([...this.#entries.keys()].map((id) => this.binding(id))),
        });
    }
    /** Advance readiness/capability changes even when no document binding changed. */
    changed() {
        this.#revision++;
        for (const listener of this.#listeners)
            listener();
    }
    /** Observe explicit semantic transitions without polling browser or host presentation. */
    subscribe(listener) {
        this.#listeners.add(listener);
        return () => {
            this.#listeners.delete(listener);
        };
    }
    /** Retire observation state with the page rather than allowing late loads to revive it. */
    dispose() {
        this.#entries.clear();
        this.#listeners.clear();
    }
    /** Refuse missing/closed occurrences; a string from a checkpoint cannot create a live binding. */
    #require(session) {
        const entry = [...this.#entries.values()].find((e) => e.context.sessionId === session);
        if (!entry)
            throw new Error('Desktop workflow occurrence is unavailable.');
        return entry;
    }
}
