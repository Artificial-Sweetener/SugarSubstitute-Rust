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
/** Prove only the executing run's newest preview is shown, and a stopped run keeps its last one. */

import { WorkflowResultStore } from '../../../frontend/comfyui/ui/execution/application/WorkflowResultStore.js';
import { WorkflowRunRegistry } from '../../../frontend/comfyui/ui/execution/application/WorkflowRunRegistry.js';
import {
  outputSourceId,
  promptId,
  workflowSessionId,
} from '../../../frontend/comfyui/ui/execution/domain/ExecutionIdentity.js';
import type { WorkflowSessionContext } from '../../../frontend/comfyui/ui/execution/domain/WorkflowSession.js';
import type { WorkflowRun } from '../../../frontend/comfyui/ui/execution/domain/WorkflowRun.js';
import type { WorkflowOutputSource } from '../../../frontend/comfyui/ui/execution/domain/WorkflowOutput.js';
import type { WorkflowPreview } from '../../../frontend/comfyui/ui/execution/domain/WorkflowPreview.js';
import { decodeComfyMedia } from '../../../frontend/comfyui/ui/execution/infrastructure/ComfyMediaDecoder.js';

const session: WorkflowSessionContext = {
  sessionId: workflowSessionId('a'),
  title: 'a',
  path: 'a.json',
  generation: 0,
};
const source: WorkflowOutputSource = {
  id: outputSourceId('9'),
  executionPath: '9',
  label: 'Preview',
  breadcrumb: [],
  order: 0,
};

/** Build a frame of `run` at `sequence`. */
function frame(run: WorkflowRun, sequence: number): WorkflowPreview {
  return {
    sessionId: session.sessionId,
    generation: session.generation,
    promptId: run.promptId,
    sourceId: source.id,
    runOrder: run.order,
    sequence,
    token: `${run.promptId}:${sequence}`,
    dimensions: null,
  };
}

/** Accept a job and report it running, as ComfyUI's execution start does. */
function start(store: WorkflowResultStore, runs: WorkflowRunRegistry, id: string): WorkflowRun {
  const accepted = runs.accept(runs.reserve({ session, sources: [source] }), promptId(id));
  store.updateRun(accepted);
  const running = runs.setStatus(accepted.promptId, 'running');
  if (!running) throw new Error(`Run ${id} was not accepted`);
  store.updateRun(running);
  return running;
}

/** Create a store with the test workflow open. */
function setup() {
  const store = new WorkflowResultStore();
  store.synchronize([session]);
  return { store, runs: new WorkflowRunRegistry() };
}

test('a run stopped early keeps its last preview and refuses anything after it', () => {
  const { store, runs } = setup();
  const run = start(store, runs, 'one');
  expect(store.publishPreview(frame(run, 1))).toBe(true);
  expect(store.publishPreview(frame(run, 2))).toBe(true);
  const stopped = runs.setStatus(run.promptId, 'cancelled');
  if (!stopped) throw new Error('Run one was not stopped');
  store.updateRun(stopped);
  expect(store.read(session.sessionId)?.preview?.token).toBe(`${run.promptId}:2`);
  expect(store.publishPreview(frame(run, 3))).toBe(false);
  expect(store.read(session.sessionId)?.preview?.token).toBe(`${run.promptId}:2`);
});

test('a run that fails after previews keeps its last preview', () => {
  const { store, runs } = setup();
  const run = start(store, runs, 'one');
  expect(store.publishPreview(frame(run, 1))).toBe(true);
  store.updateRun({ ...run, status: 'error' });
  expect(store.read(session.sessionId)?.preview?.token).toBe(`${run.promptId}:1`);
});

test('a late preview from the previous run never reaches the next run', () => {
  const { store, runs } = setup();
  const first = start(store, runs, 'one');
  expect(store.publishPreview(frame(first, 1))).toBe(true);
  const finished = runs.setStatus(first.promptId, 'complete');
  if (!finished) throw new Error('Run one did not finish');
  store.updateRun(finished);
  const second = start(store, runs, 'two');
  expect(store.publishPreview(frame(first, 2))).toBe(false);
  expect(store.publishPreview(frame(second, 1))).toBe(true);
  expect(store.publishPreview(frame(first, 3))).toBe(false);
  expect(store.read(session.sessionId)?.preview?.promptId).toBe(second.promptId);
});

test('a queued job cannot show a preview while another job executes', () => {
  const { store, runs } = setup();
  const running = start(store, runs, 'one');
  const queued = runs.accept(runs.reserve({ session, sources: [source] }), promptId('two'));
  store.updateRun(queued);
  expect(store.publishPreview(frame(queued, 1))).toBe(false);
  expect(store.publishPreview(frame(running, 1))).toBe(true);
  expect(store.read(session.sessionId)?.preview?.promptId).toBe(running.promptId);
});

test('an older preview of the executing run arriving after a newer one is refused', () => {
  const { store, runs } = setup();
  const run = start(store, runs, 'one');
  expect(store.publishPreview(frame(run, 3))).toBe(true);
  expect(store.publishPreview(frame(run, 2))).toBe(false);
  expect(store.publishPreview(frame(run, 3))).toBe(false);
  expect(store.read(session.sessionId)?.preview?.sequence).toBe(3);
});

test('the next run replaces a kept preview with its own first preview', () => {
  const { store, runs } = setup();
  const first = start(store, runs, 'one');
  expect(store.publishPreview(frame(first, 1))).toBe(true);
  store.updateRun({ ...first, status: 'cancelled' });
  const second = start(store, runs, 'two');
  // The stopped run's image stays until the next run has one of its own to show.
  expect(store.read(session.sessionId)?.preview?.promptId).toBe(first.promptId);
  expect(store.publishPreview(frame(first, 2))).toBe(false);
  expect(store.publishPreview(frame(second, 1))).toBe(true);
  expect(store.read(session.sessionId)?.preview?.promptId).toBe(second.promptId);
  const media = decodeComfyMedia({ images: [{ filename: 'two.png' }] });
  if (!media) throw new Error('Invalid test media');
  store.retain({ run: second, source, ...media, merge: false });
  expect(store.read(session.sessionId)?.preview).toBeNull();
});
