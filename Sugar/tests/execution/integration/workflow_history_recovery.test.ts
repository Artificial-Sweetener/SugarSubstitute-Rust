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
/** Prove exact-job recovery is bounded, cancellable and cannot overwrite newer live results. */

import { jest } from '@jest/globals';
import { promptId } from '../../../frontend/comfyui/ui/execution/domain/ExecutionIdentity.js';
import { ComfyWorkflowHistoryAdapter } from '../../../frontend/comfyui/ui/execution/infrastructure/ComfyWorkflowHistoryAdapter.js';
import { workflowFinal, workflowResultsFixture } from '../support/workflow_results.js';
import { WorkflowHistoryApi, settleWorkflowHistory } from '../support/workflow_history.js';

/** Exercise actual execution ownership with controlled public network responses. */
function fixture() {
  const f = workflowResultsFixture();
  const api = new WorkflowHistoryApi();
  const errors: unknown[] = [];
  const history = new ComfyWorkflowHistoryAdapter({
    api,
    runs: f.runs,
    results: f.results,
    ingestor: f.ingestor,
    logger: {
      error: (...args: unknown[]) => {
        errors.push(args);
      },
    },
  });
  f.ingestor.accepted(f.ingestor.submitting(f.a), f.id);
  return { ...f, api, history, errors };
}

/** Build a backend history entry whose raw job key is demonstrably associated. */
function entry(key = 'one', filename = 'history.png') {
  return { [key]: { outputs: { '42:23': { images: [{ filename }] } } } };
}

test('queries only exact known jobs and deduplicates concurrent requests', async () => {
  const f = fixture();
  expect(await f.history.reconcile(promptId('foreign'))).toBe(0);
  expect(f.api.requests).toHaveLength(0);
  f.ingestor.terminal(f.id, 'complete');
  const recovery = f.history.reconcile(f.id);
  expect(f.history.reconcile(f.id)).toBe(recovery);
  expect(f.api.request(0).path).toBe('/history/one');
  f.api.request(0).respond(entry());
  expect(await recovery).toBe(1);
  await settleWorkflowHistory();
  expect(f.runs.read(f.id)).toBeNull();
  expect(f.results.read(f.a.session.sessionId)?.outputs[0]?.provenance.promptId).toBe(f.id);
  expect(f.results.read(f.a.session.sessionId)?.recovery.state).toBe('ready');
  f.history.dispose();
});

test('admits two requests across workflow tabs and starts the third only after a slot settles', async () => {
  const f = fixture();
  const second = promptId('two');
  const third = promptId('three');
  f.ingestor.accepted(f.ingestor.submitting(f.b), second);
  f.ingestor.accepted(f.ingestor.submitting(f.a), third);
  const promises = [
    f.history.reconcile(f.id),
    f.history.reconcile(second),
    f.history.reconcile(third),
  ];
  expect(f.api.requests).toHaveLength(2);
  f.api.request(1).respond(entry('two', 'b.png'));
  await settleWorkflowHistory();
  expect(f.api.requests).toHaveLength(3);
  f.api.request(0).respond(entry());
  f.api.request(2).respond(entry('three', 'new.png'));
  expect(await Promise.all(promises)).toEqual([1, 1, 1]);
  expect(f.results.read(f.b.session.sessionId)?.outputs[0]?.items[0]?.file.filename).toBe('b.png');
  f.history.dispose();
});

test('late history compare-and-set cannot overwrite a live final received during the request', async () => {
  const f = fixture();
  const recovery = f.history.reconcile(f.id);
  f.ingestor.final(workflowFinal(f.id, 'live.png'));
  f.api.request(0).respond(entry());
  expect(await recovery).toBe(0);
  expect(f.results.read(f.a.session.sessionId)?.outputs[0]?.items[0]?.file.filename).toBe(
    'live.png',
  );
  f.history.dispose();
});

test('missing completed history reports unavailable recovery while preserving valid final media', async () => {
  const f = fixture();
  f.ingestor.final(workflowFinal(f.id, 'live.png'));
  f.ingestor.terminal(f.id, 'complete');
  const recovery = f.history.reconcile(f.id);
  f.api.request(0).respond(entry('wrong-job'));
  expect(await recovery).toBe(0);
  await settleWorkflowHistory();
  expect(f.results.read(f.a.session.sessionId)?.recovery.state).toBe('unavailable');
  expect(f.results.read(f.a.session.sessionId)?.outputs[0]?.items[0]?.file.filename).toBe(
    'live.png',
  );
  expect(f.errors).toHaveLength(1);
  expect(f.runs.read(f.id)).toBeNull();
  f.history.dispose();
});

test('repeats a cached request started before terminal completion rather than retiring the association early', async () => {
  const f = fixture();
  const cached = f.history.reconcile(f.id);
  f.ingestor.terminal(f.id, 'complete');
  expect(f.history.reconcile(f.id)).toBe(cached);
  f.api.request(0).respond({});
  expect(await cached).toBe(0);
  await settleWorkflowHistory();
  expect(f.runs.read(f.id)?.status).toBe('complete');
  expect(f.api.requests).toHaveLength(2);
  f.api.request(1).respond(entry());
  await settleWorkflowHistory();
  expect(f.results.read(f.a.session.sessionId)?.outputs[0]?.items[0]?.file.filename).toBe(
    'history.png',
  );
  expect(f.runs.read(f.id)).toBeNull();
  f.history.dispose();
});

test('workflow closure aborts its active request and queued request without cancelling another workflow', async () => {
  const f = fixture();
  const second = promptId('two');
  const third = promptId('three');
  f.ingestor.accepted(f.ingestor.submitting(f.b), second);
  f.ingestor.accepted(f.ingestor.submitting(f.a), third);
  const one = f.history.reconcile(f.id);
  const two = f.history.reconcile(second);
  const three = f.history.reconcile(third);
  f.history.cancelSession(f.a.session.sessionId);
  f.ingestor.closeSession(f.a.session.sessionId);
  f.results.synchronize([f.b.session]);
  expect(f.api.request(0).signal.aborted).toBe(true);
  expect(f.api.request(1).signal.aborted).toBe(false);
  expect(await one).toBe(0);
  expect(await three).toBe(0);
  f.api.request(0).respond(entry());
  f.api.request(1).respond(entry('two'));
  expect(await two).toBe(1);
  expect(f.results.read(f.a.session.sessionId)).toBeNull();
  expect(f.api.requests).toHaveLength(2);
  expect(f.errors).toEqual([]);
  f.history.dispose();
});

test('times out an unresponsive host transport and releases its admission slot', async () => {
  jest.useFakeTimers();
  const f = fixture();
  try {
    const recovery = f.history.reconcile(f.id);
    jest.advanceTimersByTime(15000);
    expect(await recovery).toBe(0);
    await settleWorkflowHistory();
    expect(f.api.request(0).signal.aborted).toBe(true);
    expect(f.errors).toHaveLength(1);
    expect(jest.getTimerCount()).toBe(0);
  } finally {
    f.history.dispose();
    jest.useRealTimers();
  }
});

test('disposal settles pending callers and ignores late responses and future reconnects', async () => {
  const f = fixture();
  const recovery = f.history.reconcile(f.id);
  f.history.dispose();
  expect(await recovery).toBe(0);
  expect(f.api.request(0).signal.aborted).toBe(true);
  f.api.request(0).respond(entry());
  await settleWorkflowHistory();
  f.history.reconnect();
  expect(await f.history.reconcile(f.id)).toBe(0);
  expect(f.api.requests).toHaveLength(1);
  expect(f.results.read(f.a.session.sessionId)?.outputs).toEqual([]);
  expect(f.errors).toEqual([]);
});

test('reconnect recovers a terminal notification missed during disconnection and retires its known job', async () => {
  const f = fixture();
  f.ingestor.started(f.id);
  f.ingestor.disconnected();
  f.history.reconnect();
  f.api.request(0).respond({
    one: {
      outputs: { '42:23': { images: [{ filename: 'completed.png' }] } },
      status: { completed: true, status_str: 'success' },
    },
  });
  await settleWorkflowHistory();
  expect(f.results.read(f.a.session.sessionId)?.runStatus).toBe('complete');
  expect(f.results.read(f.a.session.sessionId)?.outputs[0]?.items[0]?.file.filename).toBe(
    'completed.png',
  );
  expect(f.runs.read(f.id)).toBeNull();
  f.history.reconnect();
  expect(f.api.requests).toHaveLength(1);
  f.history.dispose();
});
