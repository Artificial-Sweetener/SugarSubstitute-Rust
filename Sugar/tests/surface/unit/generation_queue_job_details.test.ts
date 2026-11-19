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
/** Prove a job's details say where it stands or how it ended, from ComfyUI's own facts. */
import { expect, test } from '@jest/globals';
import { jobDetailLines } from '../../../frontend/comfyui/ui/surface/frame/queue/GenerationQueueJobDetails.js';
import type {
  QueueClock,
  QueueJob,
} from '../../../frontend/comfyui/ui/surface/frame/queue/GenerationQueueRows.js';

function job(id: string, status: QueueJob['status'], extra: Partial<QueueJob> = {}): QueueJob {
  return {
    id,
    status,
    priority: 0,
    createTime: 1000,
    startTime: null,
    endTime: null,
    workflowName: 'Demo',
    runNumber: null,
    workflowId: null,
    outputFile: null,
    outputSubfolder: null,
    outputKind: null,
    previewUrl: null,
    outputsCount: 0,
    error: null,
    errorDetail: null,
    ...extra,
  };
}

const clock: QueueClock = {
  now: 100_000,
  time: (moment) => `t${String(moment)}`,
  date: (moment) => `d${String(moment)}`,
};

test('a waiting job says its place and, once a job has finished, when it should start', () => {
  const waiting = job('w', 'pending', { priority: 5 });
  const others = [
    job('run', 'running', { startTime: 90_000 }),
    job('ahead', 'pending', { priority: 4 }),
    job('behind', 'pending', { priority: 6 }),
  ];
  expect(jobDetailLines(waiting, [waiting, ...others], clock)).toEqual([
    'Workflow: Demo',
    'Job ID: w',
    'Queued at: d1000 t1000',
    'Queue position: 3',
  ]);
  const done = [
    job('a', 'completed', { startTime: 0, endTime: 10_000 }),
    job('b', 'completed', { startTime: 0, endTime: 30_000 }),
  ];
  expect(jobDetailLines(waiting, [waiting, ...others, ...done], clock).at(-1)).toBe(
    'Estimated to start in: 40.0s',
  );
});

test('a running job says how long it has run', () => {
  const running = job('r', 'running', { startTime: 40_000, workflowName: null });
  expect(jobDetailLines(running, [running], clock)).toEqual([
    'Job ID: r',
    'Queued at: d1000 t1000',
    'Time elapsed: 1m0s',
  ]);
});

test('a finished job says when and how it ended', () => {
  const done = job('c', 'completed', { startTime: 2000, endTime: 14_300, outputFile: 'a.png' });
  expect(jobDetailLines(done, [done], clock).slice(3)).toEqual([
    'Generated on: d14300 t14300',
    'Total generation time: 12.3s',
    'Output: a.png',
  ]);
  const failed = job('f', 'failed', { startTime: 2000, endTime: 5000, error: '<b>Out of memory</b>' });
  expect(jobDetailLines(failed, [failed], clock).slice(3)).toEqual([
    'Failed after: 3.0s',
    'Error message: <b>Out of memory</b>',
  ]);
  const cancelled = job('x', 'cancelled', { startTime: 2000, endTime: 4000 });
  expect(jobDetailLines(cancelled, [cancelled], clock).at(-1)).toBe('Cancelled after: 2.0s');
});
