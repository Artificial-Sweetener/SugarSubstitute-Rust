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
/** Prove configuration barriers and generations remain safe across overlapping or obsolete loads. */

import { WorkflowSessionStore } from '../../../frontend/comfyui/ui/execution/application/WorkflowSessionStore.js';
import { workflowSessionId } from '../../../frontend/comfyui/ui/execution/domain/ExecutionIdentity.js';

const a = { sessionId: workflowSessionId('a'), title: 'A', path: 'a.json' };
const b = { sessionId: workflowSessionId('b'), title: 'B', path: 'b.json' };

test('waits for every overlapping load and fails closed if any load fails', () => {
  const sessions = new WorkflowSessionStore();
  sessions.synchronize([a, b], a.sessionId);
  const first = sessions.beginLoad();
  const second = sessions.beginLoad();
  sessions.synchronize([a, b], b.sessionId);
  sessions.completeLoad(first, false);
  expect(sessions.read().active).toBeNull();
  expect(sessions.read().loading).toBe(true);
  sessions.completeLoad(second, true);
  expect(sessions.read().active).toBeNull();
  expect(sessions.read().loading).toBe(false);
  const recovered = sessions.beginLoad();
  sessions.completeLoad(recovered, true);
  expect(sessions.read().active?.sessionId).toBe(b.sessionId);
});

test('ignores forged, duplicate and disposed load completions', () => {
  const sessions = new WorkflowSessionStore();
  sessions.synchronize([a], a.sessionId);
  const token = sessions.beginLoad();
  sessions.completeLoad({ ...token }, true);
  expect(sessions.read().loading).toBe(true);
  sessions.completeLoad(token, true);
  const revision = sessions.read().revision;
  sessions.completeLoad(token, true);
  expect(sessions.read().revision).toBe(revision);
  const obsolete = sessions.beginLoad();
  sessions.dispose();
  sessions.completeLoad(obsolete, true);
  expect(sessions.read().open).toEqual([]);
  expect(sessions.read().active).toBeNull();
});

test('preserves history generations and retains replacement invalidation across overlapping loads', () => {
  const sessions = new WorkflowSessionStore();
  sessions.synchronize([a], a.sessionId);
  const history = sessions.beginLoad('history');
  sessions.completeLoad(history, true);
  expect(sessions.read().active?.generation).toBe(0);
  const replacement = sessions.beginLoad();
  const overlappingHistory = sessions.beginLoad('history');
  sessions.completeLoad(replacement, true);
  expect(sessions.read().active).toBeNull();
  sessions.completeLoad(overlappingHistory, true);
  expect(sessions.read().active?.generation).toBe(1);
});

test('retains presentation only while every overlapping load restores the same workflow history', () => {
  const sessions = new WorkflowSessionStore();
  sessions.synchronize([a, b], a.sessionId);
  const first = sessions.beginLoad('history');
  const second = sessions.beginLoad('history');
  expect(sessions.read().active).toBeNull();
  expect(sessions.presentation()?.sessionId).toBe(a.sessionId);
  sessions.completeLoad(first, true);
  expect(sessions.presentation()?.sessionId).toBe(a.sessionId);
  const replacement = sessions.beginLoad();
  expect(sessions.presentation()).toBeNull();
  sessions.completeLoad(replacement, true);
  expect(sessions.presentation()).toBeNull();
  sessions.completeLoad(second, true);
  expect(sessions.presentation()?.generation).toBe(1);
  const switched = sessions.beginLoad('history');
  sessions.synchronize([a, b], b.sessionId);
  expect(sessions.presentation()).toBeNull();
  sessions.completeLoad(switched, true);
  expect(sessions.presentation()?.sessionId).toBe(b.sessionId);
  sessions.suspend();
  expect(sessions.presentation()).toBeNull();
});

test('keeps snapshots immutable and releases closed session metadata', () => {
  const sessions = new WorkflowSessionStore();
  sessions.synchronize([a, b], a.sessionId);
  const snapshot = sessions.read();
  expect(Object.isFrozen(snapshot)).toBe(true);
  expect(Object.isFrozen(snapshot.open)).toBe(true);
  expect(Object.isFrozen(snapshot.active)).toBe(true);
  expect(sessions.synchronize([b], b.sessionId)).toEqual([a.sessionId]);
  expect(snapshot.open).toHaveLength(2);
  expect(sessions.read().open).toHaveLength(1);
});
