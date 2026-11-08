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
/** Preserve graph meaning and modified baselines without saving, deleting or importing a Comfy class. */
export class ComfyDesktopDocuments {
    options;
    #app;
    #workflow;
    #imported = new WeakSet();
    #sourceIdentities = new WeakMap();
    #stop = null;
    /** Validate deployed capabilities once; missing capability disables desktop materialization. */
    constructor(options) {
        this.options = options;
        const app = options.app;
        const manager = isRecord(app) && isRecord(app.extensionManager) ? app.extensionManager : null;
        if (!isRecord(app) || !manager || !isRecord(manager.workflow))
            throw new TypeError('Comfy desktop workflow capabilities are unavailable.');
        this.#app = app;
        this.#workflow = manager.workflow;
        for (const method of [
            'createNewTemporary',
            'closeWorkflow',
            'attachWorkflow',
            'reorderWorkflows',
        ])
            if (typeof this.#workflow[method] !== 'function')
                throw new TypeError(`Comfy desktop workflow capability ${method} is unavailable.`);
        if (typeof app.loadGraphData !== 'function')
            throw new TypeError('Comfy desktop graph loading is unavailable.');
    }
    /** Preserve the imported saved baseline after Comfy resets its tracker during later tab activation. */
    install() {
        this.#stop ??= this.options.sessions.subscribe((snapshot) => {
            if (snapshot.loading || !snapshot.active)
                return;
            const workflow = this.options.adapter.readHostWorkflow(snapshot.active.sessionId);
            if (!isRecord(workflow))
                return;
            const tracker = workflow.changeTracker;
            if (isRecord(tracker) && this.#imported.has(tracker))
                tracker.initialState = parseGraph(workflow.originalContent);
        });
    }
    /** Stop baseline observation while allowing Comfy to keep its current workflow objects. */
    dispose() {
        this.#stop?.();
        this.#stop = null;
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
    /** Build owned in-memory workflows in order; checkpoint bytes remain the recovery authority on refusal. */
    async materialize(entries, activeEntry, current) {
        if (entries.length === 0)
            throw new DesktopSessionRefusal('unsupported', 'Empty desktop checkpoints must seed from the committed Comfy default.');
        for (const entry of entries)
            validateDocument(entry.document);
        const previous = this.#workflow.openWorkflows;
        if (!Array.isArray(previous) || !previous.every(isRecord))
            throw new TypeError('Comfy desktop open workflows are malformed.');
        const original = [...previous];
        const restored = new Map();
        assertCurrent(current);
        for (const workflow of original) {
            await invoke(this.#workflow, 'closeWorkflow', workflow);
            assertCurrent(current);
        }
        this.#workflow.activeWorkflow = null;
        for (const entry of entries) {
            assertCurrent(current);
            const document = entry.document;
            const graph = parseGraph(document.graphJson);
            const workflow = invoke(this.#workflow, 'createNewTemporary', document.name, graph);
            if (!isRecord(workflow))
                throw new TypeError('Comfy did not create a workflow occurrence.');
            await this.#load(graph, workflow, current);
            const tracker = workflow.changeTracker;
            if (!isRecord(tracker))
                throw new TypeError('Comfy did not commit the restored graph.');
            if (!document.temporary && document.path) {
                await invoke(this.#workflow, 'closeWorkflow', workflow);
                invoke(workflow, 'updatePath', document.path);
                workflow.size = utf8Bytes(document.baselineGraphJson ?? document.graphJson);
                invoke(this.#workflow, 'attachWorkflow', workflow, restored.size);
            }
            tracker.initialState = parseGraph(document.baselineGraphJson ?? document.graphJson);
            tracker.activeState = parseGraph(document.graphJson);
            tracker.ds = { scale: document.view.scale, offset: [document.view.x, document.view.y] };
            workflow.originalContent = document.baselineGraphJson ?? document.graphJson;
            workflow.content = document.graphJson;
            workflow.isModified = document.dirty;
            this.#imported.add(tracker);
            this.#sourceIdentities.set(tracker, document.sourceIdentity);
            restored.set(entry.id, workflow);
        }
        assertCurrent(current);
        const active = activeEntry ? restored.get(activeEntry) : null;
        if (active) {
            const tracker = active.changeTracker;
            if (!isRecord(tracker))
                throw new TypeError('Comfy restored active workflow is unavailable.');
            await this.#load(tracker.activeState, active, current);
            const entry = entries.find((candidate) => candidate.id === activeEntry);
            if (entry) {
                tracker.initialState = parseGraph(entry.document.baselineGraphJson ?? entry.document.graphJson);
                active.isModified = entry.document.dirty;
                const canvas = this.#app.canvas;
                if (isRecord(canvas) && isRecord(canvas.ds)) {
                    canvas.ds.scale = entry.document.view.scale;
                    canvas.ds.offset = [entry.document.view.x, entry.document.view.y];
                }
            }
        }
        if (!this.options.adapter.refresh())
            throw new Error('Comfy restored workflow observation failed.');
        const contexts = new Map();
        for (const [entry, workflow] of restored) {
            const id = this.options.adapter.identifyHostWorkflow(workflow);
            const context = id
                ? this.options.sessions.read().open.find((item) => item.sessionId === id)
                : null;
            if (!context)
                throw new Error('Comfy restored occurrence was not committed.');
            contexts.set(entry, context);
        }
        return contexts;
    }
    /** Admit one owned public load and refuse any competing load before more restore effects. */
    async #load(graph, workflow, current) {
        const before = this.options.adapter.readLoadSequence();
        await invoke(this.#app, 'loadGraphData', graph, true, true, workflow);
        assertCurrent(current);
        const active = this.#workflow.activeWorkflow;
        if (this.options.adapter.readLoadSequence() !== before + 1 ||
            this.options.sessions.read().loading ||
            !isRecord(active) ||
            (active !== workflow && active.changeTracker !== workflow.changeTracker))
            throw new DesktopSessionRefusal('stale', 'Comfy desktop materialization was superseded by another graph load.');
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
