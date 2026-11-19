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
/** Prove the generation queue orders and words its rows as SugarSubstitute's queue always has. */
import { expect, test } from '@jest/globals';
import {
  DEFAULT_QUEUE_VIEW,
  NO_QUEUE_PROGRESS,
  formatDuration,
  hasFailedJobs,
  pendingJobCount,
  queueItems,
  queueSummary,
  type QueueItem,
  type QueueJob,
} from '../../../frontend/comfyui/ui/surface/frame/queue/GenerationQueueRows.js';

function job(id: string, status: QueueJob['status'], extra: Partial<QueueJob> = {}): QueueJob {
  return {
    id,
    status,
    priority: 0,
    createTime: 0,
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

/** A fixed moment and plain wording, so day headings and times do not depend on the machine. */
const CLOCK = {
  now: 300_000,
  time: (moment: number) => `t${String(moment)}`,
  date: (moment: number) => `d${String(moment)}`,
};

/** Lay jobs out with the running job at `percent` and nothing else reported. */
function items(
  jobs: readonly QueueJob[],
  percent: number | null,
  view: Partial<typeof DEFAULT_QUEUE_VIEW> = {},
): readonly QueueItem[] {
  return queueItems(
    jobs,
    { ...NO_QUEUE_PROGRESS, percent },
    { ...DEFAULT_QUEUE_VIEW, ...view },
    CLOCK,
  );
}

/** Read the queue as a person would: one line per row or caption. */
function lines(items: readonly QueueItem[]): string[] {
  return items.map((item) =>
    item.kind === 'caption'
      ? `-- ${item.text}`
      : `${item.row.title} | ${item.row.subtitle} | ${item.row.role} | ${item.row.action}`,
  );
}

test('waiting jobs sit newest first above the running job, and finished jobs follow newest first', () => {
  const jobs = [
    job('done-old', 'completed', { runNumber: 1, startTime: 1000, endTime: 13_300 }),
    job('done-new', 'completed', { runNumber: 2, startTime: 20_000, endTime: 265_000 }),
    job('run', 'running', { runNumber: 3 }),
    job('next', 'pending', { priority: 4, runNumber: 4 }),
    job('later', 'pending', { priority: 5, runNumber: 5 }),
    job('last', 'pending', { priority: 6, runNumber: 6 }),
  ];
  expect(lines(items(jobs, 45))).toEqual([
    'Demo #006 | Waiting - 2 ahead | waiting | cancel',
    'Demo #005 | Waiting - 1 ahead | waiting | cancel',
    'Demo #004 | Next | waiting | cancel',
    'Demo #003 | 45% complete | active | cancel',
    '-- Today',
    'Demo #002 | Completed, 4m5s | resolved | remove',
    'Demo #001 | Completed, 12.3s | resolved | remove',
  ]);
  expect(pendingJobCount(jobs)).toBe(3);
  // Until ComfyUI reports progress the running job simply says so.
  expect(lines(items([job('run', 'running')], null))).toEqual(['Demo | Running | active | cancel']);
});

test('with nothing running, a caption separates waiting jobs from finished ones', () => {
  const jobs = [job('wait', 'pending'), job('done', 'completed')];
  expect(lines(items(jobs, null))).toEqual([
    'Demo | Next | waiting | cancel',
    '-- Resolved - Today',
    'Demo | Completed | resolved | remove',
  ]);
  expect(lines(items([job('done', 'completed')], null))).toEqual([
    'Demo | Completed | resolved | remove',
  ]);
});

test('finished jobs say how they ended', () => {
  const long = 'x'.repeat(80);
  const jobs = [
    job('a', 'cancelled', { endTime: 5 }),
    job('b', 'cancelled', { endTime: 4, outputsCount: 1 }),
    job('c', 'cancelled', { endTime: 3, outputsCount: 3 }),
    job('d', 'failed', { endTime: 2, error: 'Out of\n memory' }),
    job('e', 'failed', { endTime: 1, error: long }),
    job('f', 'failed', { endTime: 0 }),
  ];
  expect(items(jobs, null).map((item) => (item.kind === 'row' ? item.row.subtitle : ''))).toEqual([
    'Cancelled - No outputs saved',
    'Cancelled - 1 output saved',
    'Cancelled - 3 outputs saved',
    'Failed - Out of memory',
    `Failed - ${'x'.repeat(64)}...`,
    'Generation failed',
  ]);
  expect(hasFailedJobs(jobs)).toBe(true);
  expect(hasFailedJobs([job('g', 'completed')])).toBe(false);
});

test('a job the page has no workflow name for is named as ComfyUI names it', () => {
  const named = job('0123456789abcdef', 'completed', {
    workflowName: null,
    outputFile: 'out_00001_.png',
  });
  const bare = job('0123456789abcdef', 'pending', { workflowName: ' ' });
  expect(lines(items([named], null))[0]).toContain('out_00001_.png |');
  expect(lines(items([bare], null))[0]).toContain('Job 01234567 |');
});

test('the tabs, workflow filter, sort and search ComfyUI offers choose which jobs are listed', () => {
  const jobs = [
    job('p1', 'pending', { priority: 1, workflowId: 'w1' }),
    job('p2', 'pending', { priority: 2, workflowId: 'w2', workflowName: 'Other' }),
    job('c1', 'completed', { workflowId: 'w1', startTime: 0, endTime: 1000, runNumber: 1 }),
    job('c2', 'completed', {
      workflowId: 'w2',
      startTime: 0,
      endTime: 9000,
      workflowName: 'Other',
    }),
    job('f1', 'failed', { workflowId: 'w1', endTime: 20_000 }),
  ];
  const ids = (view: Partial<typeof DEFAULT_QUEUE_VIEW>): string[] =>
    items(jobs, null, view).flatMap((item) => (item.kind === 'row' ? [item.row.jobId] : []));
  expect(ids({})).toEqual(['p2', 'p1', 'f1', 'c2', 'c1']);
  expect(ids({ tab: 'completed' })).toEqual(['c2', 'c1']);
  expect(ids({ tab: 'failed' })).toEqual(['f1']);
  expect(ids({ workflow: 'current', currentWorkflowId: 'w1' })).toEqual(['p1', 'f1', 'c1']);
  expect(ids({ sort: 'duration' })).toEqual(['p2', 'p1', 'c2', 'c1', 'f1']);
  expect(ids({ search: 'other' })).toEqual(['p2', 'c2']);
  expect(ids({ search: 'F1' })).toEqual(['f1']);
  // A filter never changes how many jobs are really ahead of a waiting one.
  const second = items(jobs, null, { search: 'other' })[0];
  expect(second?.kind === 'row' ? second.row.subtitle : '').toBe('Waiting - 1 ahead');
});

test('durations read as seconds, minutes and seconds, or hours and minutes', () => {
  expect(formatDuration(12_340)).toBe('12.3s');
  expect(formatDuration(245_000)).toBe('4m5s');
  expect(formatDuration(3_720_000)).toBe('1h2m');
  expect(formatDuration(-5)).toBe('0.0s');
});

test('each row also carries what ComfyUI says beside the status', () => {
  const jobs = [
    job('wait', 'pending', { createTime: 42 }),
    job('run', 'running', { previewUrl: null }),
    job('done', 'completed', { endTime: 10, outputFile: 'out_00001_.png', previewUrl: '/v/a' }),
  ];
  const rows = queueItems(
    jobs,
    { percent: 45, node: 'KSampler', nodePercent: 30, previewUrl: 'blob:live' },
    DEFAULT_QUEUE_VIEW,
    CLOCK,
  ).flatMap((item) => (item.kind === 'row' ? [item.row] : []));
  expect(rows.map((row) => `${row.subtitle} / ${row.detail} / ${String(row.previewUrl)}`)).toEqual([
    'Next / t42 / null',
    '45% complete / KSampler: 30% / blob:live',
    'Completed / out_00001_.png / /v/a',
  ]);
  expect(queueSummary(jobs)).toBe('1 running, 1 queued');
  expect(queueSummary([job('done', 'completed')])).toBe('');
});

test('finished jobs sit under the day they ended', () => {
  const day = 86_400_000;
  const now = 10 * day + 1000;
  const jobs = [
    job('today', 'completed', { endTime: now - 500 }),
    job('yesterday', 'completed', { endTime: now - day }),
    job('older', 'completed', { endTime: now - 3 * day }),
    job('undated', 'failed', { endTime: null, createTime: null }),
  ];
  const listed = queueItems(jobs, NO_QUEUE_PROGRESS, DEFAULT_QUEUE_VIEW, { ...CLOCK, now });
  expect(lines(listed).map((line) => line.split(' | ')[0])).toEqual([
    '-- Today',
    'Demo',
    '-- Yesterday',
    'Demo',
    `-- d${String(now - 3 * day)}`,
    'Demo',
    '-- Undated',
    'Demo',
  ]);
});
