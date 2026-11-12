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
import type { ComfyGraph, ComfyNode } from '../../types/graph.js';

/** The live Comfy app: its graph constructs prompts and its queue runs one scene. */
export interface ComfySceneApp {
  readonly graph?: ComfyGraph | null;
  queuePrompt?(number: number, batchCount?: number): unknown;
}

/** The product's notification owner, which tells the user why a run was not queued. */
export interface SceneQueueNotice {
  pushToast(severity: 'error', summary: string, detail: string): void;
}
import { cubeFacePromptRole } from '../../surface/CubeFacePromptPolicy.js';
import type { WorkflowSubmissionContexts } from '../application/WorkflowSubmissionContexts.js';
import {
  planWorkflowScenes,
  sceneWorkflow,
  type WorkflowScenePromptField,
} from '../application/WorkflowScenePlan.js';

/**
 * Mark a scene prompt with its scene texts. Preparation steps copy prompts by spreading them,
 * which keeps this enumerable symbol, and Comfy serializes only named fields, never symbols.
 */
const SCENE_TEXTS = Symbol('sugarcubes.sceneTexts');
type ScenePrompt = Record<PropertyKey, unknown> & {
  [SCENE_TEXTS]?: ReadonlyMap<string, string>;
};

/**
 * Expand a constructed prompt into its scene prompts, each owned by the original's submission.
 *
 * Fields are the multiline prompt texts of the live graph, including those inside Cubes, read at
 * the execution identities Comfy flattened them to; a field's role follows its conditioning to
 * a sampler's positive or negative input. Expanded prompts are never expanded again. A refused
 * expansion is reported to the user and rethrown so Comfy does not queue the prompt.
 */
export class ComfyWorkflowScenePrompts {
  #only: string | null = null;

  /** Read the live app whose root graph constructs the prompts being queued. */
  constructor(
    private readonly app: () => ComfySceneApp | null | undefined,
    private readonly contexts: WorkflowSubmissionContexts,
    private readonly notice: SceneQueueNotice,
    private readonly nextId: () => string = () => crypto.randomUUID(),
  ) {}

  /** Queue only one scene of its scene run through Comfy's own queue. */
  async queueScene(scene: string): Promise<'queued' | 'unavailable'> {
    const app = this.app();
    if (typeof app?.queuePrompt !== 'function') return 'unavailable';
    this.#only = scene;
    try {
      await app.queuePrompt(0, 1);
    } finally {
      this.#only = null;
    }
    return 'queued';
  }

  /** Give a serialized workflow queued for one scene prompt that scene's prompt texts. */
  workflow(payload: unknown, workflow: unknown): unknown {
    const texts = isRecord(payload) ? (payload as ScenePrompt)[SCENE_TEXTS] : undefined;
    return texts ? sceneWorkflow(workflow, texts) : workflow;
  }

  /** Return the scene prompts to queue in place of this prompt, or null to queue it unchanged. */
  expand(payload: unknown): readonly object[] | null {
    const only = this.#only;
    this.#only = null;
    if (!isRecord(payload) || SCENE_TEXTS in payload || !isRecord(payload.output)) return null;
    const graph = this.app()?.graph;
    if (!graph) return null;
    const { fields, stacks } = sceneFields(graph, payload.output);
    let prompts: ReturnType<typeof planWorkflowScenes>;
    try {
      prompts = planWorkflowScenes(fields, stacks, payload.output, this.nextId(), only);
    } catch (error: unknown) {
      const detail = error instanceof Error ? error.message : String(error);
      this.notice.pushToast('error', 'Workflow run not queued', detail);
      throw error;
    }
    if (!prompts) return null;
    return prompts.map((prompt) => {
      const copy: ScenePrompt = { ...payload, output: prompt.nodes, [SCENE_TEXTS]: prompt.texts };
      this.contexts.associateScene(copy, payload, prompt.member);
      return copy;
    });
  }
}

/** Collect every prompt editor field the queued prompt carries, in graph order. */
function sceneFields(
  root: ComfyGraph,
  output: Readonly<Record<string, unknown>>,
): { fields: WorkflowScenePromptField[]; stacks: string[] } {
  const fields: WorkflowScenePromptField[] = [];
  const stacks: string[] = [];
  const visit = (
    graph: ComfyGraph,
    prefix: readonly string[],
    seen: ReadonlySet<ComfyGraph>,
  ): void => {
    if (prefix.length > 32 || seen.has(graph)) return;
    for (const node of graph._nodes ?? graph.nodes ?? []) {
      if (node.id === undefined || node.id === null) continue;
      const segments = [...prefix, String(node.id)];
      const stack = segments[0] ?? '';
      if (!prefix.length) stacks.push(stack);
      const field = promptField(node, segments.join(':'), stack, output);
      if (field) fields.push(field);
      const subgraph = (node as ComfyNode & { subgraph?: unknown }).subgraph;
      if (isRecord(subgraph)) visit(subgraph as ComfyGraph, segments, new Set([...seen, graph]));
    }
  };
  visit(root, [], new Set());
  return { fields, stacks };
}

/** Describe one node's multiline prompt text field as the queued prompt carries it, or null. */
function promptField(
  node: ComfyNode,
  executionId: string,
  stack: string,
  output: Readonly<Record<string, unknown>>,
): WorkflowScenePromptField | null {
  const queued = output[executionId];
  if (!isRecord(queued) || !isRecord(queued.inputs)) return null;
  const inputs = queued.inputs;
  const widget = (node.widgets ?? []).find(
    (candidate) =>
      (candidate.type === 'customtext' || candidate.options?.multiline === true) &&
      Object.hasOwn(inputs, candidate.name),
  );
  if (!widget) return null;
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
function queuedPromptRole(
  executionId: string,
  output: Readonly<Record<string, unknown>>,
): 'positive' | 'negative' | null {
  const seen = new Set<string>([executionId]);
  const queue = [executionId];
  let role: 'positive' | 'negative' | null = null;
  while (queue.length && seen.size <= 256) {
    const current = queue.shift() ?? '';
    for (const [id, node] of Object.entries(output)) {
      if (!isRecord(node) || !isRecord(node.inputs)) continue;
      for (const [name, value] of Object.entries(node.inputs)) {
        if (!Array.isArray(value) || String(value[0]) !== current) continue;
        const named = name.trim().toLowerCase();
        if (named === 'positive' || named === 'negative') {
          if (role && role !== named) return null;
          role = named;
        } else if (!seen.has(id)) {
          seen.add(id);
          queue.push(id);
        }
      }
    }
  }
  return role;
}
