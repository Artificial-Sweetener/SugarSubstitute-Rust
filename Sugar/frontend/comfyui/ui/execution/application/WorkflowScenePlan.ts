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
/** Plan one queued prompt per authored prompt scene, as Substitute's scene generation did. */
import { materializeScenePrompt, parseScenes } from '../../../../sugarprompt/core/src/index.js';
import { analyzeWorkflowScenes } from '../../../../sugarprompt/services/src/index.js';
import { isRecord } from '../../types/common.js';
import type {
  WorkflowScene,
  WorkflowSceneMember,
  WorkflowSceneRun,
} from '../domain/WorkflowScene.js';

/** One prompt field of the workflow as the queued prompt carries it. */
export interface WorkflowScenePromptField {
  /** Execution identity of the node that owns the field in the queued prompt. */
  readonly node: string;
  readonly input: string;
  /** The Cube or top-level node that holds the field; stack order ranks these. */
  readonly stack: string;
  readonly role: 'positive' | 'negative';
  /** The field's text, or null when the queued prompt links it from another node. */
  readonly source: string | null;
}

/** The queued-prompt node map: execution identity to `{ class_type, inputs }`. */
export type WorkflowPromptNodes = Readonly<Record<string, unknown>>;

/** One scene's prompt nodes with every scene-bearing field materialized for that scene. */
export interface WorkflowScenePrompt {
  readonly member: WorkflowSceneMember;
  readonly nodes: WorkflowPromptNodes;
  /** Each scene-bearing field text, as authored, mapped to its text for this scene. */
  readonly texts: ReadonlyMap<string, string>;
}

/**
 * Split a prompt whose fields author more than one scene into one prompt per scene.
 *
 * The first positive field in stack order names the scenes; every text field becomes its universal
 * text joined with its block for the scene. Linked fields keep their link. A workflow with one
 * scene or none queues unchanged and yields null. Fields whose scene titles are duplicated or are
 * unknown to the naming field refuse the queue rather than silently dropping text. `only` queues a
 * single scene of the run.
 */
export function planWorkflowScenes(
  fields: readonly WorkflowScenePromptField[],
  stackOrder: readonly string[],
  nodes: WorkflowPromptNodes,
  runId: string,
  only: string | null = null,
): readonly WorkflowScenePrompt[] | null {
  const stacks = new Set(fields.map((field) => field.stack));
  const analysis = analyzeWorkflowScenes({
    revision: 'queue',
    stackOrder: [...stackOrder.filter((stack) => stacks.has(stack))],
    prompts: fields.map((field) => ({
      id: JSON.stringify([field.node, field.input]),
      stackId: field.stack,
      role: field.role,
      linkable: field.source !== null,
      source: field.source,
    })),
  });
  if (analysis.scenes.length <= 1 && only === null) return null;
  if (analysis.generation.status !== 'ready')
    throw new Error(
      'Prompt scenes cannot be queued: every scene title must be unique and named by the first positive prompt.',
    );
  const run: WorkflowSceneRun = Object.freeze({
    id: runId,
    scenes: Object.freeze(
      analysis.scenes.map(
        (scene): WorkflowScene =>
          Object.freeze({ key: scene.key, title: scene.title, order: scene.order }),
      ),
    ),
  });
  const documents = fields
    .filter((field) => field.source !== null)
    .map((field) => ({ field, document: parseScenes(field.source ?? '') }));
  const scenes = only === null ? run.scenes : run.scenes.filter((scene) => scene.key === only);
  if (!scenes.length) throw new Error('The chosen prompt scene is no longer in the workflow.');
  return Object.freeze(
    scenes.map((scene) => {
      const copy: Record<string, unknown> = { ...nodes };
      const texts = new Map<string, string>();
      for (const { field, document } of documents) {
        const node = copy[field.node];
        if (!isRecord(node) || !isRecord(node.inputs)) continue;
        const block = document.scenes.find(
          (candidate) =>
            candidate.marker.normalizedKey === scene.key && !candidate.marker.duplicate,
        );
        const text = materializeScenePrompt(document.universalText, block?.text ?? '');
        texts.set(field.source ?? '', text);
        copy[field.node] = { ...node, inputs: { ...node.inputs, [field.input]: text } };
      }
      return Object.freeze({
        member: Object.freeze({ run, key: scene.key }),
        nodes: Object.freeze(copy),
        texts,
      });
    }),
  );
}

/**
 * Give a serialized workflow one scene's prompt texts, wherever its nodes hold them.
 *
 * Serialized Cubes hold a prompt on the Cube's face and in its definition, both as ordered widget
 * values and as SugarCubes' named widget values, so every such value equal to an authored
 * scene-bearing text becomes that text's scene materialization; a
 * materialization depends only on the text, so equal texts always agree.
 */
export function sceneWorkflow(workflow: unknown, texts: ReadonlyMap<string, string>): unknown {
  const scene = (item: unknown): unknown =>
    typeof item === 'string' ? (texts.get(item) ?? item) : item;
  const rewrite = (value: unknown, depth: number): unknown => {
    if (depth > 64) return value;
    if (Array.isArray(value)) return value.map((entry) => rewrite(entry, depth + 1));
    if (!isRecord(value)) return value;
    const copy: Record<string, unknown> = {};
    for (const [key, entry] of Object.entries(value))
      copy[key] =
        key === 'widgets_values' && Array.isArray(entry)
          ? entry.map(scene)
          : key === 'sugarcubes_widget_values' && isRecord(entry)
            ? Object.fromEntries(Object.entries(entry).map(([name, item]) => [name, scene(item)]))
            : rewrite(entry, depth + 1);
    return copy;
  };
  return texts.size ? rewrite(workflow, 0) : workflow;
}
