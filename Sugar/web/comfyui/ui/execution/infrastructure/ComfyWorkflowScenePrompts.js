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
/** Queue one Comfy prompt per authored prompt scene through the existing submission ownership. */
import { isRecord } from '../../types/common.js';
import { cubeFacePromptRole } from '../../surface/CubeFacePromptPolicy.js';
import { planWorkflowScenes, sceneWorkflow, } from '../application/WorkflowScenePlan.js';
/**
 * Mark a scene prompt with its scene texts. Preparation steps copy prompts by spreading them,
 * which keeps this enumerable symbol, and Comfy serializes only named fields, never symbols.
 */
const SCENE_TEXTS = Symbol('sugarcubes.sceneTexts');
/**
 * Expand a constructed prompt into its scene prompts, each owned by the original's submission.
 *
 * Fields are the multiline prompt texts of the live graph, including those inside Cubes, read at
 * the execution identities Comfy flattened them to; a field's role follows its conditioning to
 * a sampler's positive or negative input. Expanded prompts are never expanded again. A refused
 * expansion is reported to the user and rethrown so Comfy does not queue the prompt.
 */
export class ComfyWorkflowScenePrompts {
    app;
    contexts;
    notice;
    nextId;
    #only = null;
    /** Read the live app whose root graph constructs the prompts being queued. */
    constructor(app, contexts, notice, nextId = () => crypto.randomUUID()) {
        this.app = app;
        this.contexts = contexts;
        this.notice = notice;
        this.nextId = nextId;
    }
    /** Queue only one scene of its scene run through Comfy's own queue. */
    async queueScene(scene) {
        const app = this.app();
        if (typeof app?.queuePrompt !== 'function')
            return 'unavailable';
        this.#only = scene;
        try {
            await app.queuePrompt(0, 1);
        }
        finally {
            this.#only = null;
        }
        return 'queued';
    }
    /** Give a serialized workflow queued for one scene prompt that scene's prompt texts. */
    workflow(payload, workflow) {
        const texts = isRecord(payload) ? payload[SCENE_TEXTS] : undefined;
        return texts ? sceneWorkflow(workflow, texts) : workflow;
    }
    /** Return the scene prompts to queue in place of this prompt, or null to queue it unchanged. */
    expand(payload) {
        const only = this.#only;
        this.#only = null;
        if (!isRecord(payload) || SCENE_TEXTS in payload || !isRecord(payload.output))
            return null;
        const graph = this.app()?.graph;
        if (!graph)
            return null;
        const { fields, stacks } = sceneFields(graph, payload.output);
        let prompts;
        try {
            prompts = planWorkflowScenes(fields, stacks, payload.output, this.nextId(), only);
        }
        catch (error) {
            const detail = error instanceof Error ? error.message : String(error);
            this.notice.pushToast('error', 'Workflow run not queued', detail);
            throw error;
        }
        if (!prompts)
            return null;
        return prompts.map((prompt) => {
            const copy = { ...payload, output: prompt.nodes, [SCENE_TEXTS]: prompt.texts };
            this.contexts.associateScene(copy, payload, prompt.member);
            return copy;
        });
    }
}
/** Collect every prompt editor field the queued prompt carries, in graph order. */
function sceneFields(root, output) {
    const fields = [];
    const stacks = [];
    const visit = (graph, prefix, seen) => {
        if (prefix.length > 32 || seen.has(graph))
            return;
        for (const node of graph._nodes ?? graph.nodes ?? []) {
            if (node.id === undefined || node.id === null)
                continue;
            const segments = [...prefix, String(node.id)];
            const stack = segments[0] ?? '';
            if (!prefix.length)
                stacks.push(stack);
            const field = promptField(node, segments.join(':'), stack, output);
            if (field)
                fields.push(field);
            const subgraph = node.subgraph;
            if (isRecord(subgraph))
                visit(subgraph, segments, new Set([...seen, graph]));
        }
    };
    visit(root, [], new Set());
    return { fields, stacks };
}
/** Describe one node's multiline prompt text field as the queued prompt carries it, or null. */
function promptField(node, executionId, stack, output) {
    const queued = output[executionId];
    if (!isRecord(queued) || !isRecord(queued.inputs))
        return null;
    const inputs = queued.inputs;
    const widget = (node.widgets ?? []).find((candidate) => (candidate.type === 'customtext' || candidate.options?.multiline === true) &&
        Object.hasOwn(inputs, candidate.name));
    if (!widget)
        return null;
    const value = inputs[widget.name];
    return {
        node: executionId,
        input: widget.name,
        stack,
        role: queuedPromptRole(executionId, output) ?? cubeFacePromptRole(node) ?? 'positive',
        source: typeof value === 'string' ? value : null,
    };
}
/**
 * Follow a prompt node's conditioning through the queued prompt to a sampler's positive or
 * negative input; null when it reaches neither or both.
 */
function queuedPromptRole(executionId, output) {
    const seen = new Set([executionId]);
    const queue = [executionId];
    let role = null;
    while (queue.length && seen.size <= 256) {
        const current = queue.shift() ?? '';
        for (const [id, node] of Object.entries(output)) {
            if (!isRecord(node) || !isRecord(node.inputs))
                continue;
            for (const [name, value] of Object.entries(node.inputs)) {
                if (!Array.isArray(value) || String(value[0]) !== current)
                    continue;
                const named = name.trim().toLowerCase();
                if (named === 'positive' || named === 'negative') {
                    if (role && role !== named)
                        return null;
                    role = named;
                }
                else if (!seen.has(id)) {
                    seen.add(id);
                    queue.push(id);
                }
            }
        }
    }
    return role;
}
