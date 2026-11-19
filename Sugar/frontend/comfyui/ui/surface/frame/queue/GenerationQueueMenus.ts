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
/** Offer everything the generation queue and its jobs can do, as menus. */
import type { ContextMenu, ContextMenuEntry } from '../../../menus/ContextMenu.js';
import { isRecord } from '../../../types/common.js';
import type { ComfyQueueActions } from './ComfyQueueActions.js';
import { jobTabTitle } from './GenerationQueueTabDrop.js';
import {
  hasFailedJobs,
  pendingJobCount,
  type QueueJob,
  type QueueRow,
  type QueueViewOptions,
} from './GenerationQueueRows.js';

/** What the menus read and ask of the queue's owner. */
export interface GenerationQueueMenuHost {
  readonly document: Document;
  readonly app: unknown;
  readonly menu: ContextMenu;
  readonly actions: ComfyQueueActions;
  jobs(): readonly QueueJob[];
  view(): QueueViewOptions;
  choose(change: Partial<QueueViewOptions>): void;
  useSeeds(jobId: string): Promise<void>;
  /** Show a finished job's result; false when it has none to show. */
  inspect(job: QueueJob): boolean;
  /** Show everything known about a job. */
  details(job: QueueJob): void;
  confirm(title: string, message: string, agree: string): Promise<boolean>;
  notify(severity: 'success' | 'warn' | 'error', title: string, message: string): void;
  report(error: unknown): void;
  /** Carry out one request, telling the person plainly when ComfyUI refuses it. */
  run(failure: string, action: () => Promise<void> | void): void;
}

/**
 * The queue's menus: its own choices behind the More button, and each job's actions on its row.
 * Every entry ComfyUI's queue offers is here, beside the two the generation queue has always had.
 */
export class GenerationQueueMenus {
  constructor(private readonly host: GenerationQueueMenuHost) {}

  /** Everything ComfyUI's queue offers beyond its rows, in one menu. */
  more(anchor: HTMLElement): void {
    const jobs = this.host.jobs();
    const tab = (label: string, value: QueueViewOptions['tab']): ContextMenuEntry => ({
      label,
      selection: 'single',
      checked: this.host.view().tab === value,
      select: () => this.host.choose({ tab: value }),
    });
    this.host.menu.open({
      label: 'Generation queue options',
      anchor: { kind: 'below', bounds: anchor.getBoundingClientRect() },
      trigger: anchor,
      entries: [
        { label: 'Show', section: true },
        tab('All', 'all'),
        tab('Completed', 'completed'),
        // ComfyUI offers this view only while a job has failed or been cancelled.
        ...(hasFailedJobs(jobs) || this.host.view().tab === 'failed'
          ? [tab('Failed', 'failed')]
          : []),
        { label: '', separator: true },
        {
          label: 'All workflows',
          selection: 'single',
          checked: this.host.view().workflow === 'all',
          select: () => this.host.choose({ workflow: 'all' }),
        },
        {
          label: 'Current workflow',
          selection: 'single',
          checked: this.host.view().workflow === 'current',
          select: () => this.host.choose({ workflow: 'current' }),
        },
        { label: '', separator: true },
        {
          label: 'Most recent',
          selection: 'single',
          checked: this.host.view().sort === 'recent',
          select: () => this.host.choose({ sort: 'recent' }),
        },
        {
          label: 'Total generation time (longest first)',
          selection: 'single',
          checked: this.host.view().sort === 'duration',
          select: () => this.host.choose({ sort: 'duration' }),
        },
        { label: '', separator: true },
        this.panelEntry(),
        {
          label: 'Show run progress bar',
          selection: 'multiple',
          checked: this.host.actions.progressBar,
          select: () =>
            this.host.run('Setting not changed', () =>
              this.host.actions.setProgressBar(!this.host.actions.progressBar),
            ),
        },
        { label: 'Show assets', select: () => this.host.actions.showAssets() },
        { label: '', separator: true },
        this.clearEntry(),
        {
          label: 'Cancel running jobs',
          disabled: !jobs.some((job) => job.status === 'running'),
          select: () =>
            this.host.run('Jobs not cancelled', () =>
              this.host.actions.cancelAll(
                jobs.filter((job) => job.status === 'running').map((job) => job.id),
              ),
            ),
        },
        {
          label: 'Clear job history',
          disabled: !jobs.some((job) => job.status !== 'pending' && job.status !== 'running'),
          select: () =>
            this.host.run('Job history not cleared', async () => {
              if (
                await this.host.confirm(
                  'Clear your job queue history?',
                  'All the finished or failed jobs will be removed from the generation queue. Assets generated by these jobs are not deleted.',
                  'Clear',
                )
              )
                await this.host.actions.clearFinished();
            }),
        },
      ],
    });
  }

  panelEntry(): ContextMenuEntry {
    const shown = this.host.actions.panelOpen;
    return {
      label: shown ? 'Hide Full Queue Panel' : 'Show Full Queue Panel',
      select: () =>
        this.host.run('Queue panel not changed', () => this.host.actions.setDocked(!shown)),
    };
  }

  clearEntry(): ContextMenuEntry {
    return {
      label: 'Clear queue',
      disabled: pendingJobCount(this.host.jobs()) === 0,
      select: () => this.host.run('Queue not cleared', () => this.host.actions.clearWaiting()),
    };
  }

  /** Everything that can be done with one job: the queue's own two actions, then ComfyUI's. */
  row(row: QueueRow, x: number, y: number): void {
    const job = this.host.jobs().find((candidate) => candidate.id === row.jobId);
    if (!job) return;
    const finished = job.status === 'completed' || job.status === 'cancelled';
    const entries: ContextMenuEntry[] = [
      {
        label: 'Use Seeds in Current Workflow',
        select: () => this.host.run('Seeds not taken', () => this.host.useSeeds(job.id)),
      },
      {
        label: 'Open as Workflow Tab',
        // ComfyUI keeps a job's workflow once the job has left the queue.
        disabled: !finished && job.status !== 'failed',
        select: () => this.host.run('Workflow not opened', () => this.openWorkflow(job)),
      },
      { label: '', separator: true },
    ];
    if (job.status === 'completed') {
      const kind = job.outputKind;
      entries.push(
        { label: 'Inspect asset', disabled: !job.previewUrl, select: () => this.openResult(row) },
        {
          label: 'Add to current workflow',
          disabled: !job.outputFile || kind === null || kind === 'other',
          select: () => {
            if (!job.outputFile || kind === null || kind === 'other') return;
            if (!this.host.actions.addToWorkflow(kind, job.outputFile, job.outputSubfolder))
              this.host.notify('error', 'Not added', 'ComfyUI has no loader node for this file.');
          },
        },
        { label: 'Download', disabled: !job.previewUrl, select: () => this.#download(job) },
        {
          label: 'Export workflow',
          select: () => this.host.run('Workflow not exported', () => this.#exportWorkflow(job)),
        },
        { label: '', separator: true },
      );
    }
    entries.push(
      { label: 'Job details', select: () => this.host.details(job) },
      { label: 'Copy job ID', select: () => this.#copy(job.id) },
    );
    if (job.error !== null)
      entries.push(
        { label: 'Copy error message', select: () => this.#copy(job.error ?? '') },
        { label: 'Report error', select: () => this.host.actions.reportError(job.errorDetail) },
      );
    entries.push({ label: '', separator: true });
    entries.push(
      row.action === 'cancel'
        ? {
            label: 'Cancel job',
            select: () =>
              this.host.run('Job not cancelled', () => this.host.actions.cancel(job.id)),
          }
        : job.status === 'completed'
          ? {
              label: 'Delete asset',
              select: () =>
                this.host.run('Asset not deleted', async () => {
                  if (
                    await this.host.confirm(
                      'Delete this asset?',
                      'This asset will be permanently removed.',
                      'Delete',
                    )
                  )
                    await this.host.actions.remove(job.id);
                }),
            }
          : {
              label: 'Remove job',
              select: () =>
                this.host.run('Job not removed', () => this.host.actions.remove(job.id)),
            },
    );
    this.host.menu.open({
      label: 'Generation queue job',
      anchor: { kind: 'pointer', x, y },
      entries,
    });
  }

  /** Open a finished job's workflow as a new tab beside the open ones. */
  async openWorkflow(job: QueueJob): Promise<void> {
    const workflow = await this.host.actions.workflow(job.id);
    const load = isRecord(this.host.app) ? this.host.app.loadGraphData : null;
    if (!workflow || typeof load !== 'function')
      throw new Error('ComfyUI kept no workflow for this job.');
    // A title makes ComfyUI open the graph as a new unsaved workflow beside the open ones.
    await (Reflect.apply(load, this.host.app, [workflow, true, true, jobTabTitle(job)]) as unknown);
  }

  async #exportWorkflow(job: QueueJob): Promise<void> {
    const workflow = await this.host.actions.workflow(job.id);
    if (!workflow) throw new Error('ComfyUI kept no workflow for this job.');
    const { document } = this.host;
    const link = document.createElement('a');
    const url = URL.createObjectURL(
      new Blob([JSON.stringify(workflow, null, 2)], { type: 'application/json' }),
    );
    link.href = url;
    link.download = `${job.workflowName ?? `Job ${job.id}`}.json`;
    link.click();
    URL.revokeObjectURL(url);
  }

  #download(job: QueueJob): void {
    if (!job.previewUrl) return;
    const link = this.host.document.createElement('a');
    link.href = job.previewUrl;
    link.download = job.outputFile ?? '';
    link.click();
  }

  /** Show a finished job's result. */
  openResult(row: QueueRow): void {
    const job = this.host.jobs().find((candidate) => candidate.id === row.jobId);
    if (job) this.host.inspect(job);
  }

  #copy(text: string): void {
    const clipboard = this.host.document.defaultView?.navigator.clipboard;
    if (!clipboard) return;
    void clipboard.writeText(text).catch(this.host.report);
  }
}
