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
import { jest } from '@jest/globals';
import {
  WORKFLOW_VIEW_EXTRA,
  workflowOpensInMasonry,
} from '../../../frontend/comfyui/ui/workflow/infrastructure/ComfyWorkflowViewDocument.js';
import { ComfyWorkflowChangeTracker } from '../../../frontend/comfyui/ui/workflow/infrastructure/ComfyWorkflowChangeTracker.js';

const cube = { id: 1, type: 'subgraph-uuid', properties: { sugarcubes_kind: 'cube' } };
const plain = { id: 2, type: 'KSampler', properties: {} };

test('a workflow opens in the view it recorded, else masonry exactly when it holds a Cube', () => {
  expect(workflowOpensInMasonry({ nodes: [cube], extra: {} })).toBe(true);
  expect(workflowOpensInMasonry({ nodes: [plain], extra: {} })).toBe(false);
  expect(
    workflowOpensInMasonry({ nodes: [{ ...cube, properties: { sugarcubes_kind: 'cube_draft' } }] }),
  ).toBe(true);
  expect(workflowOpensInMasonry({ nodes: [cube], extra: { [WORKFLOW_VIEW_EXTRA]: 'graph' } })).toBe(
    false,
  );
  expect(
    workflowOpensInMasonry({ nodes: [plain], extra: { [WORKFLOW_VIEW_EXTRA]: 'masonry' } }),
  ).toBe(true);
  expect(workflowOpensInMasonry(null)).toBe(false);
});

/** Comfy's change tracker as SugarCubes reads it: stored copies, undo by reloading one. */
function comfy() {
  const state = (value: number) => ({
    nodes: [{ id: value }],
    extra: {} as Record<string, unknown>,
  });
  const tracker = {
    activeState: state(3),
    undoQueue: [state(1), state(2)],
    redoQueue: [state(4)],
    updateModified: jest.fn(),
    undoRedo: () => undefined,
  };
  const rootGraph = { extra: { ds: { scale: 1 } } as Record<string, unknown> };
  const app = {
    rootGraph,
    extensionManager: { workflow: { activeWorkflow: { changeTracker: tracker } } },
  };
  return { tracker, rootGraph, app };
}

test('a view switch is written into the workflow and every stored copy, never as an undo step', () => {
  const { tracker, rootGraph, app } = comfy();
  const report = jest.fn();
  const owner = new ComfyWorkflowChangeTracker(app, window, report);
  expect(owner.recordView('masonry')).toBe(true);
  expect(rootGraph.extra).toEqual({ ds: { scale: 1 }, [WORKFLOW_VIEW_EXTRA]: 'masonry' });
  for (const copy of [tracker.activeState, ...tracker.undoQueue, ...tracker.redoQueue])
    expect(copy.extra[WORKFLOW_VIEW_EXTRA]).toBe('masonry');
  // No new history step: the queues keep every real edit.
  expect(tracker.undoQueue).toHaveLength(2);
  expect(tracker.redoQueue).toHaveLength(1);
  expect(tracker.updateModified).toHaveBeenCalledTimes(1);
  expect(owner.recordView('graph')).toBe(true);
  expect(tracker.undoQueue[0]!.extra[WORKFLOW_VIEW_EXTRA]).toBe('graph');
  expect(report).not.toHaveBeenCalled();
});

test('a tracker of another shape keeps the view for this session only and is reported once', () => {
  const report = jest.fn();
  const owner = new ComfyWorkflowChangeTracker({ rootGraph: { extra: {} } }, window, report);
  expect(owner.recordView('masonry')).toBe(false);
  expect(owner.recordView('graph')).toBe(false);
  expect(report).toHaveBeenCalledTimes(1);
});
