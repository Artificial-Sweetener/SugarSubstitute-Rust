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
/** Persist a workflow's input canvas identity and remembered selection in the workflow itself. */
import { isRecord } from '../../types/common.js';
/** Workflow `extra` key; the value is `{version: 1, id, route, surface, mask}`. */
export const WORKFLOW_INPUT_STATE_KEY = 'sugarcubes_input_canvas';
/** Identity shared by workflows that never had input surfaces; it never holds documents. */
const EMPTY_WORKFLOW = 'workflow-none';
/**
 * Give each workflow a stable identity for the canvas's input documents and remember its route.
 *
 * The record lives in the root graph's `extra`, like other per-workflow SugarCubes UI state, so
 * it travels with the workflow across reloads. It is written outside native undo history:
 * changing the visible route is navigation, not an authored edit.
 *
 * A duplicated workflow carries its original's record. When another open workflow already holds
 * the same identity, the loaded one is the copy: it takes a fresh identity, keeping its route and
 * selection, and the change is recorded in its change tracker so it persists like any edit.
 */
export class ComfyWorkflowInputState {
    root;
    nextId;
    peers;
    commit;
    /** Read the root workflow graph lazily so each loaded workflow keeps its own record. */
    constructor(root, nextId, peers, commit) {
        this.root = root;
        this.nextId = nextId;
        this.peers = peers;
        this.commit = commit;
    }
    /**
     * Read the workflow's identity. A workflow with nothing to provision is not modified and shares
     * one empty identity; `create` persists a new identity when it has surfaces. A new identity
     * opens on Input, because it is created for a workflow whose input images exist.
     */
    workflow(create) {
        const root = this.root();
        if (!root)
            return null;
        const stored = readComfyWorkflowInputState(root);
        if (stored && !this.peers().some((peer) => storedId(peer) === stored.id))
            return stored.id;
        if (stored) {
            const id = 'workflow-' + this.nextId();
            this.#write(root, { ...stored, id });
            this.commit();
            return id;
        }
        if (!create)
            return EMPTY_WORKFLOW;
        const id = 'workflow-' + this.nextId();
        this.#write(root, { version: 1, id, route: 'input', surface: null, mask: null });
        return id;
    }
    /**
     * Read the loaded workflow's identity without assigning or replacing one: its recorded
     * identity, or the shared empty identity while it has none.
     */
    loaded() {
        const root = this.root();
        return root ? (readComfyWorkflowInputState(root)?.id ?? EMPTY_WORKFLOW) : null;
    }
    /** Read the remembered selection when it belongs to the requested workflow. */
    read(workflow) {
        const root = this.root();
        const stored = root ? readComfyWorkflowInputState(root) : null;
        return stored?.id === workflow
            ? { route: stored.route, surface: stored.surface, mask: stored.mask }
            : null;
    }
    /** Remember a selection for the workflow that is still loaded. */
    write(workflow, selection) {
        const root = this.root();
        if (!root || readComfyWorkflowInputState(root)?.id !== workflow)
            return;
        this.#write(root, { version: 1, id: workflow, ...selection });
    }
    /** Replace only this record, keeping every other `extra` entry. */
    #write(root, state) {
        root.extra = { ...(isRecord(root.extra) ? root.extra : {}), [WORKFLOW_INPUT_STATE_KEY]: state };
    }
}
/** Read a live or inactive serialized workflow's input selection through its authoritative format owner. */
export function readComfyWorkflowInputState(root) {
    const extra = isRecord(root) && isRecord(root.extra) ? root.extra : null;
    const value = extra?.[WORKFLOW_INPUT_STATE_KEY];
    if (!isRecord(value) ||
        value.version !== 1 ||
        typeof value.id !== 'string' ||
        !/^workflow-[\w-]{1,100}$/.test(value.id) ||
        (value.route !== 'input' && value.route !== 'output') ||
        !optionalKey(value.surface) ||
        !optionalKey(value.mask))
        return null;
    return Object.freeze({
        version: 1,
        id: value.id,
        route: value.route,
        surface: value.surface,
        mask: value.mask,
    });
}
/** Read the identity recorded in another workflow's serialized state, or null. */
function storedId(state) {
    const extra = isRecord(state) && isRecord(state.extra) ? state.extra : null;
    const record = extra ? extra[WORKFLOW_INPUT_STATE_KEY] : null;
    return isRecord(record) && typeof record.id === 'string' ? record.id : null;
}
/** Admit a surface or mask key of the wire's maximum length, or null. */
function optionalKey(value) {
    return value === null || (typeof value === 'string' && value.length > 0 && value.length <= 256);
}
