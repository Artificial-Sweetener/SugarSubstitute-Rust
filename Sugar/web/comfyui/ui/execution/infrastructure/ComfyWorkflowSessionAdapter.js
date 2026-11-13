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
/** Translate public workflow-store objects into explicit in-memory session bindings. */
import { isRecord } from '../../types/common.js';
import { workflowSessionId } from '../domain/ExecutionIdentity.js';
/** Bind identities to host workflow objects and retire closed browser bindings explicitly. */
export class ComfyWorkflowSessionAdapter {
    options;
    #workflow;
    #identities = new WeakMap();
    #open = new Map();
    #loads = new Map();
    #contents = new WeakMap();
    #unsubscribe = null;
    #active = false;
    #lastLoadSequence = 0;
    /** Validate the host's public store without importing private Pinia implementations. */
    constructor(options) {
        this.options = options;
        const app = options.app;
        const manager = isRecord(app) && isRecord(app.extensionManager) ? app.extensionManager : null;
        if (!manager || !isRecord(manager.workflow))
            throw new TypeError('Comfy workflow store is unavailable.');
        this.#workflow = manager.workflow;
    }
    /** Observe synchronous store commitment and explicit open/close transitions. */
    install() {
        if (this.#active)
            return;
        const subscribe = this.#workflow.$subscribe;
        if (typeof subscribe !== 'function')
            throw new TypeError('Comfy workflow subscription is unavailable.');
        this.#active = true;
        try {
            const unsubscribe = subscribe.call(this.#workflow, () => this.refresh(), {
                detached: true,
                flush: 'sync',
            });
            if (typeof unsubscribe !== 'function')
                throw new TypeError('Comfy workflow unsubscribe is unavailable.');
            this.#unsubscribe = () => {
                unsubscribe();
            };
            this.refresh();
        }
        catch (error) {
            this.#active = false;
            this.#unsubscribe?.();
            this.#unsubscribe = null;
            throw error;
        }
    }
    /** Resynchronize after a fully observed graph load without inferring ownership from graph IDs. */
    refresh() {
        if (!this.#active)
            return false;
        try {
            const workflows = this.#workflow.openWorkflows;
            if (!Array.isArray(workflows) || !workflows.every(isRecord))
                throw new TypeError('Comfy open workflows are malformed.');
            const bindings = [];
            const nextOpen = new Map();
            let selected = null;
            for (const workflow of workflows) {
                let sessionId = this.#identities.get(workflow);
                if (!sessionId) {
                    sessionId = workflowSessionId(this.options.nextKey());
                    this.#identities.set(workflow, sessionId);
                }
                nextOpen.set(sessionId, workflow);
                bindings.push({
                    sessionId,
                    title: text(workflow.filename) || text(workflow.title) || 'Untitled workflow',
                    path: text(workflow.path),
                });
                if (workflow === this.#workflow.activeWorkflow)
                    selected = sessionId;
            }
            const previousOpen = new Map(this.#open);
            this.#open.clear();
            for (const [id, workflow] of nextOpen)
                this.#open.set(id, workflow);
            const closed = this.options.sessions.synchronize(bindings, selected);
            for (const sessionId of closed) {
                const previous = previousOpen.get(sessionId);
                if (previous)
                    this.#identities.delete(previous);
            }
            return true;
        }
        catch (error) {
            this.options.sessions.suspend();
            this.options.logger.error('SugarCubes workflow session observation failed.', { error });
            return false;
        }
    }
    /** Distinguish native history, exact stored-state activation, and actual graph replacement. */
    loadKind(args) {
        if (this.#isHistoryRestore(args))
            return 'history';
        const target = args[3];
        const tracker = isRecord(target) && isRecord(target.changeTracker) ? target.changeTracker : null;
        if (args[1] === true &&
            args[2] === true &&
            tracker &&
            isRecord(args[0]) &&
            args[0] === tracker.activeState &&
            [...this.#open.values()].some((workflow) => workflow.changeTracker === tracker))
            return 'activate';
        return 'replace';
    }
    /** Qualify history against the active workflow's restoring tracker rather than graph identity. */
    #isHistoryRestore(args) {
        const target = args[3];
        const active = this.#workflow.activeWorkflow;
        if (args[1] !== false || args[2] !== false || !isRecord(target) || !isRecord(active))
            return false;
        const tracker = active.changeTracker;
        // Comfy passes the raw workflow while Pinia exposes its proxy; the tracker is markRaw.
        return (isRecord(tracker) && target.changeTracker === tracker && tracker._restoringState === true);
    }
    /** Read only the named workflow's host-owned output snapshot for validated restoration. */
    readStoredOutputs(sessionId) {
        const workflow = this.#open.get(sessionId);
        const tracker = workflow && isRecord(workflow.changeTracker) ? workflow.changeTracker : null;
        return tracker?.nodeOutputs;
    }
    /** Read the same named workflow's serialized topology for validating restoration source membership. */
    readStoredWorkflow(sessionId) {
        const workflow = this.#open.get(sessionId);
        const tracker = workflow && isRecord(workflow.changeTracker) ? workflow.changeTracker : null;
        return tracker?.activeState;
    }
    /** Expose one explicitly bound workflow only to the adjacent supported-host document adapter. */
    readHostWorkflow(sessionId) {
        return this.#open.get(sessionId) ?? null;
    }
    /** Resolve the identity already assigned by this owner after a public materialization operation. */
    identifyHostWorkflow(workflow) {
        if (!isRecord(workflow))
            return null;
        const direct = this.#identities.get(workflow);
        if (direct)
            return direct;
        return [...this.#open].find(([, candidate]) => sameWorkflow(candidate, workflow))?.[0] ?? null;
    }
    /** Expose the observed public-load frontier so materialization rejects competing or nested loads. */
    readLoadSequence() {
        return this.#lastLoadSequence;
    }
    /**
     * Record the workflow a starting graph load configures: the host's target workflow object, or
     * none when the load creates a new workflow (an opened file) that the host commits only after
     * configuration. Until the load settles, the live graph belongs to that target, not to the
     * workflow the store still reports as active.
     */
    beginLoad(sequence, args) {
        this.#lastLoadSequence = sequence;
        const target = args[3];
        this.#loads.set(sequence, isRecord(target) ? target : null);
    }
    /** Forget a settled load; the store's committed active workflow owns the graph again. */
    endLoad(sequence) {
        this.#loads.delete(sequence);
    }
    /**
     * Read the stored serialized state of every open workflow other than the one owning the live
     * graph, including restored workflows the host has not loaded yet, whose serialized content is
     * all they hold.
     */
    readPeerWorkflowStates() {
        const owner = this.#graphOwner();
        return [...this.#open.values()].flatMap((workflow) => {
            if (owner && sameWorkflow(workflow, owner))
                return [];
            const state = this.#storedState(workflow);
            return state === undefined ? [] : [state];
        });
    }
    /**
     * Ask the live graph's owning workflow tracker to record the graph's current serialized state.
     * Nothing is recorded while a load configures a workflow the host has not created yet, so the
     * previously active workflow never records another workflow's graph.
     */
    commitLoadedWorkflowState() {
        const owner = this.#graphOwner();
        const tracker = owner && isRecord(owner.changeTracker) ? owner.changeTracker : null;
        const check = tracker?.checkState;
        if (typeof check === 'function')
            check.call(tracker);
    }
    /** The workflow owning the live graph: the latest pending load's target, else the active one. */
    #graphOwner() {
        if (this.#loads.size > 0)
            return [...this.#loads.values()].at(-1) ?? null;
        const active = this.#workflow.activeWorkflow;
        return isRecord(active) ? active : null;
    }
    /** A loaded workflow's tracked state, or an unloaded workflow's parsed serialized content. */
    #storedState(workflow) {
        if (isRecord(workflow.changeTracker))
            return workflow.changeTracker.activeState;
        const content = workflow.content;
        if (typeof content !== 'string')
            return undefined;
        const cached = this.#contents.get(workflow);
        if (cached?.content === content)
            return cached.state;
        let state;
        try {
            state = JSON.parse(content);
        }
        catch (error) {
            this.options.logger.error('SugarCubes could not read an unloaded workflow.', {
                path: text(workflow.path),
                error,
            });
            state = undefined;
        }
        this.#contents.set(workflow, { content, state });
        return state;
    }
    /** Stop this adapter's subscription while leaving session/resource teardown to their owners. */
    dispose() {
        if (!this.#active)
            return;
        this.#active = false;
        this.#unsubscribe?.();
        this.#unsubscribe = null;
        this.#open.clear();
        this.#loads.clear();
    }
}
/**
 * Match one host workflow across Comfy's raw object and the store's reactive proxy: by object,
 * by its unproxied change tracker, or by its unique open path.
 */
function sameWorkflow(open, other) {
    if (open === other)
        return true;
    if (isRecord(open.changeTracker) && open.changeTracker === other.changeTracker)
        return true;
    const path = text(open.path);
    return path !== '' && path === text(other.path);
}
/** Normalize display metadata without using it as an identity or association key. */
function text(value) {
    return typeof value === 'string' ? value.trim() : '';
}
