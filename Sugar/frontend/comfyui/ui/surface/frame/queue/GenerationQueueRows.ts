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
/** Turn ComfyUI's jobs into the generation queue's rows: their order, titles and status lines. */

/** Where one job stands. ComfyUI's "in progress" is running here. */
export type QueueJobStatus = 'pending' | 'running' | 'completed' | 'failed' | 'cancelled';

/** One ComfyUI job with everything the queue shows about it. Times are milliseconds. */
export interface QueueJob {
  readonly id: string;
  readonly status: QueueJobStatus;
  /** ComfyUI's execution order: a lower number runs sooner. */
  readonly priority: number;
  readonly createTime: number | null;
  readonly startTime: number | null;
  readonly endTime: number | null;
  /** The workflow tab the job was queued from, when the queueing page recorded it. */
  readonly workflowName: string | null;
  /** The run number its outputs are saved under, when known. */
  readonly runNumber: number | null;
  /** ComfyUI's identity of the workflow, which its "Current workflow" filter compares. */
  readonly workflowId: string | null;
  readonly outputFile: string | null;
  /** Where ComfyUI keeps that file, and what kind of media it is. */
  readonly outputSubfolder: string | null;
  readonly outputKind: 'image' | 'video' | 'audio' | 'other' | null;
  readonly previewUrl: string | null;
  readonly outputsCount: number;
  readonly error: string | null;
  /** ComfyUI's whole record of the failure, which its own error dialog presents. */
  readonly errorDetail: unknown;
}

/** Which jobs the person asked to see: ComfyUI's tabs, workflow filter, sort and search. */
export interface QueueViewOptions {
  readonly tab: 'all' | 'completed' | 'failed';
  readonly workflow: 'all' | 'current';
  readonly currentWorkflowId: string | null;
  readonly sort: 'recent' | 'duration';
  readonly search: string;
}

/** What a row's one button does: stop a job that has not finished, or forget one that has. */
export type QueueRowAction = 'cancel' | 'remove';

/** One row as the queue draws it. */
export interface QueueRow {
  readonly jobId: string;
  /** Waiting rows respond to the pointer, the active row stays lit, resolved rows are still. */
  readonly role: 'waiting' | 'active' | 'resolved';
  readonly status: QueueJobStatus;
  readonly title: string;
  readonly subtitle: string;
  /** What ComfyUI adds beside the status: the node at work, when a job was queued, its file. */
  readonly detail: string;
  readonly previewUrl: string | null;
  /** A moving picture is marked as one; its thumbnail cannot show that by itself. */
  readonly video: boolean;
  readonly action: QueueRowAction;
}

/** A row, or the caption that separates waiting jobs from finished ones. */
export type QueueItem =
  | { readonly kind: 'row'; readonly row: QueueRow }
  | { readonly kind: 'caption'; readonly text: string };

/** What ComfyUI last said about the job that is running. */
export interface QueueProgress {
  /** Whole-number progress of the whole job, or null while ComfyUI has not reported any. */
  readonly percent: number | null;
  /** The node at work and how far along it is. */
  readonly node: string | null;
  readonly nodePercent: number | null;
  /** The picture ComfyUI is showing for the running job as it forms. */
  readonly previewUrl: string | null;
}

/** How the queue writes moments, so its wording follows the person's locale and can be tested. */
export interface QueueClock {
  readonly now: number;
  time(moment: number): string;
  date(moment: number): string;
}

/** Nothing reported yet. */
export const NO_QUEUE_PROGRESS: QueueProgress = {
  percent: null,
  node: null,
  nodePercent: null,
  previewUrl: null,
};

/** The view a queue opens with: everything, newest first. */
export const DEFAULT_QUEUE_VIEW: QueueViewOptions = {
  tab: 'all',
  workflow: 'all',
  currentWorkflowId: null,
  sort: 'recent',
  search: '',
};

const FAILURE_SUMMARY_LIMIT = 64;

/** Count the jobs still waiting to run, which is what the queue button's badge shows. */
export function pendingJobCount(jobs: readonly QueueJob[]): number {
  return jobs.filter((job) => job.status === 'pending').length;
}

/** Whether any job failed or was cancelled, which is when ComfyUI offers its Failed tab. */
export function hasFailedJobs(jobs: readonly QueueJob[]): boolean {
  return jobs.some((job) => job.status === 'failed' || job.status === 'cancelled');
}

/**
 * Lay the jobs out as the generation queue always has: waiting jobs with the newest on top and the
 * next to run at the bottom, then the running job, then finished jobs with the newest first, under
 * the day they finished.
 */
export function queueItems(
  jobs: readonly QueueJob[],
  progress: QueueProgress = NO_QUEUE_PROGRESS,
  view: QueueViewOptions = DEFAULT_QUEUE_VIEW,
  clock: QueueClock = localClock(),
): readonly QueueItem[] {
  const shown = jobs.filter((job) => matches(job, view));
  const waiting = shown
    .filter((job) => job.status === 'pending')
    .sort((a, b) => b.priority - a.priority);
  const active = shown.filter((job) => job.status === 'running');
  const resolved = shown
    .filter((job) => job.status !== 'pending' && job.status !== 'running')
    .sort((a, b) =>
      view.sort === 'duration'
        ? (duration(b) ?? -1) - (duration(a) ?? -1) || recency(b) - recency(a)
        : recency(b) - recency(a) || b.id.localeCompare(a.id),
    );
  // A waiting job's place is counted among all waiting jobs, not only the ones a filter left.
  const order = jobs
    .filter((job) => job.status === 'pending')
    .sort((a, b) => a.priority - b.priority)
    .map((job) => job.id);
  const items: QueueItem[] = [];
  for (const job of waiting)
    items.push(
      row(
        job,
        'waiting',
        waitingLine(order.indexOf(job.id)),
        job.createTime === null ? '' : clock.time(job.createTime),
        job.previewUrl,
      ),
    );
  for (const job of active)
    items.push(
      row(
        job,
        'active',
        progress.percent === null ? 'Running' : `${String(progress.percent)}% complete`,
        progress.node === null
          ? ''
          : progress.nodePercent === null
            ? progress.node
            : `${progress.node}: ${String(progress.nodePercent)}%`,
        progress.previewUrl ?? job.previewUrl,
      ),
    );
  // Finished jobs sit under the day they ended. One day with nothing above it needs no heading.
  const days = resolved.map((job) => dayLabel(recencyOrNull(job), clock));
  const headed = waiting.length + active.length > 0 || new Set(days).size > 1;
  resolved.forEach((job, index) => {
    if (headed && view.sort === 'recent' && days[index] !== days[index - 1])
      items.push({
        kind: 'caption',
        text:
          index === 0 && active.length === 0 && waiting.length > 0
            ? `Resolved - ${days[index] ?? ''}`
            : (days[index] ?? ''),
      });
    // A job named after its file already says the file in its title.
    const file = job.outputFile && title(job) !== job.outputFile.trim() ? job.outputFile : '';
    items.push(row(job, 'resolved', resolvedLine(job), file, job.previewUrl));
  });
  return items;
}

/** Summarize what is running and waiting, as ComfyUI does above its own list. */
export function queueSummary(jobs: readonly QueueJob[]): string {
  const running = jobs.filter((job) => job.status === 'running').length;
  const queued = pendingJobCount(jobs);
  return [
    running > 0 ? `${String(running)} running` : '',
    queued > 0 ? `${String(queued)} queued` : '',
  ]
    .filter(Boolean)
    .join(', ');
}

/** Write a duration the way the queue does: 12.3s, 4m5s, 1h2m. */
export function formatDuration(milliseconds: number): string {
  const seconds = Math.max(0, milliseconds) / 1000;
  if (seconds < 60) return `${seconds.toFixed(1)}s`;
  const minutes = Math.floor(seconds / 60);
  if (minutes < 60) return `${String(minutes)}m${String(Math.floor(seconds % 60))}s`;
  return `${String(Math.floor(minutes / 60))}h${String(minutes % 60)}m`;
}

function row(
  job: QueueJob,
  role: QueueRow['role'],
  subtitle: string,
  detail: string,
  previewUrl: string | null,
): QueueItem {
  return {
    kind: 'row',
    row: {
      jobId: job.id,
      role,
      status: job.status,
      title: title(job),
      subtitle,
      detail,
      previewUrl,
      video: job.outputKind === 'video',
      action: role === 'resolved' ? 'remove' : 'cancel',
    },
  };
}

/** Name a job by its workflow and run number, falling back to what ComfyUI itself would show. */
function title(job: QueueJob): string {
  const name = job.workflowName?.trim();
  if (name)
    return job.runNumber === null ? name : `${name} #${String(job.runNumber).padStart(3, '0')}`;
  return job.outputFile?.trim() || `Job ${job.id.slice(0, 8)}`;
}

function waitingLine(ahead: number): string {
  return ahead <= 0 ? 'Next' : `Waiting - ${String(ahead)} ahead`;
}

function resolvedLine(job: QueueJob): string {
  if (job.status === 'completed') {
    const took = duration(job);
    return took === null ? 'Completed' : `Completed, ${formatDuration(took)}`;
  }
  if (job.status === 'cancelled') {
    if (job.outputsCount <= 0) return 'Cancelled - No outputs saved';
    const outputs = job.outputsCount === 1 ? '1 output' : `${String(job.outputsCount)} outputs`;
    return `Cancelled - ${outputs} saved`;
  }
  const summary = job.error?.replace(/\s+/g, ' ').trim();
  if (!summary) return 'Generation failed';
  return `Failed - ${
    summary.length > FAILURE_SUMMARY_LIMIT
      ? `${summary.slice(0, FAILURE_SUMMARY_LIMIT)}...`
      : summary
  }`;
}

function duration(job: QueueJob): number | null {
  return job.startTime !== null && job.endTime !== null && job.endTime >= job.startTime
    ? job.endTime - job.startTime
    : null;
}

function recencyOrNull(job: QueueJob): number | null {
  return job.endTime ?? job.createTime;
}

/** Name a moment's day the way ComfyUI groups its list: today, yesterday, or the date. */
function dayLabel(moment: number | null, clock: QueueClock): string {
  if (moment === null) return 'Undated';
  const start = (value: number): number => {
    const day = new Date(value);
    day.setHours(0, 0, 0, 0);
    return day.getTime();
  };
  const days = Math.round((start(clock.now) - start(moment)) / 86_400_000);
  return days === 0 ? 'Today' : days === 1 ? 'Yesterday' : clock.date(moment);
}

/** The clock and wording of the machine the page runs on. */
export function localClock(): QueueClock {
  return {
    now: Date.now(),
    time: (moment) =>
      new Date(moment).toLocaleTimeString([], { hour: 'numeric', minute: '2-digit' }),
    date: (moment) => new Date(moment).toLocaleDateString(),
  };
}

function recency(job: QueueJob): number {
  return job.endTime ?? job.createTime ?? 0;
}

function matches(job: QueueJob, view: QueueViewOptions): boolean {
  if (view.tab === 'completed' && job.status !== 'completed') return false;
  if (view.tab === 'failed' && job.status !== 'failed' && job.status !== 'cancelled') return false;
  if (view.workflow === 'current' && job.workflowId !== view.currentWorkflowId) return false;
  const search = view.search.trim().toLocaleLowerCase();
  if (!search) return true;
  return [title(job), job.outputFile ?? '', job.id].some((text) =>
    text.toLocaleLowerCase().includes(search),
  );
}
