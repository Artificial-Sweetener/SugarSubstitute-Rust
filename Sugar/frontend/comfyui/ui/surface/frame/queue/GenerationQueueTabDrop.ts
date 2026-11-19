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
/** Let a finished job be dragged from the generation queue onto the workflow tab bar. */
import type { ReorderDropTarget } from '../../../interaction/reorder/ReorderGesture.js';
import type { WorkflowTabDropTarget } from '../WorkflowTabGhost.js';
import type { QueueJob } from './GenerationQueueRows.js';

/** What the drop needs from the queue's owner. */
export interface QueueTabDropHost {
  readonly tabBar: WorkflowTabDropTarget;
  job(jobId: string): QueueJob | null;
  /** Open the job's workflow as a new tab, which ComfyUI adds after the open ones. */
  open(job: QueueJob): Promise<void>;
  /** Carry out one request, telling the person plainly when ComfyUI refuses it. */
  run(failure: string, action: () => Promise<void> | void): void;
}

/** ComfyUI keeps a job's workflow only once the job has left the queue. */
export function jobOpensAsTab(job: QueueJob | null): job is QueueJob {
  return job !== null && job.status !== 'pending' && job.status !== 'running';
}

/** The title the job's workflow tab will have. */
export function jobTabTitle(job: QueueJob): string {
  return job.workflowName ?? `Job ${job.id.slice(0, 8)}`;
}

/**
 * The tab bar as a place to drop a finished job. While the job hangs over the bar, a dashed tab
 * with the title it would have shows where it would open; dropping it opens the workflow there.
 */
export function queueTabDropTarget(host: QueueTabDropHost): ReorderDropTarget {
  return {
    hit: (x, y, jobId) => {
      if (!jobOpensAsTab(host.job(jobId))) return null;
      const index = host.tabBar.hit(x, y);
      return index === null ? null : { index };
    },
    show: (jobId, hit) => {
      const job = host.job(jobId);
      const index = readIndex(hit);
      host.tabBar.preview(
        index !== null && jobOpensAsTab(job) ? jobTabTitle(job) : null,
        index ?? 0,
      );
    },
    accept: (jobId, hit) => {
      const job = host.job(jobId);
      const index = readIndex(hit);
      if (index === null || !jobOpensAsTab(job)) return;
      host.run('Workflow not opened', async () => {
        await host.open(job);
        host.tabBar.place(index);
      });
    },
  };
}

function readIndex(hit: unknown): number | null {
  return typeof hit === 'object' && hit !== null && 'index' in hit && typeof hit.index === 'number'
    ? hit.index
    : null;
}
