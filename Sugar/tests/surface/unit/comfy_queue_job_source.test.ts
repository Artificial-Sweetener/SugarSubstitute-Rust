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
/** Prove the queue reads ComfyUI's jobs, their workflow names and the running job's progress. */
import { expect, jest, test } from '@jest/globals';
import {
  ComfyQueueJobSource,
  type ComfyQueueApi,
  type QueueSnapshot,
} from '../../../frontend/comfyui/ui/surface/frame/queue/ComfyQueueJobSource.js';

const SAVING = (name: string) => ({ sugarcubes_output_saving: { workflow_name: name } });

/** A ComfyUI that answers from `routes` and lets a test raise its events. */
function comfy(routes: Record<string, unknown>) {
  const listeners = new Map<string, Set<(event: Event) => void>>();
  const asked: string[] = [];
  const api: ComfyQueueApi = {
    fetchApi: async (route) => {
      asked.push(route);
      const body = routes[route];
      return {
        ok: body !== undefined,
        status: body === undefined ? 404 : 200,
        json: async () => body,
      } as Response;
    },
    addEventListener: (type, listener) => {
      if (!listeners.has(type)) listeners.set(type, new Set());
      listeners.get(type)?.add(listener);
    },
    removeEventListener: (type, listener) => void listeners.get(type)?.delete(listener),
  };
  const raise = (type: string, detail?: unknown): void => {
    const event = new Event(type);
    Object.defineProperty(event, 'detail', { value: detail });
    for (const listener of [...(listeners.get(type) ?? [])]) listener(event);
  };
  const listening = (): number =>
    [...listeners.values()].reduce((total, set) => total + set.size, 0);
  return { api, asked, raise, listening, routes };
}

const ACTIVE = '/jobs?status=in_progress,pending&limit=200&offset=0';
const HISTORY = '/jobs?status=completed,failed,cancelled&limit=64&offset=0';
const settle = async (): Promise<void> => {
  for (let turn = 0; turn < 60; turn += 1) await Promise.resolve();
};

test('jobs carry the workflow name their page recorded, read once per job', async () => {
  const host = comfy({
    [ACTIVE]: {
      jobs: [
        { id: 'run', status: 'in_progress', priority: 1, create_time: 10 },
        { id: 'wait', status: 'pending', priority: 2, create_time: 20 },
      ],
    },
    [HISTORY]: {
      jobs: [
        {
          id: 'done',
          status: 'completed',
          priority: 0,
          create_time: 1,
          execution_start_time: 2,
          execution_end_time: 5002,
          outputs_count: 1,
          workflow_id: 'w1',
          preview_output: { filename: 'a b.png', type: 'output', subfolder: 'x' },
        },
        {
          id: 'bad',
          status: 'failed',
          execution_error: { exception_message: 'Out of memory' },
        },
        { id: 7, status: 'completed' },
        { id: 'odd', status: 'unheard-of' },
      ],
    },
    '/queue': {
      queue_running: [[1, 'run', {}, SAVING('Demo')]],
      queue_pending: [[2, 'wait', {}, {}]],
    },
    '/jobs/done': { workflow: { extra_data: SAVING(' Portrait ') } },
  });
  const report = jest.fn();
  const source = new ComfyQueueJobSource(host.api, report);
  const seen: QueueSnapshot[] = [];
  const stop = source.subscribe((snapshot) => seen.push(snapshot));
  await settle();

  const jobs = source.snapshot.jobs;
  expect(jobs.map((job) => `${job.id}:${job.status}:${String(job.workflowName)}`)).toEqual([
    'run:running:Demo',
    'wait:pending:null',
    'done:completed:Portrait',
    'bad:failed:null',
  ]);
  // ComfyUI's own count of queued jobs numbers each run; a job without a place in it has none.
  expect(jobs.map((job) => job.runNumber)).toEqual([1, 2, null, null]);
  expect(jobs[2]).toMatchObject({
    startTime: 2,
    endTime: 5002,
    outputsCount: 1,
    workflowId: 'w1',
    outputFile: 'a b.png',
    previewUrl: '/api/view?filename=a+b.png&type=output&subfolder=x',
  });
  expect(jobs[3]?.error).toBe('Out of memory');
  // Two jobs had no readable record: both were reported, and neither hid the queue.
  expect(report).toHaveBeenCalledTimes(2);

  host.asked.length = 0;
  host.raise('status');
  await settle();
  // Names already learned are not asked for again.
  expect(host.asked.sort()).toEqual([ACTIVE, HISTORY].sort());
  expect(seen.length).toBeGreaterThanOrEqual(2);

  stop();
  expect(host.listening()).toBe(0);
});

test('progress is the share of the running job’s nodes that are done', async () => {
  const host = comfy({
    [ACTIVE]: { jobs: [{ id: 'run', status: 'in_progress' }] },
    [HISTORY]: { jobs: [] },
    '/queue': { queue_running: [[1, 'run', {}, {}]], queue_pending: [] },
  });
  const urls = { createObjectURL: jest.fn(() => 'blob:live'), revokeObjectURL: jest.fn() };
  const source = new ComfyQueueJobSource(
    host.api,
    jest.fn(),
    (id) => (id === 'b' ? 'KSampler' : null),
    urls,
  );
  const stop = source.subscribe(() => undefined);
  await settle();
  expect(source.snapshot.progress.percent).toBeNull();

  host.raise('progress_state', {
    nodes: {
      a: { state: 'finished' },
      b: { state: 'running', value: 5, max: 10 },
      c: { state: 'pending' },
      d: { state: 'pending' },
    },
  });
  expect(source.snapshot.progress).toMatchObject({
    percent: 38,
    node: 'KSampler',
    nodePercent: 50,
  });

  // The picture ComfyUI sends as the job forms becomes the running row's thumbnail.
  host.raise('b_preview_with_metadata', { blob: new Blob(['x']) });
  expect(source.snapshot.progress.previewUrl).toBe('blob:live');

  // Once nothing is running the figure is dropped rather than left on the next job.
  host.routes[ACTIVE] = { jobs: [] };
  host.raise('execution_success');
  await settle();
  expect(source.snapshot.progress).toEqual({
    percent: null,
    node: null,
    nodePercent: null,
    previewUrl: null,
  });
  expect(urls.revokeObjectURL).toHaveBeenCalledWith('blob:live');
  stop();
});

test('a failed read is reported and the last good queue stays', async () => {
  const host = comfy({
    [ACTIVE]: { jobs: [{ id: 'wait', status: 'pending' }] },
    [HISTORY]: { jobs: [] },
    '/queue': { queue_running: [], queue_pending: [[1, 'wait', {}, SAVING('Demo')]] },
  });
  const report = jest.fn();
  const source = new ComfyQueueJobSource(host.api, report);
  const stop = source.subscribe(() => undefined);
  await settle();
  expect(source.snapshot.jobs).toHaveLength(1);

  Reflect.deleteProperty(host.routes, HISTORY);
  host.raise('status');
  await settle();
  expect(report).toHaveBeenCalledTimes(1);
  expect(source.snapshot.jobs.map((job) => job.workflowName)).toEqual(['Demo']);
  stop();
});
