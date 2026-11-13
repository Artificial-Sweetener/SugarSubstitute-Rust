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

/** Build one Cube workflow whose classification inputs are its definition and instance. */
function cubeWorkflow(instanceId: string, definitionVersion = '1.0.0', x = 0) {
  return {
    nodes: [
      {
        id: 1,
        type: 'definition',
        pos: [x, 0],
        properties: { sugarcubes_kind: 'cube', sugarcubes_cube: { instance_id: instanceId } },
      },
    ],
    definitions: { subgraphs: [{ id: 'definition', extra: { version: definitionVersion } }] },
    extra: { ds: { offset: [x, x] } },
  };
}

/** Answer every request with one classification under a controllable catalog revision. */
function revisionedApi(revision: { current: string }) {
  return jest.fn(async (body: BodyInit | null) => {
    const parsed: unknown = JSON.parse(String(body));
    const node =
      typeof parsed === 'object' && parsed !== null && 'workflow' in parsed
        ? JSON.stringify(parsed.workflow)
        : '';
    const instanceId = /"instance_id":"([^"]+)"/.exec(node)?.[1] ?? 'unknown';
    return {
      response: { ok: true, status: 200 },
      data: {
        ...classificationResponse(instanceId, 'captured'),
        catalog_revision: revision.current,
      },
    };
  });
}

test('issues no classification request when switching between unchanged workflows', async () => {
  const revision = { current: 'sha256:one' };
  const classifyWorkflow = revisionedApi(revision);
  const state = new CubeWorkflowLibraryState({ classifyWorkflow }, console);
  for (let cycle = 0; cycle < 10; cycle++) {
    state.begin(cubeWorkflow('first', '1.0.0', cycle));
    await flushPromises();
    expect(state.read('first')).not.toBeNull();
    state.begin(cubeWorkflow('second', '1.0.0', cycle * 2));
    await flushPromises();
    expect(state.read('second')).not.toBeNull();
    expect(state.read('first')).toBeNull();
  }
  expect(classifyWorkflow).toHaveBeenCalledTimes(2);
});

test('reclassifies changed definitions, new catalog revisions and local catalog mutations', async () => {
  const revision = { current: 'sha256:one' };
  const classifyWorkflow = revisionedApi(revision);
  const state = new CubeWorkflowLibraryState({ classifyWorkflow }, console);
  state.begin(cubeWorkflow('first'));
  await flushPromises();
  state.begin(cubeWorkflow('first', '2.0.0'));
  await flushPromises();
  expect(classifyWorkflow).toHaveBeenCalledTimes(2);

  revision.current = 'sha256:two';
  state.begin(cubeWorkflow('other'));
  await flushPromises();
  state.begin(cubeWorkflow('first'));
  await flushPromises();
  expect(classifyWorkflow).toHaveBeenCalledTimes(4);

  state.invalidateCatalog();
  state.begin(cubeWorkflow('first'));
  await flushPromises();
  expect(classifyWorkflow).toHaveBeenCalledTimes(5);
  state.begin(cubeWorkflow('first'));
  await flushPromises();
  expect(classifyWorkflow).toHaveBeenCalledTimes(5);
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
