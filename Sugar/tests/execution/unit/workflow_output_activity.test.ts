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
/** Prove unread output transitions remain independent of restored images and tab labels. */
import { WorkflowOutputActivity } from '../../../frontend/comfyui/ui/execution/application/WorkflowOutputActivity.js';
import { WorkflowSessionStore } from '../../../frontend/comfyui/ui/execution/application/WorkflowSessionStore.js';
import { workflowSessionId } from '../../../frontend/comfyui/ui/execution/domain/ExecutionIdentity.js';
import { WorkflowExecutionIngestor } from '../../../frontend/comfyui/ui/execution/application/WorkflowExecutionIngestor.js';
import { workflowResultsFixture, workflowFinal } from '../support/workflow_results.js';

test('accepted image activity ignores repeated deliveries, metadata and foreign jobs', () => {
  const fixture = workflowResultsFixture();
  const accepted: string[] = [];
  const ingestor = new WorkflowExecutionIngestor({
    runs: fixture.runs,
    results: fixture.results,
    clock: fixture.clock,
    logger: { warn: () => undefined },
    recover: () => undefined,
    imageAccepted: (id) => accepted.push(id),
  });
  ingestor.accepted(ingestor.submitting(fixture.a), fixture.id);
  expect(accepted).toEqual([]);
  const image = workflowFinal(fixture.id);
  ingestor.final(image);
  ingestor.final(image);
  ingestor.final({ ...image, executionPaths: ['unknown'] });
  expect(accepted).toEqual([fixture.a.session.sessionId]);
  ingestor.dispose();
  fixture.ingestor.dispose();
});

test('notify once for inactive output and clear only on committed viewing', () => {
  const activity = new WorkflowOutputActivity();
  const sessions = new WorkflowSessionStore();
  const a = workflowSessionId('a');
  const b = workflowSessionId('b');
  const bindings = [
    { sessionId: a, title: 'A', path: 'a' },
    { sessionId: b, title: 'B', path: 'b' },
  ];
  sessions.synchronize(bindings, a);
  activity.synchronize(sessions.read());
  let changes = 0;
  activity.subscribe(() => changes++);
  activity.record(a);
  expect(changes).toBe(0);
  activity.record(b);
  activity.record(b);
  expect(changes).toBe(1);
  expect(activity.unread(b)).toBe(true);
  const load = sessions.beginLoad('activate');
  sessions.synchronize(bindings, b);
  activity.synchronize(sessions.read());
  expect(activity.unread(b)).toBe(true);
  sessions.completeLoad(load, true);
  activity.synchronize(sessions.read());
  expect(activity.unread(b)).toBe(false);
  expect(changes).toBe(2);
});

test('retain activity across rename and retire closed workflows', () => {
  const activity = new WorkflowOutputActivity();
  const id = workflowSessionId('b');
  activity.synchronize({
    revision: 1,
    open: [{ sessionId: id, title: 'B', path: 'b', generation: 0 }],
    active: null,
    loading: false,
  });
  activity.record(id);
  activity.synchronize({
    revision: 2,
    open: [{ sessionId: id, title: 'Renamed', path: 'new', generation: 0 }],
    active: null,
    loading: false,
  });
  expect(activity.unread(id)).toBe(true);
  activity.synchronize({ revision: 3, open: [], active: null, loading: false });
  activity.record(id);
  expect(activity.unread(id)).toBe(false);
});
