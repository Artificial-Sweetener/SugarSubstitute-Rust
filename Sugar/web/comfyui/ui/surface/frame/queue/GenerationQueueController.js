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
/** Present ComfyUI's queue as the generation queue: a fly-out from its button, or a pinned panel. */
import { HostSurfaceObserver } from '../../../core/HostSurfaceObserver.js';
import { ContextMenu } from '../../../menus/ContextMenu.js';
import { isRecord } from '../../../types/common.js';
import { applyOwnedTheme } from '../../appearance/ThemeService.js';
import { ComfyQueueActions } from './ComfyQueueActions.js';
import { ComfyQueueJobSource, } from './ComfyQueueJobSource.js';
import { DEFAULT_QUEUE_VIEW, hasFailedJobs, pendingJobCount, queueItems, queueSummary, } from './GenerationQueueRows.js';
import { GenerationQueueFlyout } from './GenerationQueueFlyout.js';
import { GenerationQueueView } from './GenerationQueueView.js';
import { generationQueueViewCss } from './GenerationQueueViewStyles.js';
/** ComfyUI's queue button, which the frame presents as the generation queue's button. */
const BUTTON = 'button[data-sc-generation-queue]';
/** ComfyUI's pinned job panel, whose place beside the canvas the pinned queue takes. */
const DOCK = '[data-testid="job-history-sidebar"]';
/**
 * Own the generation queue for one Substitute frame.
 *
 * The queue's jobs and every action are ComfyUI's; the layout is SugarSubstitute's. A left click
 * on the queue button opens the fly-out, a right click offers the pinned panel and clearing, and
 * ComfyUI's own queue panels stay out of sight while the frame is on.
 */
export class GenerationQueueController {
    options;
    #source;
    #actions;
    #menu;
    #flyoutView;
    #panelView;
    #flyout;
    #dock;
    #style;
    #stop;
    #view = DEFAULT_QUEUE_VIEW;
    #dark = true;
    #disposed = false;
    constructor(options) {
        this.options = options;
        const { document } = options;
        this.#style = document.createElement('style');
        this.#style.textContent = generationQueueViewCss;
        document.head.append(this.#style);
        this.#source = new ComfyQueueJobSource(options.api, options.report, (id) => this.#nodeName(id));
        this.#actions = new ComfyQueueActions(options.api, options.app, () => void this.#source.refresh());
        this.#menu = new ContextMenu(document);
        const requests = {
            act: (row) => this.#run(row.action === 'cancel' ? 'Job not cancelled' : 'Job not removed', () => row.action === 'cancel'
                ? this.#actions.cancel(row.jobId)
                : this.#actions.remove(row.jobId)),
            open: (row) => this.#openResult(row),
            menu: (row, x, y) => this.#rowMenu(row, x, y),
            more: (anchor) => this.#moreMenu(anchor),
        };
        this.#flyoutView = new GenerationQueueView(document, 'flyout', requests);
        this.#panelView = new GenerationQueueView(document, 'panel', {
            ...requests,
            hide: () => this.#actions.closePanel(),
            search: (text) => this.#choose({ search: text }),
        });
        this.#flyout = new GenerationQueueFlyout(this.#flyoutView.element);
        document.addEventListener('click', this.#onButton, true);
        document.addEventListener('contextmenu', this.#onButtonMenu, true);
        this.#dock = new HostSurfaceObserver(document.body, DOCK, () => this.#mountPanel());
        this.#stop = this.#source.subscribe(() => this.#render());
        this.#mountPanel();
        this.#render();
    }
    /** Follow the frame's accepted theme. */
    setPalette(dark) {
        this.#dark = dark;
        applyOwnedTheme(this.#flyoutView.element, dark);
        applyOwnedTheme(this.#panelView.element, dark);
    }
    /** Give ComfyUI its queue button and panels back. */
    dispose() {
        this.#disposed = true;
        this.#stop();
        this.#dock.disconnect();
        this.options.document.removeEventListener('click', this.#onButton, true);
        this.options.document.removeEventListener('contextmenu', this.#onButtonMenu, true);
        this.#flyout.hide();
        this.#flyoutView.dispose();
        this.#panelView.dispose();
        this.#style.remove();
    }
    /** A left click on the queue button opens or closes the fly-out instead of ComfyUI's panel. */
    #onButton = (event) => {
        const button = this.#buttonOf(event);
        if (!button)
            return;
        event.preventDefault();
        event.stopImmediatePropagation();
        if (this.#flyout.open)
            this.#flyout.hide();
        else
            this.#openFlyout(button);
    };
    /** A right click offers the pinned panel and clearing, in place of ComfyUI's one-item menu. */
    #onButtonMenu = (event) => {
        const button = this.#buttonOf(event);
        if (!button)
            return;
        event.preventDefault();
        event.stopImmediatePropagation();
        this.#flyout.hide();
        this.#menu.open({
            label: 'Generation queue',
            anchor: { kind: 'below', bounds: button.getBoundingClientRect() },
            trigger: button,
            entries: [this.#panelEntry(), { label: '', separator: true }, this.#clearEntry()],
        });
    };
    #buttonOf(event) {
        if (this.#disposed || !(event.target instanceof Element))
            return null;
        return event.target.closest(BUTTON);
    }
    #openFlyout(button) {
        this.#flyout.show(button, () => undefined);
        applyOwnedTheme(this.#flyoutView.element, this.#dark);
        this.#render();
    }
    /** Put the pinned queue where ComfyUI shows its own pinned panel. */
    #mountPanel() {
        if (this.#disposed)
            return;
        const dock = this.options.document.querySelector(DOCK);
        const host = dock?.closest('.side-bar-panel') ?? null;
        if (!host) {
            this.#panelView.element.remove();
            return;
        }
        this.#flyout.hide();
        if (this.#panelView.element.parentElement !== host)
            host.append(this.#panelView.element);
        applyOwnedTheme(this.#panelView.element, this.#dark);
        this.#render();
    }
    #render() {
        if (this.#disposed)
            return;
        const snapshot = this.#source.snapshot;
        const view = { ...this.#view, currentWorkflowId: this.#actions.currentWorkflowId };
        const pending = pendingJobCount(snapshot.jobs);
        this.options.showPending(pending);
        const details = new Map(snapshot.jobs.map((job) => [job.id, describe(job)]));
        const narrowed = view.tab !== 'all' || view.workflow !== 'all' || view.search.trim().length > 0;
        const summary = queueSummary(snapshot.jobs);
        if (this.#flyout.open)
            this.#flyoutView.update({
                // The fly-out has no search box, so text typed in the pinned queue does not narrow it.
                items: queueItems(snapshot.jobs, snapshot.progress, { ...view, search: '' }),
                pending,
                summary,
                narrowed: view.tab !== 'all' || view.workflow !== 'all',
                details,
            });
        if (this.#panelView.element.isConnected)
            this.#panelView.update({
                items: queueItems(snapshot.jobs, snapshot.progress, view),
                pending,
                summary,
                narrowed,
                details,
            });
    }
    /** Read a node's title from the graph on the page, which is the one that was queued. */
    #nodeName(id) {
        const graph = isRecord(this.options.app) ? this.options.app.graph : null;
        const find = isRecord(graph) ? graph.getNodeById : null;
        if (typeof find !== 'function')
            return null;
        // A node inside a subgraph is addressed by a path; its own id is the last part.
        const own = id.split(':').pop() ?? id;
        const node = Reflect.apply(find, graph, [
            Number.isNaN(Number(own)) ? own : Number(own),
        ]);
        const title = isRecord(node) ? (node.title ?? node.type) : null;
        return typeof title === 'string' && title.trim() ? title.trim() : null;
    }
    #choose(change) {
        this.#view = { ...this.#view, ...change };
        this.#render();
    }
    /** Everything ComfyUI's queue offers beyond its rows, in one menu. */
    #moreMenu(anchor) {
        const jobs = this.#source.snapshot.jobs;
        const tab = (label, value) => ({
            label,
            selection: 'single',
            checked: this.#view.tab === value,
            select: () => this.#choose({ tab: value }),
        });
        this.#menu.open({
            label: 'Generation queue options',
            anchor: { kind: 'below', bounds: anchor.getBoundingClientRect() },
            trigger: anchor,
            entries: [
                { label: 'Show', section: true },
                tab('All', 'all'),
                tab('Completed', 'completed'),
                // ComfyUI offers this view only while a job has failed or been cancelled.
                ...(hasFailedJobs(jobs) || this.#view.tab === 'failed' ? [tab('Failed', 'failed')] : []),
                { label: '', separator: true },
                {
                    label: 'All workflows',
                    selection: 'single',
                    checked: this.#view.workflow === 'all',
                    select: () => this.#choose({ workflow: 'all' }),
                },
                {
                    label: 'Current workflow',
                    selection: 'single',
                    checked: this.#view.workflow === 'current',
                    select: () => this.#choose({ workflow: 'current' }),
                },
                { label: '', separator: true },
                {
                    label: 'Most recent',
                    selection: 'single',
                    checked: this.#view.sort === 'recent',
                    select: () => this.#choose({ sort: 'recent' }),
                },
                {
                    label: 'Total generation time (longest first)',
                    selection: 'single',
                    checked: this.#view.sort === 'duration',
                    select: () => this.#choose({ sort: 'duration' }),
                },
                { label: '', separator: true },
                this.#panelEntry(),
                {
                    label: 'Show run progress bar',
                    selection: 'multiple',
                    checked: this.#actions.progressBar,
                    select: () => this.#run('Setting not changed', () => this.#actions.setProgressBar(!this.#actions.progressBar)),
                },
                { label: 'Show assets', select: () => this.#actions.showAssets() },
                { label: '', separator: true },
                this.#clearEntry(),
                {
                    label: 'Cancel running jobs',
                    disabled: !jobs.some((job) => job.status === 'running'),
                    select: () => this.#run('Jobs not cancelled', () => this.#actions.cancelAll(jobs.filter((job) => job.status === 'running').map((job) => job.id))),
                },
                {
                    label: 'Clear job history',
                    disabled: !jobs.some((job) => job.status !== 'pending' && job.status !== 'running'),
                    select: () => this.#run('Job history not cleared', async () => {
                        if (await this.options.confirm('Clear your job queue history?', 'All the finished or failed jobs will be removed from the generation queue. Assets generated by these jobs are not deleted.', 'Clear'))
                            await this.#actions.clearFinished();
                    }),
                },
            ],
        });
    }
    #panelEntry() {
        const shown = this.#actions.panelOpen;
        return {
            label: shown ? 'Hide Full Queue Panel' : 'Show Full Queue Panel',
            select: () => this.#run('Queue panel not changed', () => this.#actions.setDocked(!shown)),
        };
    }
    #clearEntry() {
        return {
            label: 'Clear queue',
            disabled: pendingJobCount(this.#source.snapshot.jobs) === 0,
            select: () => this.#run('Queue not cleared', () => this.#actions.clearWaiting()),
        };
    }
    /** Everything that can be done with one job: the queue's own two actions, then ComfyUI's. */
    #rowMenu(row, x, y) {
        const job = this.#source.snapshot.jobs.find((candidate) => candidate.id === row.jobId);
        if (!job)
            return;
        const finished = job.status === 'completed' || job.status === 'cancelled';
        const entries = [
            {
                label: 'Use Seeds in Current Workflow',
                select: () => this.#run('Seeds not taken', () => this.options.useSeeds(job.id)),
            },
            {
                label: 'Open as Workflow Tab',
                // ComfyUI keeps a job's workflow once the job has left the queue.
                disabled: !finished && job.status !== 'failed',
                select: () => this.#run('Workflow not opened', () => this.#openWorkflow(job)),
            },
            { label: '', separator: true },
        ];
        if (job.status === 'completed') {
            const kind = job.outputKind;
            entries.push({ label: 'Inspect asset', disabled: !job.previewUrl, select: () => this.#openResult(row) }, {
                label: 'Add to current workflow',
                disabled: !job.outputFile || kind === null || kind === 'other',
                select: () => {
                    if (!job.outputFile || kind === null || kind === 'other')
                        return;
                    if (!this.#actions.addToWorkflow(kind, job.outputFile, job.outputSubfolder))
                        this.options.notify('error', 'Not added', 'ComfyUI has no loader node for this file.');
                },
            }, { label: 'Download', disabled: !job.previewUrl, select: () => this.#download(job) }, {
                label: 'Export workflow',
                select: () => this.#run('Workflow not exported', () => this.#exportWorkflow(job)),
            }, { label: '', separator: true });
        }
        entries.push({ label: 'Copy job ID', select: () => this.#copy(job.id) });
        if (job.error !== null)
            entries.push({ label: 'Copy error message', select: () => this.#copy(job.error ?? '') }, { label: 'Report error', select: () => this.#actions.reportError(job.errorDetail) });
        entries.push({ label: '', separator: true });
        entries.push(row.action === 'cancel'
            ? {
                label: 'Cancel job',
                select: () => this.#run('Job not cancelled', () => this.#actions.cancel(job.id)),
            }
            : job.status === 'completed'
                ? {
                    label: 'Delete asset',
                    select: () => this.#run('Asset not deleted', async () => {
                        if (await this.options.confirm('Delete this asset?', 'This asset will be permanently removed.', 'Delete'))
                            await this.#actions.remove(job.id);
                    }),
                }
                : {
                    label: 'Remove job',
                    select: () => this.#run('Job not removed', () => this.#actions.remove(job.id)),
                });
        this.#menu.open({
            label: 'Generation queue job',
            anchor: { kind: 'pointer', x, y },
            entries,
        });
    }
    async #openWorkflow(job) {
        const workflow = await this.#actions.workflow(job.id);
        const load = isRecord(this.options.app) ? this.options.app.loadGraphData : null;
        if (!workflow || typeof load !== 'function')
            throw new Error('ComfyUI kept no workflow for this job.');
        // A title makes ComfyUI open the graph as a new unsaved workflow beside the open ones.
        await Reflect.apply(load, this.options.app, [
            workflow,
            true,
            true,
            job.workflowName ?? `Job ${job.id.slice(0, 8)}`,
        ]);
    }
    async #exportWorkflow(job) {
        const workflow = await this.#actions.workflow(job.id);
        if (!workflow)
            throw new Error('ComfyUI kept no workflow for this job.');
        const { document } = this.options;
        const link = document.createElement('a');
        const url = URL.createObjectURL(new Blob([JSON.stringify(workflow, null, 2)], { type: 'application/json' }));
        link.href = url;
        link.download = `${job.workflowName ?? `Job ${job.id}`}.json`;
        link.click();
        URL.revokeObjectURL(url);
    }
    #download(job) {
        if (!job.previewUrl)
            return;
        const link = this.options.document.createElement('a');
        link.href = job.previewUrl;
        link.download = job.outputFile ?? '';
        link.click();
    }
    /** Show a finished job's result where ComfyUI keeps generated assets. */
    #openResult(row) {
        if (row.status === 'completed' && row.previewUrl)
            this.#actions.showAssets();
    }
    #copy(text) {
        const clipboard = this.options.document.defaultView?.navigator.clipboard;
        if (!clipboard)
            return;
        void clipboard.writeText(text).catch(this.options.report);
    }
    /** Carry out one request, telling the person plainly when ComfyUI refuses it. */
    #run(failure, action) {
        void Promise.resolve()
            .then(action)
            .catch((error) => {
            this.options.report(error);
            this.options.notify('error', failure, error instanceof Error ? error.message : String(error));
        });
    }
}
/** One job's details for its tooltip: what ComfyUI lists under "Job Details". */
function describe(job) {
    const time = (value) => value === null ? null : new Date(value).toLocaleString();
    const lines = [
        job.workflowName ? `Workflow: ${job.workflowName}` : null,
        `Job ID: ${job.id}`,
        time(job.createTime) ? `Queued at: ${time(job.createTime) ?? ''}` : null,
        job.status === 'completed' && time(job.endTime)
            ? `Generated on: ${time(job.endTime) ?? ''}`
            : null,
        job.outputFile ? `Last output: ${job.outputFile}` : null,
        job.error ? `Error message: ${job.error}` : null,
    ];
    return lines.filter((line) => line !== null).join('\n');
}
