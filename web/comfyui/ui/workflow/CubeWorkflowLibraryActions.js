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
/** Coordinate explicit library operations for workflow-owned Cube definitions. */
import { parseCanonicalCubeId } from '../core/CubeId.js';
import { enrichWorkflowPayload } from '../graph/WorkflowPayloadBuilder.js';
import { isRecord } from '../types/common.js';
/** Keep workflow authority intact while invoking explicit machine-library use cases. */
export class CubeWorkflowLibraryActions {
    #api;
    #state;
    #host;
    #dialogs;
    #feedback;
    /** Bind API, workflow snapshot, approval, and feedback ports. */
    constructor(options) {
        this.#api = options.api;
        this.#state = options.state;
        this.#host = options.host;
        this.#dialogs = options.dialogs;
        this.#feedback = options.feedback;
    }
    /** Read the current non-persisted classification for menu and edit policy. */
    classification(instanceId) {
        return this.#state.read(instanceId);
    }
    /** Acknowledge that the embedded workflow copy remains authoritative. */
    keep(instanceId) {
        const classification = this.#requireClassification(instanceId);
        this.#feedback.push('info', 'Cube kept in workflow', `${classification.cubeId} remains embedded and runnable without a library match.`);
    }
    /** Capture exact embedded content without changing its workflow definition. */
    async capture(instanceId) {
        await this.#run('Unable to capture Cube', async () => {
            const classification = this.#requireClassification(instanceId, 'capture');
            const workflow = await this.#workflowSnapshot();
            const result = await this.#api.captureWorkflowCube(requestBody(workflow, classification), jsonRequest());
            requireSuccess(result, 'Cube capture');
            this.#state.begin(workflow);
            this.#feedback.push('success', 'Cube captured', `${classification.cubeId} is preserved unchanged and remains read-only.`);
        });
    }
    /** Synchronize a claimed home source only after immediate user approval. */
    async syncSource(instanceId) {
        await this.#run('Unable to synchronize Cube source', async () => {
            const classification = this.#requireClassification(instanceId, 'track_source');
            const approved = await this.#dialogs.confirm({
                title: 'Synchronize Cube source?',
                message: [
                    `SugarCubes will preflight and synchronize the source claimed by ${classification.cubeId}.`,
                    'The workflow-embedded definition will remain unchanged even if the source is missing or different.',
                ],
                confirmLabel: 'Synchronize source',
                confirmClassName: 'p-button-primary sugarcubes-confirm__confirm',
            });
            if (!approved)
                return;
            const workflow = await this.#workflowSnapshot();
            const result = await this.#api.syncWorkflowCubeSource(JSON.stringify({
                workflow,
                definition_id: classification.definitionId,
                expected_semantic_hash: classification.semanticHash,
                approved: true,
            }), jsonRequest());
            requireSuccess(result, 'Source synchronization');
            this.#state.begin(workflow);
            const status = readResultString(result.data, 'status') || 'unknown';
            this.#feedback.push(status === 'exact' ? 'success' : 'warn', 'Cube source synchronized', `Source comparison: ${status}. The workflow copy was not replaced.`);
        });
    }
    /** Fork exact embedded content into a separately writable personal Cube. */
    async forkToLocal(instanceId) {
        await this.#run('Unable to fork Cube', async () => {
            const classification = this.#requireClassification(instanceId, 'fork');
            const newCubeId = await this.#dialogs.promptText({
                title: 'Fork Cube to Local Cubes',
                message: [
                    'The current workflow copy remains unchanged. The fork receives a new writable identity and lineage.',
                ],
                label: 'New Cube ID',
                initialValue: defaultForkCubeId(classification.cubeId),
                confirmLabel: 'Create fork',
                validate: validateLocalCubeId,
            });
            if (!newCubeId)
                return;
            const workflow = await this.#workflowSnapshot();
            const result = await this.#api.forkWorkflowCube(JSON.stringify({
                workflow,
                definition_id: classification.definitionId,
                expected_semantic_hash: classification.semanticHash,
                new_cube_id: newCubeId,
                selected_instance_ids: [],
                destination: 'local',
            }), jsonRequest());
            requireSuccess(result, 'Cube fork');
            this.#feedback.push('success', 'Cube fork created', `${newCubeId} is writable in Local Cubes; the workflow source remains unchanged.`);
        });
    }
    /** Resolve one current classification and require the requested operation. */
    #requireClassification(instanceId, operation) {
        const classification = this.#state.read(instanceId);
        if (!classification)
            throw new Error('Cube library status is not available yet.');
        if (operation && !classification.permittedOperations.has(operation)) {
            throw new Error(`Cube library operation '${operation}' is not permitted.`);
        }
        return classification;
    }
    /** Capture one immutable, definition-complete workflow from Comfy's public seam. */
    async #workflowSnapshot() {
        const app = this.#host.getApp?.();
        const graph = isRecord(app?.graph) ? app.graph : null;
        const result = await app?.graphToPrompt?.();
        const record = isRecord(result) ? result : {};
        if (!isRecord(record.workflow))
            throw new Error('Current workflow payload is unavailable.');
        return enrichWorkflowPayload(record.workflow, graph);
    }
    /** Convert an expected action failure into one actionable SugarCubes diagnostic. */
    async #run(summary, operation) {
        try {
            await operation();
        }
        catch (error) {
            this.#feedback.push('error', summary, readErrorMessage(error));
        }
    }
}
/** Build the stale-state guarded request shared by exact-content operations. */
function requestBody(workflow, classification) {
    return JSON.stringify({
        workflow,
        definition_id: classification.definitionId,
        expected_semantic_hash: classification.semanticHash,
    });
}
/** Apply the one JSON request contract used by workflow-library routes. */
function jsonRequest() {
    return { headers: { 'Content-Type': 'application/json' } };
}
/** Reject unsuccessful or structured-error API responses. */
function requireSuccess(result, operation) {
    if (result.response.ok && !isRecord(result.data.error))
        return;
    const error = isRecord(result.data.error) ? result.data.error : {};
    const message = readResultString(error, 'message') || `${operation} failed.`;
    throw new Error(message);
}
/** Suggest a collision-avoiding personal identity without changing the source identity. */
function defaultForkCubeId(cubeId) {
    const filename = cubeId
        .split('/')
        .at(-1)
        ?.replace(/\.cube$/i, '')
        .trim() || 'Forked Cube';
    return `local/personal/${filename} Fork.cube`;
}
/** Require the first Comfy action to produce a writable Local Cube identity. */
function validateLocalCubeId(value) {
    try {
        const parsed = parseCanonicalCubeId(value.trim());
        return parsed.sourceKind === 'local' ? null : 'Use a local namespace such as local/personal.';
    }
    catch (error) {
        return readErrorMessage(error);
    }
}
/** Read one normalized string from an untrusted response record. */
function readResultString(value, key) {
    const item = value[key];
    return typeof item === 'string' ? item.trim() : '';
}
/** Normalize thrown values for user-visible action feedback. */
function readErrorMessage(error) {
    return error instanceof Error && error.message ? error.message : String(error);
}
