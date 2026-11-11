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
/** Prove event ordering and bounded early metadata cannot cross workflow ownership. */

import { promptId } from '../../../frontend/comfyui/ui/execution/domain/ExecutionIdentity.js';
import { workflowFinal, workflowResultsFixture } from '../support/workflow_results.js';

test('replays finals received before acceptance and recovers only the accepted successful job', () => {
  const f = workflowResultsFixture();
  f.ingestor.started(f.id);
  expect(f.ingestor.final(workflowFinal(f.id))).toBe(false);
  f.ingestor.terminal(f.id, 'complete');
  expect(f.results.read(f.a.session.sessionId)?.outputs).toEqual([]);
  expect(f.recoveries).toEqual([]);
  f.ingestor.accepted(f.ingestor.submitting(f.a), f.id);
  expect(f.results.read(f.a.session.sessionId)?.outputs[0]?.items[0]?.file.filename).toBe(
    'final.png',
  );
  expect(f.results.read(f.a.session.sessionId)?.runStatus).toBe('complete');
  expect(f.recoveries).toEqual([f.id]);
  expect(f.clock.pending).toBe(0);
  f.ingestor.started(f.id);
  expect(f.results.read(f.a.session.sessionId)?.runStatus).toBe('complete');
});

test('keeps interleaved jobs with colliding paths in their captured workflows', () => {
  const f = workflowResultsFixture();
  const other = promptId('two');
  f.ingestor.accepted(f.ingestor.submitting(f.a), f.id);
  f.ingestor.accepted(f.ingestor.submitting(f.b), other);
  f.ingestor.final(workflowFinal(other, 'b.png'));
  f.ingestor.final(workflowFinal(f.id, 'a.png'));
  expect(f.results.read(f.a.session.sessionId)?.outputs[0]?.items[0]?.file.filename).toBe('a.png');
  expect(f.results.read(f.b.session.sessionId)?.outputs[0]?.items[0]?.file.filename).toBe('b.png');
  f.ingestor.final({ ...workflowFinal(f.id), executionPaths: ['23'] });
  expect(f.results.read(f.a.session.sessionId)?.outputs[0]?.items[0]?.file.filename).toBe('a.png');
});

test('foreign progress cannot create an association with another workflow job', () => {
  const f = workflowResultsFixture();
  f.ingestor.accepted(f.ingestor.submitting(f.a), f.id);
  f.ingestor.started(f.id);
  f.ingestor.executing(f.id, ['42:23']);
  expect(f.ingestor.resolveSource(f.id, ['42:23'])?.run.promptId).toBe(f.id);
  f.ingestor.executing(promptId('foreign'), ['42:23']);
  expect(f.ingestor.resolveSource(promptId('foreign'), ['42:23'])).toBeNull();
  f.ingestor.executing(f.id, ['42:23']);
  f.ingestor.disconnected();
  expect(f.results.read(f.a.session.sessionId)?.preview).toBeNull();
});

test('late acceptance preserves its captured identity independently of foreign job starts', () => {
  const f = workflowResultsFixture();
  f.ingestor.started(f.id);
  f.ingestor.started(promptId('foreign'));
  f.ingestor.accepted(f.ingestor.submitting(f.a), f.id);
  expect(f.ingestor.resolveSource(promptId('foreign'), ['42:23'])).toBeNull();
  expect(f.ingestor.resolveSource(f.id, ['42:23'])?.run.promptId).toBe(f.id);
});

test('bounds early final deliveries and evicts old unknown jobs without retaining timers indefinitely', () => {
  const f = workflowResultsFixture();
  for (let index = 0; index < 80; index++) f.ingestor.final(workflowFinal(f.id, `${index}.png`));
  f.ingestor.accepted(f.ingestor.submitting(f.a), f.id);
  expect(f.results.read(f.a.session.sessionId)?.outputs[0]?.items[0]?.file.filename).toBe('63.png');
  expect(f.warnings).toHaveLength(16);
  const oldest = promptId('oldest');
  f.ingestor.final(workflowFinal(oldest));
  for (let index = 0; index < 16; index++) f.ingestor.started(promptId(`foreign-${index}`));
  f.ingestor.accepted(f.ingestor.submitting(f.b), oldest);
  expect(f.results.read(f.b.session.sessionId)?.outputs).toEqual([]);
  expect(f.clock.pending).toBe(1);
  f.clock.advance(30000);
  expect(f.clock.pending).toBe(0);
});

test('expires early events after thirty seconds and releases metadata immediately on disposal', () => {
  const f = workflowResultsFixture();
  f.ingestor.final(workflowFinal(f.id));
  f.clock.advance(30000);
  f.ingestor.accepted(f.ingestor.submitting(f.a), f.id);
  expect(f.results.read(f.a.session.sessionId)?.outputs).toEqual([]);
  f.ingestor.started(promptId('unknown'));
  const late = f.ingestor.submitting(f.a);
  f.ingestor.dispose();
  expect(f.clock.pending).toBe(0);
  f.ingestor.accepted(late, promptId('late'));
  f.ingestor.started(promptId('late'));
  f.ingestor.terminal(promptId('late'), 'complete');
  expect(f.runs.read(promptId('late'))).toBeNull();
  expect(f.clock.pending).toBe(0);
  expect(f.recoveries).toEqual([]);
});

test('reserves result order at submission even when accepted responses arrive in reverse order', () => {
  const f = workflowResultsFixture();
  const first = f.ingestor.submitting(f.a);
  const second = f.ingestor.submitting(f.a);
  const newer = promptId('newer');
  f.ingestor.accepted(second, newer);
  f.ingestor.final(workflowFinal(newer, 'newer.png'));
  f.ingestor.accepted(first, f.id);
  expect(f.ingestor.final(workflowFinal(f.id, 'older.png'))).toBe(false);
  expect(f.results.read(f.a.session.sessionId)?.runOrder).toBe(second.order);
  expect(f.results.read(f.a.session.sessionId)?.outputs[0]?.items[0]?.file.filename).toBe(
    'newer.png',
  );
});

test('closing a workflow and loading a new generation both reject accepted-job late media', () => {
  const f = workflowResultsFixture();
  f.ingestor.accepted(f.ingestor.submitting(f.a), f.id);
  f.ingestor.closeSession(f.a.session.sessionId);
  f.results.synchronize([f.b.session]);
  expect(f.ingestor.final(workflowFinal(f.id))).toBe(false);
  f.ingestor.terminal(f.id, 'complete');
  expect(f.recoveries).toEqual([]);
  f.results.synchronize([{ ...f.a.session, generation: 1 }]);
  f.ingestor.accepted(f.ingestor.submitting(f.a), promptId('obsolete'));
  expect(f.runs.read(promptId('obsolete'))).toBeNull();
  expect(f.results.read(f.a.session.sessionId)?.outputs).toEqual([]);
});
