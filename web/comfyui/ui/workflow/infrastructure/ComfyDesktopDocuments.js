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
/** Capture and materialize Comfy-owned documents through the exposed workflow and graph surfaces. */
import { SESSION_DOCUMENT_BYTES, sessionGraph, sessionJsonScalars, utf8Bytes, isSessionDocument, } from '../../core/embedded/SessionRuntime.js';
import { isRecord } from '../../types/common.js';
import { DesktopSessionRefusal } from '../domain/DesktopSession.js';
/**
 * Capture Comfy-owned documents and match a checkpoint to the tabs Comfy restored.
 *
 * Comfy owns workflow tabs: it reopens them from its own persistence. This owner never opens,
 * closes, reorders or reloads a tab; it only reads documents and names which restored tab a
 * checkpoint entry describes.
 */
export class ComfyDesktopDocuments {
    options;
    #app;
    #workflow;
    #sourceIdentities = new WeakMap();
    /** Validate deployed capabilities once; missing capability disables desktop capture. */
    constructor(options) {
        this.options = options;
        const app = options.app;
        const manager = isRecord(app) && isRecord(app.extensionManager) ? app.extensionManager : null;
        if (!isRecord(app) || !manager || !isRecord(manager.workflow))
            throw new TypeError('Comfy desktop workflow capabilities are unavailable.');
        this.#app = app;
        this.#workflow = manager.workflow;
    }
    /**
     * List open workflows whose document Comfy has not read yet: a saved tab restored in the
     * background or opened but still awaiting its file hold neither a tracked graph nor content.
     * They are a normal transient state of Comfy's tab lifecycle, not a capture failure.
     */
    unread() {
        const snapshot = this.options.sessions.read();
        return snapshot.open.filter((context) => context.sessionId !== snapshot.active?.sessionId &&
            !readable(this.options.adapter.readHostWorkflow(context.sessionId)));
    }
    /** Whether Comfy is itself fetching this workflow's file; a second load would race its change tracker. */
    isBeingRead(context) {
        const workflow = this.options.adapter.readHostWorkflow(context.sessionId);
        return isRecord(workflow) && workflow.isLoading === true;
    }
    /** Ask Comfy to read one saved workflow through its own load, without activating the tab or loading its graph. */
    async read(context) {
        const workflow = this.options.adapter.readHostWorkflow(context.sessionId);
        if (!isRecord(workflow))
            throw new Error('Comfy workflow occurrence is unavailable.');
        await invoke(workflow, 'load');
        if (!readable(workflow))
            throw new Error('Comfy did not provide the workflow document after loading it.');
    }
    /** Capture the active root graph or the explicitly named inactive tracker/content, never a peer's graph. */
    capture(context, graphRevision) {
        const snapshot = this.options.sessions.read();
        if (snapshot.loading)
            throw new DesktopSessionRefusal('not-committed', 'Comfy workflow capture requires a committed graph.');
        if (!snapshot.open.some((item) => item.sessionId === context.sessionId && item.generation === context.generation))
            throw new DesktopSessionRefusal('stale', 'Comfy workflow capture generation is stale.');
        const workflow = this.options.adapter.readHostWorkflow(context.sessionId);
        if (!isRecord(workflow))
            throw new Error('Comfy workflow occurrence is unavailable.');
        const tracker = isRecord(workflow.changeTracker) ? workflow.changeTracker : null;
        const active = snapshot.active?.sessionId === context.sessionId;
        const root = this.#app.rootGraph;
        let graph = tracker?.activeState;
        if (active)
            graph = invoke(root, 'serialize');
        else if (!isRecord(graph))
            graph = parseGraph(workflow.content);
        if (!isRecord(graph))
            throw new DesktopSessionRefusal('not-committed', 'Comfy has not read this workflow document yet.');
        const graphJson = encodeGraph(graph);
        const baseline = tracker?.initialState ?? parseGraph(workflow.originalContent);
        const baselineGraphJson = baseline === null ? null : encodeGraph(baseline);
        const temporary = workflow.isTemporary === true;
        const identityOwner = tracker ?? workflow;
        if (!this.#sourceIdentities.has(identityOwner))
            this.#sourceIdentities.set(identityOwner, temporary ? null : text(workflow.path));
        const ds = active ? (isRecord(this.#app.canvas) ? this.#app.canvas.ds : null) : tracker?.ds;
        const document = {
            graphJson,
            baselineGraphJson,
            graphRevision,
            path: temporary ? null : text(workflow.path),
            name: text(workflow.filename) ?? context.title,
            temporary,
            dirty: workflow.isModified === true,
            sourceIdentity: this.#sourceIdentities.get(identityOwner) ?? null,
            view: graphView(ds),
        };
        validateDocument(document);
        return Object.freeze({ ...document, view: Object.freeze(document.view) });
    }
    /**
     * Name the tab Comfy restored for each checkpoint entry: a saved entry by its path, a
     * temporary one by its name among Comfy's temporary tabs. An entry Comfy did not reopen has
     * no tab and is left out; a tab no entry describes keeps only what Comfy restored.
     */
    match(entries, current) {
        for (const entry of entries)
            validateDocument(entry.document);
        assertCurrent(current);
        if (!this.options.adapter.refresh())
            throw new Error('Comfy restored workflow observation failed.');
        const open = this.#workflow.openWorkflows;
        if (!Array.isArray(open) || !open.every(isRecord))
            throw new TypeError('Comfy desktop open workflows are malformed.');
        const unclaimed = [...open];
        const committed = this.options.sessions.read().open;
        const contexts = new Map();
        for (const entry of entries) {
            const document = entry.document;
            const index = unclaimed.findIndex((workflow) => document.temporary
                ? workflow.isTemporary === true && text(workflow.filename) === document.name
                : workflow.isTemporary !== true && text(workflow.path) === document.path);
            if (index < 0)
                continue;
            const [workflow] = unclaimed.splice(index, 1);
            const id = workflow ? this.options.adapter.identifyHostWorkflow(workflow) : null;
            const context = id ? committed.find((item) => item.sessionId === id) : undefined;
            if (context)
                contexts.set(entry.id, context);
        }
        return contexts;
    }
}
/** A workflow exposes its document through a tracked graph or serialized content once Comfy has read it. */
function readable(workflow) {
    if (!isRecord(workflow))
        return false;
    const tracker = workflow.changeTracker;
    return ((isRecord(tracker) && isRecord(tracker.activeState)) ||
        (typeof workflow.content === 'string' && workflow.content.length > 0));
}
/** Call a qualified exposed method with its owning receiver. */
function invoke(owner, name, ...args) {
    if (!isRecord(owner) || typeof owner[name] !== 'function')
        throw new TypeError(`Comfy desktop capability ${name} is unavailable.`);
    return Reflect.apply(owner[name], owner, args);
}
/** Refuse late effects after cancellation or a competing user load. */
function assertCurrent(current) {
    if (!current())
        throw new DesktopSessionRefusal('cancelled', 'Comfy desktop materialization was superseded.');
}
/** Parse only bounded graph objects, retaining null as explicit unavailable baseline. */
function parseGraph(value) {
    if (value === null || value === undefined)
        return null;
    if (typeof value !== 'string' || !sessionGraph(value))
        throw new TypeError('Comfy desktop serialized graph is invalid.');
    const graph = JSON.parse(value);
    if (!isRecord(graph) || Array.isArray(graph))
        throw new TypeError('Comfy graph must be an object.');
    return graph;
}
/** Preserve Comfy's optional JSON fields while refusing lossy scalar values before serialization. */
function encodeGraph(value) {
    if (!isRecord(value) || Array.isArray(value))
        throw new TypeError('Comfy graph is unavailable.');
    const encoded = JSON.stringify(value, (key, item) => {
        if (!Number.isFinite(utf8Bytes(key)) ||
            (item !== undefined && typeof item !== 'object' && !sessionJsonScalars(item)))
            throw new TypeError('Comfy graph contains unsupported snapshot values.');
        return item;
    });
    if (!sessionGraph(encoded))
        throw new TypeError('Comfy graph exceeds desktop snapshot admission.');
    return encoded;
}
/** Preserve exact document spelling; empty paths remain explicit null. */
function text(value) {
    return typeof value === 'string' && value.length > 0 ? value : null;
}
/** Capture graph-view units through Comfy's own scale/offset surface. */
function graphView(value) {
    const ds = isRecord(value) ? value : null;
    const offset = ds?.offset;
    const x = Array.isArray(offset) ? offset[0] : 0;
    const y = Array.isArray(offset) ? offset[1] : 0;
    const scale = ds?.scale ?? 1;
    if (![x, y, scale].every((item) => typeof item === 'number' && Number.isFinite(item)))
        throw new TypeError('Comfy graph view is invalid.');
    if (typeof x !== 'number' || typeof y !== 'number' || typeof scale !== 'number')
        throw new TypeError('Comfy graph view is unavailable.');
    return { x, y, scale };
}
/** Enforce the combined baseline/current budget before graph decoding or host effects. */
function validateDocument(document) {
    if (!isSessionDocument(document) ||
        utf8Bytes(document.graphJson) + utf8Bytes(document.baselineGraphJson ?? '') >
            SESSION_DOCUMENT_BYTES ||
        utf8Bytes(JSON.stringify(document)) > SESSION_DOCUMENT_BYTES ||
        !sessionGraph(document.graphJson) ||
        (document.baselineGraphJson !== null && !sessionGraph(document.baselineGraphJson)))
        throw new TypeError('Comfy desktop document failed snapshot admission.');
}
