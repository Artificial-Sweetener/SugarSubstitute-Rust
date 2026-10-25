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
/** Prove async prompt construction and queueing preserve entry-time session ownership. */

import { jest } from '@jest/globals';
import { WorkflowSessionStore } from '../../../frontend/comfyui/ui/execution/application/WorkflowSessionStore.js';
import { WorkflowSubmissionContexts } from '../../../frontend/comfyui/ui/execution/application/WorkflowSubmissionContexts.js';
import { workflowSessionId } from '../../../frontend/comfyui/ui/execution/domain/ExecutionIdentity.js';
import { createComfyPromptConstructionObserver } from '../../../frontend/comfyui/ui/execution/infrastructure/ComfyPromptConstructionObserver.js';
import {
  ComfyPromptQueueBridge,
  type ComfyPromptQueueApi,
} from '../../../frontend/comfyui/ui/execution/infrastructure/ComfyPromptQueueBridge.js';
import { ComfyPromptSubmissionObserver } from '../../../frontend/comfyui/ui/execution/infrastructure/ComfyPromptSubmissionObserver.js';
import type { WorkflowSubmissionContext } from '../../../frontend/comfyui/ui/execution/domain/WorkflowSubmission.js';
import type { PromptId } from '../../../frontend/comfyui/ui/execution/domain/ExecutionIdentity.js';

const a = { sessionId: workflowSessionId('a'), title: 'identical', path: 'same.json' };
const b = { sessionId: workflowSessionId('b'), title: 'identical', path: 'same.json' };

/** Hold host stages without timers so each tab transition is reproducible. */
function deferred<T>() {
  let resolve: ((value: T) => void) | undefined;
  const promise = new Promise<T>((complete) => {
    resolve = complete;
  });
  return {
    promise,
    finish(value: T) {
      if (!resolve) throw new Error('Missing deferred completion');
      resolve(value);
    },
  };
}

test.each([false, true])(
  'keeps submitting ownership across construction, preparation and response (direct=%s)',
  async (direct) => {
    const sessions = new WorkflowSessionStore();
    sessions.synchronize([a, b], a.sessionId);
    const contexts = new WorkflowSubmissionContexts();
    const construction = deferred<object>();
    const transformation = deferred<unknown>();
    const response = deferred<unknown>();
    const original = {
      output: { '42:23': { class_type: 'PreviewImage', _meta: { title: '<literal>' } } },
    };
    const host = { graphToPrompt: async () => await construction.promise };
    const logger = { error: jest.fn(), warn: jest.fn() };
    const accepted = jest.fn<(context: WorkflowSubmissionContext, id: PromptId) => void>();
    const observer = createComfyPromptConstructionObserver(host, sessions, contexts, logger);
    observer.install();
    const pendingConstruction = host.graphToPrompt();
    sessions.synchronize([a, b], b.sessionId);
    construction.finish(original);
    expect(await pendingConstruction).toBe(original);
    const api: ComfyPromptQueueApi = { queuePrompt: async () => await response.promise };
    const bridge = new ComfyPromptQueueBridge({
      api,
      transform: async () => ({ payload: await transformation.promise }),
      ...(direct
        ? { executeDirect: async () => await response.promise, shouldExecuteDirect: () => true }
        : {}),
      observer: new ComfyPromptSubmissionObserver({
        contexts,
        logger,
        accepted,
        submitting: (context) => ({ ...context, order: 1 }),
      }),
    });
    bridge.install();
    const queued = api.queuePrompt?.(0, original);
    const load = sessions.beginLoad();
    sessions.completeLoad(load, true);
    transformation.finish(original);
    response.finish({ prompt_id: 'job-a' });
    expect(await queued).toEqual({ prompt_id: 'job-a' });
    expect(accepted).toHaveBeenCalledWith(
      expect.objectContaining({
        session: expect.objectContaining({ sessionId: a.sessionId, generation: 0 }),
        sources: [
          expect.objectContaining({
            executionPath: '42:23',
            label: '<literal>',
            breadcrumb: [],
          }),
        ],
      }),
      'prompt:job-a',
    );
    bridge.dispose();
    observer.dispose();
    contexts.dispose();
  },
);

test('leaves foreign raw objects and construction during loading unassigned', async () => {
  const sessions = new WorkflowSessionStore();
  sessions.synchronize([a], a.sessionId);
  sessions.beginLoad();
  const contexts = new WorkflowSubmissionContexts();
  const payload = { output: { 7: { class_type: 'PreviewImage' } } };
  const host = { graphToPrompt: async () => payload };
  const logger = { error: jest.fn(), warn: jest.fn() };
  const observer = createComfyPromptConstructionObserver(host, sessions, contexts, logger);
  observer.install();
  await host.graphToPrompt();
  expect(contexts.read(payload)).toBeNull();
  const accepted = jest.fn();
  const submission = new ComfyPromptSubmissionObserver({
    contexts,
    logger,
    accepted,
    submitting: (context) => ({ ...context, order: 1 }),
  });
  expect(submission.capture({ ...payload })).toBeNull();
  expect(submission.capture(payload)).toBeNull();
  observer.dispose();
});

test('allows repeated batch requests from an exact known object but never associates rejection or malformed acceptance', async () => {
  const sessions = new WorkflowSessionStore();
  sessions.synchronize([a], a.sessionId);
  const context = sessions.read().active;
  if (!context) throw new Error('Missing active context');
  const contexts = new WorkflowSubmissionContexts();
  const payload = { output: {} };
  contexts.associate(payload, context, []);
  const accepted = jest.fn<(context: WorkflowSubmissionContext, id: PromptId) => void>();
  const logger = { error: jest.fn(), warn: jest.fn() };
  const observer = new ComfyPromptSubmissionObserver({
    contexts,
    logger,
    accepted,
    submitting: (context) => ({ ...context, order: 1 }),
  });
  observer.capture(payload)?.accepted({ prompt_id: 'one' });
  observer.capture(payload)?.accepted({ prompt_id: 'two' });
  const rejected = observer.capture(payload);
  rejected?.rejected(new Error('Queue failed'));
  rejected?.accepted({ prompt_id: 'rejected' });
  observer.capture(payload)?.accepted({ prompt_id: 42 });
  expect(accepted.mock.calls.map((call) => call[1])).toEqual(['prompt:one', 'prompt:two']);
  expect(logger.warn).toHaveBeenCalledTimes(1);
});

test('uses the direct backend receipt execution snapshot including prompt-only cube marker sources', () => {
  const contexts = new WorkflowSubmissionContexts();
  const payload = { workflow: { nodes: [] }, output: {} };
  contexts.associate(payload, { ...a, generation: 0 }, []);
  const accepted = jest.fn<(context: WorkflowSubmissionContext, id: PromptId) => void>();
  const observer = new ComfyPromptSubmissionObserver({
    submitting: (context) => ({ ...context, order: 1 }),
    contexts,
    logger: { error: jest.fn(), warn: jest.fn() },
    accepted,
  });
  observer.capture(payload)?.accepted({
    prompt_id: 'direct',
    execution_prompt: {
      '__sugarcubes_cube_output__:7:0': {
        class_type: 'SugarCubes.CubeOutput',
        _meta: { title: 'Authored image binding' },
      },
      '7:23': { class_type: 'PreviewImage', _meta: { title: 'Internal preview' } },
    },
  });
  expect(accepted).toHaveBeenCalledWith(
    expect.objectContaining({
      sources: [
        expect.objectContaining({
          executionPath: '__sugarcubes_cube_output__:7:0',
          label: 'Authored image binding',
        }),
        expect.objectContaining({ executionPath: '7:23', label: 'Internal preview' }),
      ],
    }),
    'prompt:direct',
  );
});

test.each([false, true])(
  'resolves captured return metadata against the authoritative queue graph (direct=%s)',
  async (direct) => {
    const contexts = new WorkflowSubmissionContexts();
    const payload = { output: { '7': { class_type: 'PreviewImage' } } };
    const prepared = { output: { '8': { class_type: 'PreviewImage' } } };
    const canonical = { '9': { class_type: 'PreviewImage' } };
    contexts.associate(payload, { ...a, generation: 0 }, []);
    const accepted = jest.fn<(context: WorkflowSubmissionContext, id: PromptId) => void>();
    const resolve = jest.fn(() => [
      {
        kind: 'unavailable' as const,
        executionPath: direct ? '9' : '8',
        message: 'No registered placement',
      },
    ]);
    const logger = { error: jest.fn(), warn: jest.fn() };
    const api: ComfyPromptQueueApi = { queuePrompt: async () => ({ prompt_id: 'queued' }) };
    const bridge = new ComfyPromptQueueBridge({
      api,
      transform: async () => ({ payload: prepared, canvas: { resolve } }),
      ...(direct
        ? {
            executeDirect: async (_position, _payload, _options, _submit, preparedInput) => {
              preparedInput({ payload: prepared, canvas: { resolve } });
              return { prompt_id: 'queued', execution_prompt: canonical };
            },
          }
        : {}),
      observer: new ComfyPromptSubmissionObserver({
        contexts,
        logger,
        submitting: (context) => ({ ...context, order: 1 }),
        accepted,
      }),
    });
    bridge.install();
    await api.queuePrompt?.(0, payload);
    expect(resolve).toHaveBeenCalledWith(direct ? canonical : prepared.output);
    expect(accepted.mock.calls[0]?.[0].canvasReturns?.[0]).toMatchObject({
      executionPath: direct ? '9' : '8',
    });
    expect(logger.error).not.toHaveBeenCalled();
    bridge.dispose();
  },
);

test('keeps accepted job ownership when return attribution fails and refuses native return explicitly', () => {
  const contexts = new WorkflowSubmissionContexts(),
    payload = { output: { '7': { class_type: 'PreviewImage' } } };
  contexts.associate(payload, { ...a, generation: 0 }, []);
  const accepted = jest.fn<(context: WorkflowSubmissionContext, id: PromptId) => void>(),
    logger = { error: jest.fn(), warn: jest.fn() };
  const observer = new ComfyPromptSubmissionObserver({
    contexts,
    logger,
    submitting: (context) => ({ ...context, order: 1 }),
    accepted,
  });
  const observed = observer.capture(payload);
  observed?.prepared(payload, {
    resolve() {
      throw new Error('Malformed canonical graph');
    },
  });
  observed?.accepted({ prompt_id: 'queued' });
  expect(accepted).toHaveBeenCalledTimes(1);
  expect(accepted.mock.calls[0]?.[0].canvasReturns).toEqual([
    {
      kind: 'unavailable',
      executionPath: '7',
      message: 'The inpaint output has no verified captured placement.',
    },
  ]);
  expect(logger.error).toHaveBeenCalledWith(
    expect.any(String),
    expect.objectContaining({ error: expect.any(Error) }),
  );
});
