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
/** Prove accepted ownership survives before finals and remains separate from execution replay. */
import { WorkflowSubmissionSnapshots } from '../../../frontend/comfyui/ui/execution/application/WorkflowSubmissionSnapshots.js';
import type { SessionSubmission } from '../../../integrations/contracts/SessionProtocol.generated.js';
import { WorkflowRunRegistry } from '../../../frontend/comfyui/ui/execution/application/WorkflowRunRegistry.js';
import { workflowSubmission } from '../support/workflow_results.js';

const entry = '00000000-0000-4000-8000-000000000001';
const other = '00000000-0000-4000-8000-000000000002';

/** Supply an accepted prompt whose final has not arrived. */
function submission(promptId = 'accepted'): SessionSubmission {
  return {
    entryId: entry,
    backend: '00000000-0000-4000-8000-000000000003',
    promptId,
    runId: '00000000-0000-4000-8000-000000000004',
    runOrder: 2,
    graphRevision: 7,
    scene: null,
    sources: [
      {
        id: '00000000-0000-4000-8000-000000000005',
        nodeId: '23',
        executionPath: '42:23',
        label: 'Saved source',
        graphRevision: 7,
      },
    ],
  };
}

test('acceptance is observable without any final, and a caller cannot mutate captured source ownership', () => {
  const snapshots = new WorkflowSubmissionSnapshots();
  let changes = 0;
  snapshots.subscribe(() => changes++);
  const accepted = submission();
  snapshots.accept(accepted);
  expect(changes).toBe(1);
  expect(snapshots.read(entry)).toEqual([accepted]);
  expect(Object.isFrozen(snapshots.read(entry)[0]?.sources[0])).toBe(true);
  expect(snapshots.source(entry, 'accepted', '42:23')?.label).toBe('Saved source');
  snapshots.accept(accepted);
  expect(changes).toBe(1);
  expect(() => snapshots.accept({ ...accepted, graphRevision: 8 })).toThrow('cannot change');
  expect(() => snapshots.accept({ ...accepted, entryId: other })).toThrow('multiple');
});

test('restoration replaces metadata atomically without invoking generation or editor destinations', () => {
  const snapshots = new WorkflowSubmissionSnapshots();
  snapshots.accept(submission());
  snapshots.accept({ ...submission('elsewhere'), entryId: other });
  const previous = snapshots.read(entry);
  expect(() => snapshots.restore(entry, [submission('elsewhere')])).toThrow('ambiguous');
  expect(snapshots.read(entry)).toEqual(previous);
  snapshots.restore(entry, [{ ...submission('restored'), runOrder: 1 }]);
  expect(snapshots.read(entry).map((item) => item.promptId)).toEqual(['restored']);
  snapshots.synchronize([
    {
      entryId: entry,
      runtimeSession: 'fresh',
      generation: 0,
      graphRevision: 7,
      sourceRevision: 1,
      navigationRevision: 1,
    },
  ]);
  expect(snapshots.read(other)).toEqual([]);
  expect(snapshots.read(entry)).toHaveLength(1);
});

test('a new accepted run starts above the restored frontier without resurrecting previous jobs', () => {
  const runs = new WorkflowRunRegistry();
  const context = workflowSubmission();
  runs.restoreOrder(context.session.sessionId, 44);
  expect(runs.outstanding()).toEqual([]);
  expect(runs.reserve(context).order).toBe(45);
  runs.restoreOrder(context.session.sessionId, 2);
  expect(runs.latestOrder(context.session.sessionId)).toBe(45);
});
