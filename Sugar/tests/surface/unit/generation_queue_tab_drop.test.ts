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
/** Prove a finished job dragged onto the tab bar is previewed there and opens where it is dropped. */
import { expect, jest, test } from '@jest/globals';
import type { QueueJob } from '../../../frontend/comfyui/ui/surface/frame/queue/GenerationQueueRows.js';
import {
  jobOpensAsTab,
  queueTabDropTarget,
} from '../../../frontend/comfyui/ui/surface/frame/queue/GenerationQueueTabDrop.js';

function job(id: string, status: QueueJob['status'], workflowName: string | null): QueueJob {
  return {
    id,
    status,
    priority: 0,
    createTime: 0,
    startTime: null,
    endTime: null,
    workflowName,
    runNumber: null,
    workflowId: null,
    outputFile: null,
    outputSubfolder: null,
    outputKind: null,
    previewUrl: null,
    outputsCount: 0,
    error: null,
    errorDetail: null,
  };
}

function fixture() {
  const jobs = [
    job('done', 'completed', 'Portrait'),
    job('failed-0123456789', 'failed', null),
    job('waiting', 'pending', 'Portrait'),
    job('running', 'running', 'Portrait'),
  ];
  const tabBar = {
    hit: jest.fn((_x: number, y: number): number | null => (y < 30 ? 2 : null)),
    preview: jest.fn(),
    place: jest.fn(),
  };
  const order: string[] = [];
  const open = jest.fn(async (opened: QueueJob) => {
    order.push(`open ${opened.id}`);
  });
  tabBar.place.mockImplementation((index: unknown) => void order.push(`place ${String(index)}`));
  const failures: string[] = [];
  let settled: Promise<void> = Promise.resolve();
  const target = queueTabDropTarget({
    tabBar,
    job: (id) => jobs.find((candidate) => candidate.id === id) ?? null,
    open,
    run: (failure, action) => {
      settled = Promise.resolve()
        .then(action)
        .catch(() => void failures.push(failure));
    },
  });
  return { target, tabBar, open, order, failures, settled: () => settled };
}

test('only a job that has left the queue can open as a tab', () => {
  const f = fixture();
  expect(jobOpensAsTab(null)).toBe(false);
  expect(f.target.hit(10, 10, 'done')).toEqual({ index: 2 });
  expect(f.target.hit(10, 10, 'failed-0123456789')).toEqual({ index: 2 });
  expect(f.target.hit(10, 10, 'waiting')).toBeNull();
  expect(f.target.hit(10, 10, 'running')).toBeNull();
  expect(f.target.hit(10, 10, 'gone')).toBeNull();
  // Off the bar there is nothing to drop on.
  expect(f.target.hit(10, 200, 'done')).toBeNull();
});

test('the bar previews the tab under the title it will have and withdraws it', () => {
  const f = fixture();
  f.target.show('done', { index: 2 });
  expect(f.tabBar.preview).toHaveBeenLastCalledWith('Portrait', 2);
  f.target.show('failed-0123456789', { index: 0 });
  expect(f.tabBar.preview).toHaveBeenLastCalledWith('Job failed-0', 0);
  f.target.show('done', null);
  expect(f.tabBar.preview).toHaveBeenLastCalledWith(null, 0);
});

test('a drop opens the workflow and then moves its new tab to where it was dropped', async () => {
  const f = fixture();
  f.target.accept('done', { index: 1 });
  await f.settled();
  expect(f.order).toEqual(['open done', 'place 1']);
  // A workflow ComfyUI refuses to open is reported and its tab is not placed.
  f.open.mockRejectedValueOnce(new Error('gone'));
  f.target.accept('done', { index: 0 });
  await f.settled();
  expect(f.failures).toEqual(['Workflow not opened']);
  expect(f.order).toEqual(['open done', 'place 1']);
});
