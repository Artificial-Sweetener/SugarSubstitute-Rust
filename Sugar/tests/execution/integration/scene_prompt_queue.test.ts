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
/** Prove one Queue click on a multi-scene prompt queues one owned Comfy prompt per scene. */

import { jest } from '@jest/globals';
import { ComfyPromptQueueBridge } from '../../../frontend/comfyui/ui/execution/infrastructure/ComfyPromptQueueBridge.js';
import { ComfyWorkflowScenePrompts } from '../../../frontend/comfyui/ui/execution/infrastructure/ComfyWorkflowScenePrompts.js';
import { WorkflowSubmissionContexts } from '../../../frontend/comfyui/ui/execution/application/WorkflowSubmissionContexts.js';
import { workflowSessionId } from '../../../frontend/comfyui/ui/execution/domain/ExecutionIdentity.js';
import type { ComfyGraph, ComfyNode } from '../../../frontend/comfyui/ui/types/graph.js';

/** A positive prompt node whose text authors two scenes. */
function graph(text: string): ComfyGraph {
  const node = {
    id: 6,
    type: 'CLIPTextEncode',
    title: 'Positive Prompt',
    widgets: [{ name: 'text', type: 'customtext', value: text, options: { multiline: true } }],
    outputs: [],
  } as unknown as ComfyNode;
  return { _nodes: [node] } as unknown as ComfyGraph;
}

/** Compose the expansion, the owned submission contexts and the queue bridge over a fake host. */
function fixture(text: string) {
  const contexts = new WorkflowSubmissionContexts();
  const notice = { pushToast: jest.fn() };
  // The app queues by constructing the payload and handing it to the API, as Comfy does.
  const app = {
    graph: graph(text),
    queuePrompt: async () => api.queuePrompt(0, payload),
  };
  const scenes = new ComfyWorkflowScenePrompts(
    () => app,
    contexts,
    notice,
    () => 'run-1',
  );
  const queued: unknown[] = [];
  const api = {
    queuePrompt: jest.fn(async (_position: number, payload: unknown) => {
      queued.push(payload);
      return { prompt_id: `p${String(queued.length)}` };
    }),
  };
  const bridge = new ComfyPromptQueueBridge({
    api,
    transform: async (payload) => ({ payload }),
    expand: (payload) => scenes.expand(payload),
  });
  bridge.install();
  const payload = {
    output: { '6': { class_type: 'CLIPTextEncode', inputs: { text } } },
    workflow: {},
  };
  contexts.associate(
    payload,
    { sessionId: workflowSessionId('w'), title: 'W', path: '', generation: 0 },
    [],
  );
  return { api, queued, contexts, payload, scenes, notice };
}

test('a prompt with two scenes queues two prompts, each owned by its scene', async () => {
  const f = fixture('a castle\n\n**Morning\nsunrise\n\n**Night\nstars');
  await expect(f.api.queuePrompt(0, f.payload)).resolves.toEqual({ prompt_id: 'p2' });
  expect(
    f.queued.map(
      (prompt) =>
        (prompt as { output: { '6': { inputs: { text: string } } } }).output['6'].inputs.text,
    ),
  ).toEqual(['a castle\n\nsunrise', 'a castle\n\nstars']);
  expect(f.queued.map((prompt) => f.contexts.read(prompt as object)?.scene?.key)).toEqual([
    'morning',
    'night',
  ]);
  expect(f.contexts.read(f.queued[0] as object)?.scene?.run.id).toBe('run-1');
});

test('queuing one chosen scene queues only that scene of the run', async () => {
  const f = fixture('a castle\n\n**Morning\nsunrise\n\n**Night\nstars');
  await expect(f.scenes.queueScene('night')).resolves.toBe('queued');
  expect(f.queued).toHaveLength(1);
  expect(f.contexts.read(f.queued[0] as object)?.scene?.run.scenes).toHaveLength(2);
});

test('a prompt without several scenes queues unchanged', async () => {
  const f = fixture('a castle');
  await f.api.queuePrompt(0, f.payload);
  expect(f.queued).toEqual([f.payload]);
});

test('prompts inside a Cube split by scene, with roles from the sampler inputs they reach', async () => {
  const contexts = new WorkflowSubmissionContexts();
  const field = (id: number, value: string) => ({
    id,
    type: 'CLIPTextEncode',
    title: 'CLIP Text Encode',
    widgets: [{ name: 'text', type: 'customtext', value }],
    outputs: [],
  });
  const positive = 'a castle\n\n**Morning\nsunrise\n\n**Night\nstars';
  const negative = 'blurry\n\n**Night\nnoise';
  const cube = {
    id: 73,
    type: 'cube',
    widgets: [{ name: 'text', type: 'customtext', value: positive }],
    subgraph: { _nodes: [field(72, positive), field(74, negative)] },
  };
  const scenes = new ComfyWorkflowScenePrompts(
    () => ({ graph: { _nodes: [cube] } as unknown as ComfyGraph }),
    contexts,
    { pushToast: jest.fn() },
    () => 'run-cube',
  );
  const payload = {
    output: {
      '73:72': { class_type: 'CLIPTextEncode', inputs: { text: positive } },
      '73:74': { class_type: 'CLIPTextEncode', inputs: { text: negative } },
      '3': {
        class_type: 'KSampler',
        inputs: { positive: ['73:72', 0], negative: ['73:74', 0] },
      },
    },
  };
  contexts.associate(
    payload,
    { sessionId: workflowSessionId('cube'), title: 'C', path: '', generation: 0 },
    [],
  );
  const prompts = scenes.expand(payload) as {
    output: Record<string, { inputs: { text: string } }>;
  }[];
  expect(prompts.map((prompt) => prompt.output['73:72']?.inputs.text)).toEqual([
    'a castle\n\nsunrise',
    'a castle\n\nstars',
  ]);
  expect(prompts.map((prompt) => prompt.output['73:74']?.inputs.text)).toEqual([
    'blurry',
    'blurry\n\nnoise',
  ]);
});

test('a scene prompt keeps its scene texts through preparation copies for direct execution', () => {
  const f = fixture('a castle\n\n**Morning\nsunrise\n\n**Night\nstars');
  const [, night] = f.scenes.expand(f.payload) ?? [];
  const prepared = { ...(night as object), extra_data: { saving: true } };
  expect(f.scenes.expand(prepared)).toBeNull();
  expect(
    f.scenes.workflow(prepared, {
      nodes: [{ id: 6, widgets_values: ['a castle\n\n**Morning\nsunrise\n\n**Night\nstars'] }],
    }),
  ).toEqual({ nodes: [{ id: 6, widgets_values: ['a castle\n\nstars'] }] });
  expect(JSON.stringify(prepared)).not.toContain('sceneTexts');
});

test('a refused scene queue tells the user why and queues nothing', async () => {
  const f = fixture('a castle\n\n**Morning\nsunrise\n\n**Morning\nagain\n\n**Night\nstars');
  await expect(f.api.queuePrompt(0, f.payload)).rejects.toThrow(/scene/);
  expect(f.queued).toHaveLength(0);
  expect(f.notice.pushToast).toHaveBeenCalledWith(
    'error',
    'Workflow run not queued',
    expect.stringContaining('scene'),
  );
});
