//    SugarCubes - composable workflow units for ComfyUI
//    Copyright (C) 2026  Artificial Sweetener and contributors
//
//    This program is free software: you can redistribute it and/or modify
//    it under the terms of the GNU Affero General Public License as published by
//    the Free Software Foundation, either version 3 of the License, or
//    (at your option) any later version.
/** Verify workflow classification remains non-persisted, current, and typed. */

import { jest } from '@jest/globals';
import { CubeWorkflowLibraryState } from '../../../frontend/comfyui/ui/workflow/CubeWorkflowLibraryState.js';

test('indexes the latest classification by stable instance without mutating workflow', async () => {
  const workflow = { nodes: [{ id: 1 }], definitions: { subgraphs: [] } };
  const before = JSON.stringify(workflow);
  const classifyWorkflow = jest.fn(async () => ({
    response: { ok: true, status: 200 },
    data: classificationResponse('cube-instance', 'none'),
  }));
  const listener = jest.fn();
  const state = new CubeWorkflowLibraryState({ classifyWorkflow }, console);
  state.subscribe(listener);

  state.begin(workflow);
  await flushPromises();

  expect(classifyWorkflow).toHaveBeenCalledWith(JSON.stringify({ workflow }), {
    headers: { 'Content-Type': 'application/json' },
  });
  expect(state.read('cube-instance')).toMatchObject({
    primaryClass: 'none',
    access: 'read_only',
  });
  expect(state.canEdit('cube-instance')).toBe(false);
  expect(state.canEdit('missing-instance')).toBeNull();
  expect(JSON.stringify(workflow)).toBe(before);
  expect(listener).toHaveBeenCalled();
});

test('ignores a stale response after a newer workflow begins classification', async () => {
  const resolvers: Array<(value: ReturnType<typeof classificationResult>) => void> = [];
  const classifyWorkflow = jest.fn(
    () =>
      new Promise<ReturnType<typeof classificationResult>>((resolve) => {
        resolvers.push(resolve);
      }),
  );
  const state = new CubeWorkflowLibraryState({ classifyWorkflow }, console);

  state.begin({ workflow: 'first' });
  state.begin({ workflow: 'second' });
  resolvers[1]?.(classificationResult('second-instance'));
  await flushPromises();
  resolvers[0]?.(classificationResult('first-instance'));
  await flushPromises();

  expect(state.read('second-instance')).not.toBeNull();
  expect(state.read('first-instance')).toBeNull();
});

function classificationResult(instanceId: string) {
  return {
    response: { ok: true, status: 200 },
    data: classificationResponse(instanceId, 'captured'),
  };
}

function classificationResponse(instanceId: string, primaryClass: 'none' | 'captured') {
  return {
    schema_version: 1,
    definitions: [
      {
        definition_id: 'definition',
        cube_id: 'local/personal/Demo.cube',
        cube_version: '1.0.0',
        semantic_hash: 'a'.repeat(64),
        instance_ids: [instanceId],
        primary_class: primaryClass,
        access: 'read_only',
        source_available: false,
        permitted_operations: ['keep', 'fork'],
      },
    ],
  };
}

async function flushPromises(): Promise<void> {
  await Promise.resolve();
  await Promise.resolve();
}
