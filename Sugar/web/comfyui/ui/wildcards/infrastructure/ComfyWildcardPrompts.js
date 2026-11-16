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
/** Resolve the wildcards of a run Comfy is about to queue. */
import { workflowWithPromptTexts } from '../../execution/application/WorkflowPromptTexts.js';
import { queuedPromptFields, } from '../../execution/infrastructure/ComfyWorkflowScenePrompts.js';
import { isRecord } from '../../types/common.js';
import { resolveWildcardTexts, } from '../application/WildcardPromptResolution.js';
/** Seed inputs, in the order a node is searched for its seed. */
const SEED_INPUTS = ['seed', 'noise_seed'];
/**
 * Give a queued run its prompt texts with every wildcard resolved.
 *
 * The texts are the multiline prompt fields of the live graph as the queued prompt carries them,
 * after scene expansion. The seed is the first seed of the queued prompt in workflow stack
 * order, which is also the global seed whenever the workflow overrides it. The workflow the
 * person authored keeps its wildcards; only what is sent to run is resolved.
 */
export class ComfyWildcardPrompts {
    app;
    library;
    entropy;
    /** Read the live app whose root graph constructed the prompt, and the wildcard library. */
    constructor(app, library, entropy = () => String(crypto.getRandomValues(new BigUint64Array(1))[0] ?? 0n)) {
        this.app = app;
        this.library = library;
        this.entropy = entropy;
    }
    /** Return the queued prompt with its prompt fields resolved, or the same prompt unchanged. */
    async prompt(payload) {
        if (!isRecord(payload) || !isRecord(payload.output))
            return payload;
        const graph = this.app()?.graph;
        if (!graph)
            return payload;
        const output = payload.output;
        const fields = queuedPromptFields(graph, output).fields;
        const resolved = await this.#resolve(graph, output);
        if (!resolved.size)
            return payload;
        const nodes = { ...output };
        for (const field of fields) {
            const text = field.source === null ? undefined : resolved.get(field.source);
            const queued = nodes[field.node];
            if (text === undefined || !isRecord(queued) || !isRecord(queued.inputs))
                continue;
            nodes[field.node] = { ...queued, inputs: { ...queued.inputs, [field.input]: text } };
        }
        return { ...payload, output: nodes };
    }
    /** Give a workflow sent to run directly the resolved text of each of its prompt fields. */
    async workflow(payload, workflow) {
        const graph = this.app()?.graph;
        if (!graph || !isRecord(payload) || !isRecord(payload.output))
            return workflow;
        return workflowWithPromptTexts(workflow, await this.#resolve(graph, payload.output));
    }
    /** Resolve the queued prompt's field texts with the run's seed. */
    async #resolve(graph, output) {
        const texts = queuedPromptFields(graph, output).fields.flatMap((field) => field.source === null ? [] : [field.source]);
        return await resolveWildcardTexts(texts, queuedPromptSeed(graph, output), this.entropy(), this.library);
    }
}
/**
 * Read the run's seed: the first whole-number seed input of the queued prompt, searching the
 * workflow's top-level nodes in stack order and each Cube's nodes in graph order.
 */
export function queuedPromptSeed(root, output) {
    const visit = (graph, prefix, seen) => {
        if (prefix.length > 32 || seen.has(graph))
            return null;
        for (const node of graph._nodes ?? graph.nodes ?? []) {
            if (node.id === undefined || node.id === null)
                continue;
            const segments = [...prefix, String(node.id)];
            const queued = output[segments.join(':')];
            const inputs = isRecord(queued) && isRecord(queued.inputs) ? queued.inputs : null;
            for (const name of inputs ? SEED_INPUTS : []) {
                const value = inputs?.[name];
                if (typeof value === 'number' && Number.isInteger(value))
                    return BigInt(value).toString();
            }
            const subgraph = node.subgraph;
            const nested = isRecord(subgraph)
                ? visit(subgraph, segments, new Set([...seen, graph]))
                : null;
            if (nested !== null)
                return nested;
        }
        return null;
    };
    return visit(root, [], new Set());
}
