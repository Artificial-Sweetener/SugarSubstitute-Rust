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
/**
 * Prove each open workflow owns a unique input-canvas identity: copies are found among restored
 * tabs the host has not loaded and among the previous tab while an opened file still configures,
 * and one open keeps one identity from its first sync to its later ones.
 */
import { jest } from '@jest/globals';
import { WorkflowSessionStore } from '../../../frontend/comfyui/ui/execution/application/WorkflowSessionStore.js';
import { ComfyWorkflowSessionAdapter } from '../../../frontend/comfyui/ui/execution/infrastructure/ComfyWorkflowSessionAdapter.js';
import { createComfyWorkflowLoadObserver } from '../../../frontend/comfyui/ui/execution/infrastructure/ComfyWorkflowLoadObserver.js';
import {
  ComfyWorkflowInputState,
  WORKFLOW_INPUT_STATE_KEY,
} from '../../../frontend/comfyui/ui/input_canvas/infrastructure/ComfyWorkflowInputState.js';
import { isRecord } from '../../../frontend/comfyui/ui/types/common.js';
import type { ComfyGraph } from '../../../frontend/comfyui/ui/types/graph.js';
import { WorkflowHostFixture, workflowFixture } from '../support/workflow_host.js';

/** The identity an authored workflow file carries in every copy opened from it. */
const AUTHORED = 'workflow-b64966ef-e0e6-4c60-a0f4-18f55dc2b205';

/** A workflow's serialized `extra` recording `id` as its input-canvas identity. */
function extra(id: string): Record<string, unknown> {
  return {
    [WORKFLOW_INPUT_STATE_KEY]: { version: 1, id, route: 'input', surface: null, mask: null },
  };
}

/** The identity recorded in a live root graph. */
function recorded(root: ComfyGraph): unknown {
  const value = isRecord(root.extra) ? root.extra[WORKFLOW_INPUT_STATE_KEY] : undefined;
  return isRecord(value) ? value.id : undefined;
}

/** Hold the host between configuration and its later active-store commitment. */
function deferred(): { promise: Promise<void>; finish(): void } {
  let finish: () => void = () => undefined;
  const promise = new Promise<void>((resolve) => {
    finish = resolve;
  });
  return { promise, finish: () => finish() };
}

/** Compose the host, its load observation and the input-canvas identity over one live root. */
function fixture() {
  const active = workflowFixture('restored');
  active.changeTracker.activeState = { extra: extra('workflow-active') };
  const host = new WorkflowHostFixture(active);
  const sessions = new WorkflowSessionStore();
  const logger = { error: jest.fn() };
  let keys = 0;
  const adapter = new ComfyWorkflowSessionAdapter({
    app: host,
    sessions,
    nextKey: () => String(++keys),
    logger,
  });
  adapter.install();
  const observer = createComfyWorkflowLoadObserver(host, sessions, adapter, logger);
  observer.install();
  const root: ComfyGraph = { extra: extra('workflow-active') };
  let ids = 0;
  const state = new ComfyWorkflowInputState(
    () => root,
    () => 'fresh-' + String(++ids),
    () => adapter.readPeerWorkflowStates(),
    () => adapter.commitLoadedWorkflowState(),
  );
  return { host, active, adapter, observer, root, state, logger };
}

test('a workflow sharing its identity with a restored tab not loaded yet takes a fresh identity', () => {
  const f = fixture();
  f.host.extensionManager.workflow.openWorkflows.push({
    path: 'workflows/proof_regional_two.json',
    filename: 'proof_regional_two',
    content: JSON.stringify({ nodes: [], extra: extra(AUTHORED) }),
  });
  f.host.notify();
  expect(f.adapter.readPeerWorkflowStates()).toEqual([{ nodes: [], extra: extra(AUTHORED) }]);
  f.root.extra = extra(AUTHORED);
  const identity = f.state.workflow(true);
  expect(identity).toBe('workflow-fresh-1');
  expect(recorded(f.root)).toBe('workflow-fresh-1');
  expect(f.state.workflow(false)).toBe('workflow-fresh-1');
  expect(f.logger.error).not.toHaveBeenCalled();
  f.observer.dispose();
  f.adapter.dispose();
});

test('an opened file copying the previous tab takes one fresh identity before its first sync', async () => {
  const f = fixture();
  f.active.changeTracker.activeState = { extra: extra(AUTHORED) };
  f.root.extra = extra(AUTHORED);
  const checkState = jest.fn();
  Object.assign(f.active.changeTracker, { checkState });
  const opened = { nodes: [], extra: extra(AUTHORED) };
  const pending = deferred();
  f.host.pauseAfterConfigure = () => pending.promise;
  f.host.beforeConfigure = () => {
    f.root.extra = { ...opened.extra };
  };
  const load = f.host.loadGraphData(opened, true, true, 'proof_regional_two (2).json');
  // The page's first sync, carrying the loader's files, runs while the store still names the
  // previous tab active.
  const first = f.state.workflow(true);
  expect(first).toBe('workflow-fresh-1');
  expect(checkState).not.toHaveBeenCalled();
  expect(f.active.changeTracker.activeState).toEqual({ extra: extra(AUTHORED) });
  pending.finish();
  await load;
  expect(f.host.extensionManager.workflow.activeWorkflow).not.toBe(f.active);
  // Later syncs of the same open, after the host committed the new tab, keep its identity.
  expect(f.state.workflow(true)).toBe(first);
  expect(f.state.workflow(false)).toBe(first);
  f.observer.dispose();
  f.adapter.dispose();
});

test('switching to an open tab keeps its own identity while its graph configures', async () => {
  const f = fixture();
  const other = workflowFixture('other');
  other.changeTracker.activeState = { extra: extra('workflow-other') };
  f.host.extensionManager.workflow.openWorkflows.push(other);
  f.host.notify();
  const pending = deferred();
  f.host.pauseAfterConfigure = () => pending.promise;
  f.host.beforeConfigure = () => {
    f.root.extra = extra('workflow-other');
  };
  const load = f.host.loadGraphData(other.changeTracker.activeState, true, true, other);
  expect(f.state.workflow(true)).toBe('workflow-other');
  pending.finish();
  await load;
  expect(f.state.workflow(true)).toBe('workflow-other');
  expect(f.adapter.readPeerWorkflowStates()).toEqual([{ extra: extra('workflow-active') }]);
  f.observer.dispose();
  f.adapter.dispose();
});
