import { isRecord } from '../../types/common.js';
import { readCubeGraphAnalysis } from './CubeGraphResponse.js';
/** Retain topology authority on the backend while exposing typed browser operations. */
export class CubeGraphApi {
    #transport;
    /** Bind the shared Comfy transport rather than constructing another network client. */
    constructor(transport) {
        this.#transport = transport;
    }
    /** Analyze one exact snapshot without changing the live workflow. */
    analyze(snapshot) {
        return this.#request('analyze', snapshot);
    }
    /** Submit an exact segment permutation for canonical validation and geometry planning. */
    reorder(snapshot, segmentInstanceIds, orderedInstanceIds) {
        return this.#request('reorder', snapshot, {
            segment_instance_ids: segmentInstanceIds,
            ordered_instance_ids: orderedInstanceIds,
        });
    }
    /** Plan the complete cart without changing live host nodes before validation succeeds. */
    applyStack(snapshot, entries) {
        return this.#request('stack', snapshot, {
            entries: entries.map((entry) => entry.source === 'existing'
                ? { source: entry.source, instance_id: entry.instanceId }
                : {
                    source: entry.source,
                    instance_id: entry.instanceId,
                    alias: entry.alias,
                    document: entry.document,
                    size: entry.size,
                }),
        });
    }
    /** Bound transport lifetime and reject failures before exposing any graph facts. */
    async #request(operation, snapshot, fields = {}) {
        const workflow = JSON.parse(snapshot.serializedWorkflow);
        if (!isRecord(workflow) || Array.isArray(workflow))
            throw new TypeError('Cube graph snapshot must be an object.');
        const abort = new AbortController();
        const timeout = globalThis.setTimeout(() => abort.abort(), 15000);
        try {
            const result = await this.#transport.fetchJson(`/sugarcubes/v2/workflows/${operation}`, {
                method: 'POST',
                headers: { 'Content-Type': 'application/json' },
                signal: abort.signal,
                body: JSON.stringify({ schema_version: 1, workflow, ...fields }),
            });
            if (abort.signal.aborted)
                throw new Error(`Cube graph ${operation} timed out.`);
            if (result.response.status !== 200) {
                const error = isRecord(result.data.error) ? result.data.error : {};
                const message = typeof error.message === 'string' ? error.message.trim() : '';
                throw new Error(message ||
                    `Cube graph ${operation} failed (${String(result.response.status ?? 'unknown')}).`);
            }
            return readCubeGraphAnalysis(result.data);
        }
        finally {
            globalThis.clearTimeout(timeout);
        }
    }
}
