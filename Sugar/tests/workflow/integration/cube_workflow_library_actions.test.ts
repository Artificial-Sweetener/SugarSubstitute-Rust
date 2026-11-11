//    SugarCubes - composable workflow units for ComfyUI
//    Copyright (C) 2026  Artificial Sweetener and contributors
//
//    This program is free software: you can redistribute it and/or modify
//    it under the terms of the GNU Affero General Public License as published by
//    the Free Software Foundation, either version 3 of the License, or
//    (at your option) any later version.
/** Verify explicit workflow-library actions preserve embedded workflow authority. */

import { jest } from '@jest/globals';
import { CubeWorkflowLibraryActions } from '../../../frontend/comfyui/ui/workflow/CubeWorkflowLibraryActions.js';
import { CubeWorkflowLibraryState } from '../../../frontend/comfyui/ui/workflow/CubeWorkflowLibraryState.js';

test('Capture submits exact classified content and refreshes transient state', async () => {
  const workflow = workflowPayload();
  const classifyWorkflow = jest.fn(async () => classificationResult());
  const state = new CubeWorkflowLibraryState({ classifyWorkflow }, console);
  state.begin(workflow);
  await flushPromises();
  let capturedBody: BodyInit | null = null;
  let capturedOptions: RequestInit | undefined;
  const captureWorkflowCube = jest.fn(async (body: BodyInit | null, options?: RequestInit) => {
    capturedBody = body;
    capturedOptions = options;
    return successResult({ created: true });
  });
  const feedback = { push: jest.fn() };
  const actions = new CubeWorkflowLibraryActions({
    api: {
      captureWorkflowCube,
      forkWorkflowCube: jest.fn(async () => successResult()),
      syncWorkflowCubeSource: jest.fn(async () => successResult()),
    },
    state,
    host: { getApp: () => ({ graph: {}, graphToPrompt: async () => ({ workflow }) }) },
    dialogs: {
      confirm: jest.fn(async () => true),
      promptText: jest.fn(async () => null),
    },
    feedback,
  });

  await actions.capture('cube-instance');

  expect(JSON.parse(String(capturedBody))).toMatchObject({
    definition_id: 'definition',
    expected_semantic_hash: 'a'.repeat(64),
    workflow,
  });
  expect(capturedOptions).toEqual({ headers: { 'Content-Type': 'application/json' } });
  expect(classifyWorkflow).toHaveBeenCalledTimes(2);
  expect(feedback.push).toHaveBeenCalledWith(
    'success',
    'Cube captured',
    expect.stringContaining('remains read-only'),
  );
});

test('source synchronization performs no network action when approval is declined', async () => {
  const workflow = workflowPayload();
  const state = new CubeWorkflowLibraryState(
    { classifyWorkflow: jest.fn(async () => classificationResult()) },
    console,
  );
  state.begin(workflow);
  await flushPromises();
  const syncWorkflowCubeSource = jest.fn(async () => successResult());
  const actions = new CubeWorkflowLibraryActions({
    api: {
      captureWorkflowCube: jest.fn(async () => successResult()),
      forkWorkflowCube: jest.fn(async () => successResult()),
      syncWorkflowCubeSource,
    },
    state,
    host: { getApp: () => ({ graph: {}, graphToPrompt: async () => ({ workflow }) }) },
    dialogs: {
      confirm: jest.fn(async () => false),
      promptText: jest.fn(async () => null),
    },
    feedback: { push: jest.fn() },
  });

  await actions.syncSource('cube-instance');

  expect(syncWorkflowCubeSource).not.toHaveBeenCalled();
});

function workflowPayload() {
  return {
    nodes: [],
    definitions: { subgraphs: [{ id: 'definition' }] },
  };
}

function classificationResult() {
  return successResult({
    schema_version: 1,
    definitions: [
      {
        definition_id: 'definition',
        cube_id: 'Artificial-Sweetener/Base-Cubes/Text to Image.cube',
        cube_version: '1.0.0',
        semantic_hash: 'a'.repeat(64),
        instance_ids: ['cube-instance'],
        primary_class: 'none',
        access: 'read_only',
        source_available: true,
        permitted_operations: ['keep', 'capture', 'track_source', 'fork'],
      },
    ],
  });
}

function successResult(data: Record<string, unknown> = {}) {
  return { response: { ok: true, status: 200 }, data };
}

async function flushPromises(): Promise<void> {
  await Promise.resolve();
  await Promise.resolve();
}
