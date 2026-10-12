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
/** Prove workflow association waits for real load commitment rather than synchronous hooks. */

import { jest } from '@jest/globals';
import { WorkflowSessionStore } from '../../../frontend/comfyui/ui/execution/application/WorkflowSessionStore.js';
import { ComfyWorkflowSessionAdapter } from '../../../frontend/comfyui/ui/execution/infrastructure/ComfyWorkflowSessionAdapter.js';
import { createComfyWorkflowLoadObserver } from '../../../frontend/comfyui/ui/execution/infrastructure/ComfyWorkflowLoadObserver.js';
import { WorkflowHostFixture, workflowFixture } from '../support/workflow_host.js';

/** Hold the host between configuration and its later active-store commitment. */
function deferred(): { promise: Promise<void>; finish(): void } {
  let resolve: (() => void) | null = null;
  const promise = new Promise<void>((complete) => {
    resolve = complete;
  });
  return {
    promise,
    finish() {
      if (!resolve) throw new Error('Missing deferred completion.');
      resolve();
    },
  };
}

test('keeps the reused root unassigned until the loaded workflow has committed', async () => {
  const a = workflowFixture('a');
  const b = workflowFixture('b');
  const host = new WorkflowHostFixture(a);
  const sessions = new WorkflowSessionStore();
  let key = 0;
  const adapter = new ComfyWorkflowSessionAdapter({
    app: host,
    sessions,
    nextKey: () => String(++key),
    logger: { error: jest.fn() },
  });
  adapter.install();
  const first = sessions.read().active;
  const originalGraph = host.graph;
  const pending = deferred();
  host.pauseAfterConfigure = () => pending.promise;
  host.afterConfigure = () => {
    expect(host.graph).toBe(originalGraph);
    expect(host.extensionManager.workflow.activeWorkflow).toBe(a);
    expect(sessions.read().active).toBeNull();
    expect(host.graph.nodes).toEqual(b.nodes);
  };
  const observer = createComfyWorkflowLoadObserver(host, sessions, adapter, { error: jest.fn() });
  observer.install();
  const load = host.loadGraphData({}, true, true, b);
  expect(sessions.read().loading).toBe(true);
  pending.finish();
  await load;
  expect(sessions.read().active?.title).toBe('b');
  expect(sessions.read().active?.sessionId).not.toBe(first?.sessionId);
  expect(sessions.read().loading).toBe(false);
  observer.dispose();
  adapter.dispose();
  expect(host.subscriberCount).toBe(0);
});

test('preserves identity across rename and Save As but retires a closed binding before reopening', () => {
  const a = workflowFixture('a');
  const b = workflowFixture('b');
  const host = new WorkflowHostFixture(a);
  host.extensionManager.workflow.openWorkflows.push(b);
  const sessions = new WorkflowSessionStore();
  let key = 0;
  const adapter = new ComfyWorkflowSessionAdapter({
    app: host,
    sessions,
    nextKey: () => String(++key),
    logger: { error: jest.fn() },
  });
  adapter.install();
  const first = sessions.read().active;
  expect(sessions.read().open).toHaveLength(2);
  a.filename = 'renamed';
  a.path = 'workflows/saved-as.json';
  host.notify();
  expect(sessions.read().active?.sessionId).toBe(first?.sessionId);
  expect(sessions.read().active?.title).toBe('renamed');
  host.close(a);
  expect(sessions.read().open).toHaveLength(1);
  expect(sessions.read().active).toBeNull();
  host.extensionManager.workflow.openWorkflows.push(a);
  host.extensionManager.workflow.activeWorkflow = a;
  host.notify();
  expect(sessions.read().active?.sessionId).not.toBe(first?.sessionId);
  adapter.dispose();
});

test('advances same-object reload and undo generations without resetting a tab-return generation', async () => {
  const a = workflowFixture('a');
  const b = workflowFixture('b');
  const host = new WorkflowHostFixture(a);
  const sessions = new WorkflowSessionStore();
  let key = 0;
  const adapter = new ComfyWorkflowSessionAdapter({
    app: host,
    sessions,
    nextKey: () => String(++key),
    logger: { error: jest.fn() },
  });
  adapter.install();
  const firstId = sessions.read().active?.sessionId;
  const observer = createComfyWorkflowLoadObserver(host, sessions, adapter, { error: jest.fn() });
  observer.install();
  await host.loadGraphData({}, true, true, a);
  expect(sessions.read().active?.generation).toBe(1);
  await host.loadGraphData({}, true, true, a);
  expect(sessions.read().active?.generation).toBe(2);
  await host.loadGraphData({}, true, true, b);
  expect(sessions.read().active?.generation).toBe(0);
  await host.loadGraphData({}, true, true, a);
  expect(sessions.read().active?.sessionId).toBe(firstId);
  expect(sessions.read().active?.generation).toBe(2);
  observer.dispose();
  adapter.dispose();
});

test('does not restore association if the public store is malformed when a load completes', async () => {
  const a = workflowFixture('a');
  const store: { activeWorkflow: unknown; openWorkflows: unknown; $subscribe(): () => void } = {
    activeWorkflow: a,
    openWorkflows: [a],
    $subscribe: () => () => undefined,
  };
  const app = {
    extensionManager: { workflow: store },
    loadGraphData: async () => {
      store.openWorkflows = 'malformed';
    },
  };
  const sessions = new WorkflowSessionStore();
  const logger = { error: jest.fn() };
  const adapter = new ComfyWorkflowSessionAdapter({ app, sessions, nextKey: () => 'a', logger });
  adapter.install();
  const observer = createComfyWorkflowLoadObserver(app, sessions, adapter, logger);
  observer.install();
  await app.loadGraphData();
  expect(sessions.read().active).toBeNull();
  expect(sessions.read().loading).toBe(false);
  expect(logger.error).toHaveBeenCalledTimes(1);
  observer.dispose();
  adapter.dispose();
});

test('keeps a rejected host load unassigned and preserves its original failure', async () => {
  const a = workflowFixture('a');
  const host = new WorkflowHostFixture(a);
  const failure = new Error('Cannot load workflow');
  host.beforeConfigure = () => {
    throw failure;
  };
  const sessions = new WorkflowSessionStore();
  const logger = { error: jest.fn() };
  const adapter = new ComfyWorkflowSessionAdapter({
    app: host,
    sessions,
    nextKey: () => 'a',
    logger,
  });
  adapter.install();
  const observer = createComfyWorkflowLoadObserver(host, sessions, adapter, logger);
  observer.install();
  await expect(host.loadGraphData({}, true, true, a)).rejects.toBe(failure);
  expect(sessions.read().active).toBeNull();
  expect(sessions.read().loading).toBe(false);
  expect(logger.error).toHaveBeenCalledWith(expect.any(String), { error: failure });
  observer.dispose();
  adapter.dispose();
});
