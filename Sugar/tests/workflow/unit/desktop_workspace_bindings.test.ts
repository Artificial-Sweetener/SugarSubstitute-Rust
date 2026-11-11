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
/** Qualify occurrence identity and revision fences without relying on names or host graph objects. */
import { DesktopWorkspaceBindings } from '../../../frontend/comfyui/ui/workflow/application/DesktopWorkspaceBindings.js';
import { WorkflowSessionStore } from '../../../frontend/comfyui/ui/execution/application/WorkflowSessionStore.js';
import { workflowSessionId } from '../../../frontend/comfyui/ui/execution/domain/ExecutionIdentity.js';
import { sameSessionFence } from '../../../integrations/shared/SessionProtocol.js';

/** Supply deterministic distinct UUIDs independently of browser cryptography. */
function identities(): () => string {
  let sequence = 10;
  return () => `00000000-0000-4000-8000-${(++sequence).toString(16).padStart(12, '0')}`;
}

const webview = '00000000-0000-4000-8000-000000000001';
const a = { sessionId: workflowSessionId('a'), title: 'Same', path: 'workflows/same.json' };
const b = { ...a, sessionId: workflowSessionId('b') };

test('same labels and paths have independent occurrences while rename and reorder preserve identity', () => {
  const sessions = new WorkflowSessionStore();
  const bindings = new DesktopWorkspaceBindings(webview, identities());
  sessions.synchronize([a, b], b.sessionId);
  bindings.synchronize(sessions.read());
  const first = bindings.fence();
  expect(first.bindings[0].entryId).not.toBe(first.bindings[1].entryId);
  sessions.synchronize([b, { ...a, title: 'Renamed', path: 'workflows/new.json' }], a.sessionId);
  bindings.synchronize(sessions.read());
  expect(bindings.fence().bindings.map((binding) => binding.entryId)).toEqual(
    [...first.bindings].reverse().map((binding) => binding.entryId),
  );
  expect(first.bindings[0].graphRevision).toBe(0);
  expect(sameSessionFence(first, bindings.fence())).toBe(false);
  const settled = bindings.fence();
  bindings.synchronize(sessions.read());
  expect(bindings.fence()).toEqual(settled);
});

test('graph source navigation and load commitment each invalidate a captured immutable fence', () => {
  const sessions = new WorkflowSessionStore();
  const bindings = new DesktopWorkspaceBindings(webview, identities());
  sessions.synchronize([a], a.sessionId);
  bindings.synchronize(sessions.read());
  for (const owner of ['graph', 'source', 'navigation'] as const) {
    const previous = bindings.fence();
    bindings.observe(a.sessionId, owner, 'changed');
    expect(sameSessionFence(previous, bindings.fence())).toBe(false);
    const current = bindings.fence();
    bindings.observe(a.sessionId, owner, 'changed');
    expect(bindings.fence()).toEqual(current);
  }
  const beforeLoad = bindings.fence();
  const load = sessions.beginLoad('replace');
  bindings.synchronize(sessions.read());
  expect(sameSessionFence(beforeLoad, bindings.fence())).toBe(false);
  sessions.completeLoad(load, true);
  bindings.synchronize(sessions.read());
  expect(bindings.binding(a.sessionId).generation).toBe(1);
  expect(beforeLoad.bindings[0].generation).toBe(0);
});

test('host adoption is exact and closed workflows cannot be revived by delayed bindings', () => {
  const sessions = new WorkflowSessionStore();
  const bindings = new DesktopWorkspaceBindings(webview, identities());
  sessions.synchronize([a, b], a.sessionId);
  bindings.synchronize(sessions.read());
  const adopted = { ...bindings.binding(a.sessionId), entryId: webview, graphRevision: 7 };
  bindings.bind(adopted);
  expect(bindings.binding(a.sessionId)).toEqual(adopted);
  expect(() => bindings.bind({ ...adopted, runtimeSession: b.sessionId })).toThrow();
  sessions.synchronize([b], b.sessionId);
  bindings.synchronize(sessions.read());
  expect(() => bindings.bind(adopted)).toThrow('unavailable');
  sessions.synchronize([a, b], a.sessionId);
  bindings.synchronize(sessions.read());
  expect(bindings.binding(a.sessionId).entryId).not.toBe(adopted.entryId);
});

test('empty repeated commitment does not create dirty observations and reused UUIDs refuse admission', () => {
  const sessions = new WorkflowSessionStore();
  const bindings = new DesktopWorkspaceBindings(webview, () => webview);
  bindings.synchronize(sessions.read());
  const empty = bindings.fence();
  bindings.synchronize(sessions.read());
  expect(bindings.fence()).toEqual(empty);
  sessions.synchronize([a, b], a.sessionId);
  expect(() => bindings.synchronize(sessions.read())).toThrow('reused');
  expect(bindings.fence()).toEqual(empty);
});

test('a fence describes a complete workspace only once every occurrence has every owner observed', () => {
  const sessions = new WorkflowSessionStore();
  const bindings = new DesktopWorkspaceBindings(webview, identities());
  sessions.synchronize([a, b], a.sessionId);
  bindings.synchronize(sessions.read());
  expect(bindings.observes(sessions.read())).toBe(false);
  for (const owner of ['graph', 'source', 'navigation'] as const)
    bindings.observe(a.sessionId, owner, 'seen');
  expect(bindings.observes(sessions.read())).toBe(false);
  bindings.prime(b.sessionId, 'graph', 'seen');
  bindings.prime(b.sessionId, 'source', 'seen');
  expect(bindings.observes(sessions.read())).toBe(false);
  bindings.prime(b.sessionId, 'navigation', 'seen');
  expect(bindings.observes(sessions.read())).toBe(true);
  const load = sessions.beginLoad('replace');
  sessions.completeLoad(load, true);
  bindings.synchronize(sessions.read());
  expect(bindings.observes(sessions.read())).toBe(false);
});
