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
  expect(adapter.readOtherStoredWorkflows()).toEqual([states.a]);
  const checkState = jest.fn();
  Object.assign(b.changeTracker, { checkState });
  Object.assign(a.changeTracker, { checkState: jest.fn() });
  adapter.commitActiveWorkflowState();
  expect(checkState).toHaveBeenCalledTimes(1);
  adapter.dispose();
});
