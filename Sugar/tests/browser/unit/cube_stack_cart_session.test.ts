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
/** Prove full-cart staging remains isolated and delayed applications cannot mutate another workflow. */
import { expect, jest, test } from '@jest/globals';
import { CubeStackCartSession } from '../../../frontend/comfyui/ui/picker/application/CubeStackCartSession.js';
import type { CubeGraphAnalysis } from '../../../frontend/comfyui/ui/workflow/domain/CubeGraphModels.js';
import type { CubeGraphStackEntry } from '../../../frontend/comfyui/ui/workflow/domain/CubeGraphStackModels.js';

test('Reset restores the full opening stack and unchanged Apply creates no request or transaction', async () => {
  const f = fixture();
  f.session.remove('existing:first');
  f.session.add('artifact');
  expect(f.session.entries).toHaveLength(2);
  expect(f.commit).not.toHaveBeenCalled();
  f.session.reset();
  expect(f.session.entries.map((entry) => entry.draftId)).toEqual([
    'existing:first',
    'existing:second',
  ]);
  expect(await f.session.apply()).toBe(false);
  expect(f.gateway).not.toHaveBeenCalled();
});

test('repeated staged artifacts retain exact captured documents and independent instance identities', async () => {
  const f = fixture();
  f.session.add('artifact');
  f.session.add('artifact');
  expect(await f.session.apply()).toBe(true);
  const entries = f.gateway.mock.calls[0]![1];
  expect(entries.map((entry) => entry.instanceId)).toEqual(['first', 'second', 'new-1', 'new-2']);
  expect(entries.slice(2)).toEqual(
    ['new-1', 'new-2'].map((instanceId) => ({
      source: 'new',
      instanceId,
      catalogKey: 'artifact',
      cubeId: 'cube',
      version: '1',
      alias: 'Cube',
      document: f.document,
      size: [920, 600],
    })),
  );
  expect(f.commit).toHaveBeenCalledTimes(1);
  await expect(f.session.apply()).rejects.toThrow('closed');
});

test.each(['stale', 'closed'] as const)(
  'a %s cart cannot commit a delayed successful plan',
  async (reason) => {
    const f = fixture();
    let complete: ((result: CubeGraphAnalysis) => void) | undefined;
    f.gateway.mockImplementation(
      () =>
        new Promise((resolve) => {
          complete = resolve;
        }),
    );
    f.session.add('artifact');
    const applying = f.session.apply();
    expect(f.session.pending).toBe(true);
    expect(() => f.session.reset()).toThrow('in progress');
    await expect(f.session.apply()).rejects.toThrow('in progress');
    if (reason === 'stale') f.editWorkflow();
    else f.session.dispose();
    complete?.(analysis(['first', 'second', 'new-1']));
    await expect(applying).rejects.toThrow('Workflow changed');
    expect(f.commit).not.toHaveBeenCalled();
    expect(f.session.pending).toBe(false);
  },
);

test.each(['identity', 'order', 'placements', 'artifact'] as const)(
  'rejects an invalid %s response before native commit',
  async (kind) => {
    const f = fixture();
    f.session.add('artifact');
    const result = analysis(['first', 'second', 'new-1']);
    f.gateway.mockResolvedValue(
      kind === 'identity'
        ? {
            ...result,
            instances: result.instances.map((instance) => ({ ...instance, alias: 'changed' })),
          }
        : kind === 'order'
          ? {
              ...result,
              placements: result.placements.map((placement, index) => ({
                ...placement,
                position: [index === 0 ? 244 : index === 1 ? 0 : 488, 0],
              })),
            }
          : kind === 'artifact'
            ? {
                ...result,
                instances: result.instances.map((instance) =>
                  instance.instanceId === 'new-1' ? { ...instance, version: 'later' } : instance,
                ),
              }
            : { ...result, placements: result.placements.slice(1) },
    );
    await expect(f.session.apply()).rejects.toThrow('Cube cart response');
    expect(f.commit).not.toHaveBeenCalled();
    expect(f.session.pending).toBe(false);
    expect(f.session.changed).toBe(true);
  },
);

test('a planner rejection retains the edited cart for retry and reports the original failure', async () => {
  const f = fixture();
  f.session.add('artifact');
  f.gateway.mockRejectedValueOnce(new Error('Fixed by wiring'));
  await expect(f.session.apply()).rejects.toThrow('Fixed by wiring');
  expect(f.session.changed).toBe(true);
  expect(f.session.pending).toBe(false);
  expect(await f.session.apply()).toBe(true);
});

/** Provide immutable captured facts and inject only external planning and native commit boundaries. */
function fixture() {
  let revision = 'opening';
  let id = 0;
  const document = Object.freeze({
    cube_id: 'cube',
    version: '1',
    implementation: Object.freeze({ nodes: {} }),
  });
  const snapshot = { revision, serializedWorkflow: '{}' };
  const gateway = jest.fn(
    async (_snapshot: typeof snapshot, entries: readonly CubeGraphStackEntry[]) =>
      analysis(entries.map((entry) => entry.instanceId)),
  );
  const commit = jest.fn();
  const session = new CubeStackCartSession({
    snapshot,
    analysis: analysis(['first', 'second']),
    artifacts: [
      {
        catalogKey: 'artifact',
        cubeId: 'cube',
        version: '1',
        alias: 'Cube',
        document,
        size: [920, 600],
      },
    ],
    snapshots: { available: true, snapshot: () => ({ ...snapshot, revision }) },
    gateway: { applyStack: gateway },
    commit: { commit },
    createId: () => `new-${++id}`,
  });
  return {
    session,
    gateway,
    commit,
    document,
    editWorkflow: () => {
      revision = 'changed';
    },
  };
}

/** Generate typed canonical results for independently retained and newly staged instances. */
function analysis(ids: readonly string[]): CubeGraphAnalysis {
  return {
    semanticHash: 'hash',
    instances: ids.map((instanceId) => ({
      instanceId,
      nodeId: instanceId,
      definitionId: instanceId,
      cubeId: 'cube',
      version: '1',
      alias: instanceId,
      executionMode: 0,
    })),
    segments: [{ instanceIds: ids, reorderable: true, boundaryNodeIds: [] }],
    placements: ids.map((instanceId, index) => ({
      instanceId,
      nodeId: instanceId,
      position: [index * 244, 0],
    })),
  };
}
