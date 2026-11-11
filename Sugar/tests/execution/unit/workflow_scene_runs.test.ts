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
/** Prove one queued prompt per prompt scene and per-scene results, as Substitute's scene runs. */

import { WorkflowResultStore } from '../../../frontend/comfyui/ui/execution/application/WorkflowResultStore.js';
import { WorkflowRunRegistry } from '../../../frontend/comfyui/ui/execution/application/WorkflowRunRegistry.js';
import {
  planWorkflowScenes,
  sceneWorkflow,
  type WorkflowScenePromptField,
} from '../../../frontend/comfyui/ui/execution/application/WorkflowScenePlan.js';
import {
  outputSourceId,
  promptId,
  workflowSessionId,
} from '../../../frontend/comfyui/ui/execution/domain/ExecutionIdentity.js';
import type { WorkflowSessionContext } from '../../../frontend/comfyui/ui/execution/domain/WorkflowSession.js';
import type { WorkflowRun } from '../../../frontend/comfyui/ui/execution/domain/WorkflowRun.js';
import type { WorkflowOutputSource } from '../../../frontend/comfyui/ui/execution/domain/WorkflowOutput.js';
import type { WorkflowSceneMember } from '../../../frontend/comfyui/ui/execution/domain/WorkflowScene.js';
import { decodeComfyMedia } from '../../../frontend/comfyui/ui/execution/infrastructure/ComfyMediaDecoder.js';

const POSITIVE = 'castle, oil painting\n\n**Morning\nsunrise light\n\n**Night\nmoonlit, stars';
const NEGATIVE = 'blurry\n\n**Night\noverexposed';

/** Describe the positive and negative prompt fields of one Cube as the queued prompt holds them. */
function fields(positive: string | null = POSITIVE): WorkflowScenePromptField[] {
  return [
    { node: '4:6', input: 'text', stack: '4', role: 'positive', source: positive },
    { node: '4:7', input: 'text', stack: '4', role: 'negative', source: NEGATIVE },
  ];
}

/** Build the queued node map for those fields. */
function nodes(positive: unknown = POSITIVE): Record<string, unknown> {
  return {
    '4:6': { class_type: 'CLIPTextEncode', inputs: { text: positive, clip: ['4:1', 1] } },
    '4:7': { class_type: 'CLIPTextEncode', inputs: { text: NEGATIVE, clip: ['4:1', 1] } },
    '4:9': { class_type: 'KSampler', inputs: { seed: 7 } },
  };
}

/** Read one queued node's prompt text. */
function text(prompt: { nodes: Readonly<Record<string, unknown>> }, node: string): unknown {
  return (prompt.nodes[node] as { inputs: { text: unknown } }).inputs.text;
}

test('a prompt with several scenes queues one prompt per scene with its universal text', () => {
  const prompts = planWorkflowScenes(fields(), ['2', '4'], nodes(), 'run-1');
  expect(prompts?.map((prompt) => prompt.member.key)).toEqual(['morning', 'night']);
  expect(prompts?.[0]?.member.run).toEqual({
    id: 'run-1',
    scenes: [
      { key: 'morning', title: 'Morning', order: 0 },
      { key: 'night', title: 'Night', order: 1 },
    ],
  });
  expect(prompts?.map((prompt) => text(prompt, '4:6'))).toEqual([
    'castle, oil painting\n\nsunrise light',
    'castle, oil painting\n\nmoonlit, stars',
  ]);
  expect(prompts?.map((prompt) => text(prompt, '4:7'))).toEqual([
    'blurry',
    'blurry\n\noverexposed',
  ]);
  expect(prompts?.[1]?.nodes['4:9']).toEqual({ class_type: 'KSampler', inputs: { seed: 7 } });
  expect(
    planWorkflowScenes(fields(), ['4'], nodes(), 'run-2', 'night')?.map((p) => p.member.key),
  ).toEqual(['night']);
});

test('one scene or none queues unchanged; linked fields keep their link', () => {
  expect(planWorkflowScenes(fields('castle\n\n**Only\nalone'), ['4'], nodes(), 'r')).toBeNull();
  expect(planWorkflowScenes(fields('castle'), ['4'], nodes(), 'r')).toBeNull();
  const linked = planWorkflowScenes(
    [fields()[0]!, { ...fields()[1]!, source: null }],
    ['4'],
    { ...nodes(), '4:7': { class_type: 'CLIPTextEncode', inputs: { text: ['3', 0] } } },
    'r',
  );
  expect(linked?.map((prompt) => text(prompt, '4:7'))).toEqual([
    ['3', 0],
    ['3', 0],
  ]);
});

test('scene titles unknown to the naming prompt refuse the queue', () => {
  const orphan = [fields()[0]!, { ...fields()[1]!, source: 'blurry\n\n**Dusk\nhaze' }];
  expect(() => planWorkflowScenes(orphan, ['4'], nodes(), 'r')).toThrow(/scene/);
});

const session: WorkflowSessionContext = {
  sessionId: workflowSessionId('scenes'),
  title: 'Scenes',
  path: 'scenes.json',
  generation: 0,
};
const source: WorkflowOutputSource = {
  id: outputSourceId('9'),
  executionPath: '9',
  label: 'Save Image',
  breadcrumb: [],
  order: 0,
};

/** Accept one queued prompt of a scene run. */
function accept(runs: WorkflowRunRegistry, id: string, scene?: WorkflowSceneMember): WorkflowRun {
  return runs.accept(
    runs.reserve({ session, sources: [source], ...(scene ? { scene } : {}) }),
    promptId(id),
  );
}

/** Retain one final image for a run. */
function retain(store: WorkflowResultStore, run: WorkflowRun, file: string): void {
  const decoded = decodeComfyMedia({ images: [{ filename: file }] });
  if (!decoded) throw new Error('Invalid test event');
  store.retain({ run, source, ...decoded, merge: false, delivery: file });
}

test('each scene of a scene run keeps its own outputs until a newer run replaces the set', () => {
  const store = new WorkflowResultStore();
  const runs = new WorkflowRunRegistry();
  store.synchronize([session]);
  const run = {
    id: 'scene-run',
    scenes: [
      { key: 'morning', title: 'Morning', order: 0 },
      { key: 'night', title: 'Night', order: 1 },
    ],
  };
  const morning = accept(runs, 'p1', { run, key: 'morning' });
  const night = accept(runs, 'p2', { run, key: 'night' });
  store.updateRun({ ...morning, status: 'running' });
  retain(store, morning, 'morning.png');
  expect(store.read(session.sessionId)?.scenes?.running).toBe('morning');
  store.updateRun({ ...night, status: 'running' });
  retain(store, night, 'night.png');
  const scenes = store.read(session.sessionId)?.scenes;
  expect(scenes?.id).toBe('scene-run');
  expect(scenes?.running).toBe('night');
  expect(
    scenes?.scenes.map((scene) => [scene.title, scene.outputs[0]?.items[0]?.file.filename]),
  ).toEqual([
    ['Morning', 'morning.png'],
    ['Night', 'night.png'],
  ]);
  expect(store.read(session.sessionId)?.outputs[0]?.items[0]?.file.filename).toBe('night.png');
  const single = accept(runs, 'p3');
  store.updateRun({ ...single, status: 'running' });
  expect(store.read(session.sessionId)?.scenes?.id).toBe('scene-run');
  retain(store, single, 'single.png');
  expect(store.read(session.sessionId)?.scenes).toBeNull();
});

test('a directly executed Cube workflow takes the scene text on its face and in its definition', () => {
  const night = planWorkflowScenes(fields(), ['4'], nodes(), 'r')?.[1];
  const workflow = {
    nodes: [
      {
        id: 4,
        type: 'cube',
        widgets_values: [POSITIVE, NEGATIVE, 7],
        sugarcubes_widget_values: { text: POSITIVE, seed: 7 },
      },
    ],
    definitions: {
      subgraphs: [{ id: 'cube', nodes: [{ id: 6, widgets_values: [POSITIVE] }] }],
    },
    extra: { note: POSITIVE },
  };
  const scene = sceneWorkflow(workflow, night?.texts ?? new Map()) as typeof workflow;
  expect(scene.nodes[0]?.widgets_values).toEqual([
    'castle, oil painting\n\nmoonlit, stars',
    'blurry\n\noverexposed',
    7,
  ]);
  expect(scene.definitions.subgraphs[0]?.nodes[0]?.widgets_values).toEqual([
    'castle, oil painting\n\nmoonlit, stars',
  ]);
  expect(scene.nodes[0]?.sugarcubes_widget_values).toEqual({
    text: 'castle, oil painting\n\nmoonlit, stars',
    seed: 7,
  });
  expect(scene.extra.note).toBe(POSITIVE);
  expect(workflow.nodes[0]?.widgets_values[0]).toBe(POSITIVE);
});
