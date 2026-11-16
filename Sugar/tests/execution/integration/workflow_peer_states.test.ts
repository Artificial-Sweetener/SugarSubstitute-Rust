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
/** Prove that other open workflows' stored states are readable and the active one committable. */
import { jest } from '@jest/globals';
import { WorkflowSessionStore } from '../../../frontend/comfyui/ui/execution/application/WorkflowSessionStore.js';
import { ComfyWorkflowSessionAdapter } from '../../../frontend/comfyui/ui/execution/infrastructure/ComfyWorkflowSessionAdapter.js';
import { createComfyWorkflowLoadObserver } from '../../../frontend/comfyui/ui/execution/infrastructure/ComfyWorkflowLoadObserver.js';
import { WorkflowHostFixture, workflowFixture } from '../support/workflow_host.js';

test('reads every open workflow except the active one and commits only the active tracker', async () => {
  const a = workflowFixture('a');
  const b = workflowFixture('b');
  const host = new WorkflowHostFixture(a);
  let key = 0;
  const adapter = new ComfyWorkflowSessionAdapter({
    app: host,
    sessions: new WorkflowSessionStore(),
    nextKey: () => String(++key),
    logger: { error: jest.fn() },
  });
  adapter.install();
  await host.loadGraphData({}, true, true, b);
  const states = { a: { extra: { name: 'a' } }, b: { extra: { name: 'b' } } };
  a.changeTracker.activeState = states.a;
  b.changeTracker.activeState = states.b;
  expect(adapter.readPeerWorkflowStates()).toEqual([states.a]);
  const checkState = jest.fn();
  Object.assign(b.changeTracker, { checkState });
  Object.assign(a.changeTracker, { checkState: jest.fn() });
  adapter.commitLoadedWorkflowState();
  expect(checkState).toHaveBeenCalledTimes(1);
  adapter.dispose();
});

test('a workflow the host closed without announcing it is nobody’s peer', async () => {
  const a = workflowFixture('a');
  const b = workflowFixture('b');
  const host = new WorkflowHostFixture(a);
  let key = 0;
  const adapter = new ComfyWorkflowSessionAdapter({
    app: host,
    sessions: new WorkflowSessionStore(),
    nextKey: () => String(++key),
    logger: { error: jest.fn() },
  });
  adapter.install();
  await host.loadGraphData({}, true, true, b);
  a.changeTracker.activeState = { extra: { name: 'a' } };
  expect(adapter.readPeerWorkflowStates()).toHaveLength(1);
  // Comfy removes a closed tab from its store without notifying store subscribers.
  const store = host.extensionManager.workflow;
  store.openWorkflows = store.openWorkflows.filter((workflow) => workflow !== a);
  expect(adapter.readPeerWorkflowStates()).toEqual([]);
  adapter.dispose();
});

test('the workflow a file load opens owns the live graph before the host activates it', async () => {
  const a = workflowFixture('a');
  const host = new WorkflowHostFixture(a);
  let key = 0;
  const sessions = new WorkflowSessionStore();
  const adapter = new ComfyWorkflowSessionAdapter({
    app: host,
    sessions,
    nextKey: () => String(++key),
    logger: { error: jest.fn() },
  });
  adapter.install();
  const load = createComfyWorkflowLoadObserver(host, sessions, adapter, { error: jest.fn() });
  load.install();
  a.changeTracker.activeState = { extra: { name: 'a' } };
  const opened = { extra: { name: 'opened' } };
  const seen: unknown[][] = [];
  const commits: string[] = [];
  Object.assign(a.changeTracker, { checkState: () => commits.push('a') });
  // After configuration nothing is open for the load yet: the workflow still active is a peer.
  host.afterConfigure = () => seen.push([...adapter.readPeerWorkflowStates()]);
  // Once Comfy has opened the load's own workflow, it is the owner and never its own peer.
  host.pauseBeforeActivate = async () => {
    seen.push([...adapter.readPeerWorkflowStates()]);
    adapter.commitLoadedWorkflowState();
  };
  await host.loadGraphData(opened, true, true, 'opened');
  expect(seen).toEqual([[a.changeTracker.activeState], [a.changeTracker.activeState]]);
  expect(commits).toEqual([]);
  expect(adapter.readPeerWorkflowStates()).toEqual([a.changeTracker.activeState]);
  load.dispose();
  adapter.dispose();
});
