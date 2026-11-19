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
import { DEFAULT_QUEUE_VIEW, pendingJobCount, queueItems, queueSummary, } from './GenerationQueueRows.js';
import { GenerationQueueFlyout } from './GenerationQueueFlyout.js';
import { GenerationQueueMenus } from './GenerationQueueMenus.js';
import { GenerationQueueReorder } from './GenerationQueueReorder.js';
import { jobOpensAsTab, queueTabDropTarget } from './GenerationQueueTabDrop.js';
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
    #menus;
    #reorders;
    #reordering = false;
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
        this.#menus = new GenerationQueueMenus({
            document,
            app: options.app,
            menu: this.#menu,
            actions: this.#actions,
            jobs: () => this.#source.snapshot.jobs,
            view: () => this.#view,
            choose: (change) => this.#choose(change),
            useSeeds: options.useSeeds,
            confirm: options.confirm,
            notify: options.notify,
            report: options.report,
            run: (failure, action) => this.#run(failure, action),
        });
        const requests = {
            act: (row) => this.#run(row.action === 'cancel' ? 'Job not cancelled' : 'Job not removed', () => row.action === 'cancel'
                ? this.#actions.cancel(row.jobId)
                : this.#actions.remove(row.jobId)),
            open: (row) => this.#menus.openResult(row),
            menu: (row, x, y) => this.#menus.row(row, x, y),
            more: (anchor) => this.#menus.more(anchor),
        };
        this.#flyoutView = new GenerationQueueView(document, 'flyout', requests);
        this.#panelView = new GenerationQueueView(document, 'panel', {
            ...requests,
            hide: () => this.#actions.closePanel(),
            search: (text) => this.#choose({ search: text }),
        });
        this.#flyout = new GenerationQueueFlyout(this.#flyoutView.element);
        const job = (jobId) => this.#source.snapshot.jobs.find((candidate) => candidate.id === jobId) ?? null;
        const tabDrop = options.tabBar
            ? queueTabDropTarget({
                tabBar: options.tabBar,
                job,
                open: (finished) => this.#menus.openWorkflow(finished),
                run: (failure, action) => this.#run(failure, action),
            })
            : null;
        this.#reorders = [this.#flyoutView, this.#panelView].map((view) => new GenerationQueueReorder({
            root: view.element,
            rows: view.rows,
            waiting: () => view.waiting(),
            busy: () => this.#reordering,
            preview: (order) => view.previewWaiting(order),
            commit: (order) => this.#reorder(view, order),
            ...(tabDrop
                ? { targets: [tabDrop], carries: (jobId) => jobOpensAsTab(job(jobId)) }
                : {}),
        }));
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
        for (const reorder of this.#reorders)
            reorder.dispose();
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
            entries: [this.#menus.panelEntry(), { label: '', separator: true }, this.#menus.clearEntry()],
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
        // A job that started or left while it was being dragged is no longer the person's to move.
        for (const reorder of this.#reorders)
            reorder.reconcile();
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
    /**
     * Run the waiting jobs in the order they were dropped into. The list shows the newest job on
     * top and the next to run at the bottom, so ComfyUI is given the order from the bottom up. The
     * dropped order stays on screen until ComfyUI's own list has been read again.
     */
    #reorder(view, order) {
        this.#reordering = true;
        view.previewWaiting(order);
        void this.#actions
            .reorder([...order].reverse())
            .then(() => this.#source.refresh())
            .catch((error) => {
            this.options.report(error);
            this.options.notify('error', 'Queue not reordered', error instanceof Error ? error.message : String(error));
        })
            .finally(() => {
            this.#reordering = false;
            view.previewWaiting(null);
            for (const reorder of this.#reorders)
                reorder.resume();
        });
    }
    #choose(change) {
        this.#view = { ...this.#view, ...change };
        this.#render();
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
