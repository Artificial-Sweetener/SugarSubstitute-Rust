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
/** Prove stack reorder remains canonical, atomic and tied to the exact current workflow. */
import { expect, jest, test } from '@jest/globals';
import { CubeGraphReorderService } from '../../../frontend/comfyui/ui/workflow/application/CubeGraphReorderService.js';
import { readCubeGraphAnalysis } from '../../../frontend/comfyui/ui/workflow/infrastructure/CubeGraphResponse.js';
import type {
  CubeGraphAnalysis,
  CubeGraphSnapshot,
  CubeGraphPlacement,
} from '../../../frontend/comfyui/ui/workflow/domain/CubeGraphModels.js';
import { graphResponse } from '../support/CubeGraphFixtures.js';

test('commits an accepted permutation once after canonical validation', async () => {
  const fixture = setup();
  await fixture.service.refresh();
  expect(fixture.service.canMove('first', 1)).toBe(true);
  await fixture.service.move('first', 1);
  expect(fixture.reorder).toHaveBeenCalledWith(
    fixture.snapshot(),
    ['first', 'second'],
    ['second', 'first'],
  );
  expect(fixture.commit).toHaveBeenCalledTimes(1);
  expect(fixture.service.pending).toBe(false);
});

test('retains the validated commit analysis for immediate repeated moves and still refreshes external changes', async () => {
  const fixture = setup();
  fixture.commit.mockImplementation(() => {
    fixture.changeRevision();
    return true;
  });
  await fixture.service.move('first', 1);
  expect(fixture.service.canMove('first', -1)).toBe(true);
  expect(await fixture.service.refresh()).toEqual(reordered());
  expect(fixture.analyze).toHaveBeenCalledTimes(1);
  fixture.changeRevision();
  expect(fixture.service.analysis).toBeNull();
  await fixture.service.refresh();
  expect(fixture.analyze).toHaveBeenCalledTimes(2);
});

test('does not retain commit analysis if the native workflow lifetime ends during its notification', async () => {
  const fixture = setup();
  fixture.commit.mockImplementation(() => {
    fixture.replaceWorkflow();
    return true;
  });
  await fixture.service.move('first', 1);
  expect(fixture.service.analysis).toBeNull();
  expect(fixture.service.pending).toBe(false);
});

test('does not expose reorder permissions for stale graph facts', async () => {
  const fixture = setup();
  await fixture.service.refresh();
  fixture.changeRevision();
  expect(fixture.service.canMove('first', 1)).toBe(false);
});

test('rebases a delayed reorder on the latest edited workflow before committing', async () => {
  const fixture = setup();
  fixture.reorder.mockImplementationOnce(async () => {
    fixture.changeRevision();
    return reordered();
  });
  await fixture.service.move('first', 1);
  expect(fixture.reorder).toHaveBeenCalledTimes(2);
  expect(fixture.commit).toHaveBeenCalledWith(fixture.snapshot(), reordered().placements);
});

test('does not confuse background cache invalidation with a workflow replacement', async () => {
  const fixture = setup();
  fixture.reorder.mockImplementationOnce(async () => {
    fixture.changeRevision();
    fixture.service.invalidate();
    await fixture.service.refresh();
    return reordered();
  });
  await fixture.service.move('first', 1);
  expect(fixture.commit).toHaveBeenCalledTimes(1);
  expect(fixture.service.pending).toBe(false);
});

test('accepts placement analysis while unrelated serialized values continue changing', async () => {
  const fixture = setup();
  fixture.analyze.mockImplementation(async () => {
    fixture.changeValue();
    return readCubeGraphAnalysis(fixture.analysis);
  });
  await fixture.service.move('first', 1);
  expect(fixture.analyze).toHaveBeenCalledTimes(1);
  expect(fixture.reorder).toHaveBeenCalledTimes(1);
  expect(fixture.commit).toHaveBeenCalledTimes(1);
});

test('keeps drag permissions stable across value notifications without another analysis request', async () => {
  const fixture = setup();
  await fixture.service.refresh();
  fixture.changeValue();
  fixture.service.invalidate();
  expect(fixture.service.canMove('first', 1)).toBe(true);
  expect(await fixture.service.refresh()).not.toBeNull();
  expect(fixture.analyze).toHaveBeenCalledTimes(1);
});

test('never rebases across a replaced workflow even when its serialization matches', async () => {
  const fixture = setup();
  fixture.reorder.mockImplementationOnce(async () => {
    fixture.replaceWorkflow();
    return reordered();
  });
  await expect(fixture.service.move('first', 1)).rejects.toThrow('workflow');
  expect(fixture.commit).not.toHaveBeenCalled();
  expect(fixture.reorder).toHaveBeenCalledTimes(1);
  expect(fixture.service.pending).toBe(false);
});

test('retries a stale commit without creating duplicate native transactions', async () => {
  const fixture = setup();
  fixture.commit.mockImplementationOnce(() => {
    fixture.changeRevision();
    return false;
  });
  await fixture.service.move('first', 1);
  expect(fixture.reorder).toHaveBeenCalledTimes(2);
  expect(fixture.commit).toHaveBeenCalledTimes(2);
  expect(fixture.service.pending).toBe(false);
});

test('bounds continuously edited reorders and permits the next settled drag', async () => {
  const fixture = setup();
  fixture.reorder.mockImplementation(async () => {
    fixture.changeRevision();
    return reordered();
  });
  await expect(fixture.service.reorder('first', ['second', 'first'])).rejects.toThrow('settle');
  expect(fixture.reorder.mock.calls.length).toBeLessThanOrEqual(8);
  expect(fixture.commit).not.toHaveBeenCalled();
  expect(fixture.service.pending).toBe(false);
  fixture.reorder.mockImplementation(async () => reordered());
  await fixture.service.reorder('first', ['second', 'first']);
  expect(fixture.commit).toHaveBeenCalledTimes(1);
});

test('rejects a duplicate or foreign permutation without contacting the mutation owner', async () => {
  const fixture = setup();
  await expect(fixture.service.reorder('first', ['first', 'first'])).rejects.toThrow(
    'exact Cube permutation',
  );
  await expect(fixture.service.reorder('first', ['first', 'foreign'])).rejects.toThrow(
    'exact Cube permutation',
  );
  expect(fixture.reorder).not.toHaveBeenCalled();
  expect(fixture.commit).not.toHaveBeenCalled();
});

test('reorders wired cubes by geometry independently of execution segments', async () => {
  const fixture = setup();
  fixture.analysis.segments[0]!.reorderable = false;
  await fixture.service.refresh();
  expect(fixture.service.canMove('first', 1)).toBe(true);
  await fixture.service.move('first', 1);
  expect(fixture.commit).toHaveBeenCalledTimes(1);
});

test('retains the live graph and releases busy state after a backend failure', async () => {
  const fixture = setup();
  fixture.reorder.mockRejectedValue(new Error('Rejected by graph owner'));
  await expect(fixture.service.move('first', 1)).rejects.toThrow('Rejected by graph owner');
  expect(fixture.commit).not.toHaveBeenCalled();
  expect(fixture.service.pending).toBe(false);
});

test('rejects a response that moved or replaced another workflow instance', async () => {
  const fixture = setup();
  const result = reordered();
  fixture.reorder.mockResolvedValue({ ...result, instances: result.instances.slice(0, 1) });
  await expect(fixture.service.move('first', 1)).rejects.toThrow('instance identity');
  expect(fixture.commit).not.toHaveBeenCalled();
});

test.each(['alias', 'version', 'definitionId', 'cubeId', 'executionMode'] as const)(
  'rejects a reorder response that changes instance %s',
  async (field) => {
    const fixture = setup();
    const result = reordered();
    fixture.reorder.mockResolvedValue({
      ...result,
      instances: result.instances.map((instance, index) =>
        index === 0
          ? {
              ...instance,
              [field]: field === 'executionMode' ? 4 : 'different',
            }
          : instance,
      ),
    });
    await expect(fixture.service.move('first', 1)).rejects.toThrow('instance identity');
    expect(fixture.commit).not.toHaveBeenCalled();
  },
);

test('rejects incomplete placements without creating a native transaction', async () => {
  const fixture = setup();
  const result = reordered();
  fixture.reorder.mockResolvedValue({ ...result, placements: result.placements.slice(0, 1) });
  await expect(fixture.service.move('first', 1)).rejects.toThrow('placement');
  expect(fixture.commit).not.toHaveBeenCalled();
});

test('coalesces simultaneous analysis of the same workflow', async () => {
  const fixture = setup();
  await Promise.all([fixture.service.refresh(), fixture.service.refresh()]);
  expect(fixture.analyze).toHaveBeenCalledTimes(1);
});

test('does not accept an analysis response after the owning lifecycle invalidates it', async () => {
  const fixture = setup();
  fixture.analyze.mockImplementation(async () => {
    fixture.service.invalidate();
    return readCubeGraphAnalysis(fixture.analysis);
  });
  expect(await fixture.service.refresh()).toBeNull();
  expect(fixture.service.analysis).toBeNull();
  expect(fixture.commit).not.toHaveBeenCalled();
});

test('refreshes the latest snapshot when native geometry settles during analysis', async () => {
  const fixture = setup();
  fixture.analyze.mockImplementationOnce(async () => {
    fixture.changeRevision();
    return readCubeGraphAnalysis(fixture.analysis);
  });
  expect(await fixture.service.refresh()).not.toBeNull();
  expect(fixture.analyze).toHaveBeenCalledTimes(2);
  expect(fixture.service.canMove('first', 1)).toBe(true);
  expect(fixture.commit).not.toHaveBeenCalled();
});

test('bounds repeated stale analysis, rejects obsolete permissions and recovers on a settled refresh', async () => {
  const fixture = setup();
  fixture.analyze.mockImplementation(async () => {
    fixture.changeRevision();
    return readCubeGraphAnalysis(fixture.analysis);
  });
  await expect(fixture.service.refresh()).rejects.toThrow('Workflow kept changing');
  expect(fixture.analyze).toHaveBeenCalledTimes(4);
  expect(fixture.service.analysis).toBeNull();
  expect(fixture.service.pending).toBe(false);
  fixture.analyze.mockImplementation(async () => readCubeGraphAnalysis(fixture.analysis));
  expect(await fixture.service.refresh()).not.toBeNull();
  expect(fixture.service.canMove('first', 1)).toBe(true);
});

/** Return the canonical accepted order without introducing another ordering algorithm. */
function reordered(): CubeGraphAnalysis {
  const response = graphResponse();
  response.segments[0]!.instance_ids = ['second', 'first'];
  response.workflow.nodes[0]!.pos = [244, 0];
  response.workflow.nodes[1]!.pos = [0, 0];
  return readCubeGraphAnalysis(response);
}

/** Bind isolated snapshot state and an observable commit boundary. */
function setup() {
  let revision = 'original';
  let placementRevision = 'original';
  const placementsByRevision = new Map<string, string>();
  let current = true;
  const analysis = graphResponse();
  const snapshot = () => {
    placementsByRevision.set(revision, placementRevision);
    return { revision, serializedWorkflow: JSON.stringify(analysis.workflow) };
  };
  const commit = jest.fn(
    (_snapshot: CubeGraphSnapshot, _placements: readonly CubeGraphPlacement[]) => true,
  );
  const reorder = jest.fn(async () => reordered());
  const analyze = jest.fn(async () => readCubeGraphAnalysis(analysis));
  const service = new CubeGraphReorderService({
    gateway: { analyze, reorder },
    host: {
      available: true,
      get revision() {
        return revision;
      },
      snapshot,
      isCurrent: () => current,
      isOrderCurrent: (before) =>
        current && placementsByRevision.get(before.revision) === placementRevision,
      isPlacementCurrent: (before) =>
        current && placementsByRevision.get(before.revision) === placementRevision,
      commit: (before, placements) => before.revision === revision && commit(before, placements),
    },
  });
  return {
    service,
    analysis,
    analyze,
    reorder,
    commit,
    snapshot,
    replaceWorkflow: () => {
      current = false;
    },
    changeRevision: () => {
      revision += ':edited';
      placementRevision = revision;
    },
    changeValue: () => {
      revision += ':value';
    },
  };
}
