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
/** Prove the queue shows a finished job's result and a job's details in SugarCubes dialogs. */
import { afterEach, expect, test } from '@jest/globals';
import { GenerationQueueDialogs } from '../../../frontend/comfyui/ui/surface/frame/queue/GenerationQueueDialogs.js';
import type { QueueJob } from '../../../frontend/comfyui/ui/surface/frame/queue/GenerationQueueRows.js';

afterEach(() => document.body.replaceChildren());

function job(extra: Partial<QueueJob>): QueueJob {
  return {
    id: 'job-1',
    status: 'completed',
    priority: 0,
    createTime: 1000,
    startTime: 2000,
    endTime: 3000,
    workflowName: 'Demo',
    runNumber: null,
    workflowId: null,
    outputFile: '<b>a</b>.png',
    outputSubfolder: null,
    outputKind: 'image',
    previewUrl: '/api/view?filename=a.png',
    outputsCount: 1,
    error: null,
    errorDetail: null,
    ...extra,
  };
}

test('a finished image opens in a dialog titled with its file name, shown literally', () => {
  const dialogs = new GenerationQueueDialogs(document);
  expect(dialogs.inspect(job({}))).toBe(true);
  const dialog = document.querySelector('.sc-generation-queue-asset-dialog');
  expect(dialog?.textContent).toContain('<b>a</b>.png');
  expect(dialog?.querySelector('b')).toBeNull();
  const image = dialog?.querySelector<HTMLImageElement>('img.sc-generation-queue-asset');
  expect(image?.getAttribute('src')).toBe('/api/view?filename=a.png');
  dialogs.dispose();
  expect(document.querySelector('.sugarcubes-modal-overlay.is-visible')).toBeNull();
});

test('a video plays with controls, and a job with nothing to show opens nothing', () => {
  const dialogs = new GenerationQueueDialogs(document);
  expect(dialogs.inspect(job({ status: 'failed' }))).toBe(false);
  expect(dialogs.inspect(job({ previewUrl: null }))).toBe(false);
  expect(document.querySelector('.sc-generation-queue-asset-dialog')).toBeNull();
  dialogs.inspect(job({ outputKind: 'video' }));
  const video = document.querySelector<HTMLVideoElement>('video.sc-generation-queue-asset');
  expect(video?.controls).toBe(true);
  dialogs.dispose();
});

test('job details list what is known about the job', () => {
  const dialogs = new GenerationQueueDialogs(document);
  dialogs.details(job({}), []);
  const text = document.querySelector('.sugarcubes-modal-overlay.is-visible')?.textContent ?? '';
  expect(text).toContain('Job details');
  expect(text).toContain('Job ID: job-1');
  expect(text).toContain('Total generation time: 1.0s');
});
