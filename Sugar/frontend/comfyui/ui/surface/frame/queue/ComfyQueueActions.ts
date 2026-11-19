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
/** Perform the generation queue's actions through ComfyUI's own routes, commands and settings. */
import { isRecord } from '../../../types/common.js';
import type { ComfyQueueApi } from './ComfyQueueJobSource.js';

/** ComfyUI's setting that chooses the docked job panel over the floating one. */
export const QUEUE_DOCKED_SETTING = 'Comfy.Queue.QPOV2';
/** ComfyUI's setting for the run progress bar under its actions. */
export const QUEUE_PROGRESS_BAR_SETTING = 'Comfy.Queue.ShowRunProgressBar';
const SIDEBAR_TAB = 'job-history';

/**
 * Own every change the queue makes to ComfyUI.
 *
 * Each action is ComfyUI's own: the route its client calls, the command its button runs or the
 * setting its menu flips, so the queue and ComfyUI can never disagree about what an action does.
 * `app` is narrowed on each use because ComfyUI replaces its stores while it starts.
 */
export class ComfyQueueActions {
  constructor(
    private readonly api: ComfyQueueApi,
    private readonly app: unknown,
    private readonly changed: () => void,
  ) {}

  /** Stop one job that is waiting or running. */
  async cancel(jobId: string): Promise<void> {
    await this.#post(`/jobs/${encodeURIComponent(jobId)}/cancel`);
    this.changed();
  }

  /** Stop every job that is running. */
  async cancelAll(jobIds: readonly string[]): Promise<void> {
    if (jobIds.length === 0) return;
    await this.#post('/jobs/cancel', { job_ids: jobIds });
    this.changed();
  }

  /**
   * Run the waiting jobs in `order`, soonest first. ComfyUI has no way to move a waiting job, so
   * SugarCubes moves them inside ComfyUI's own queue, where each keeps its identity.
   */
  async reorder(order: readonly string[]): Promise<void> {
    await this.#post('/sugarcubes/v2/queue/order', { order });
    this.changed();
  }

  /** Forget one finished job. Its saved outputs are not touched. */
  async remove(jobId: string): Promise<void> {
    await this.#post('/history', { delete: [jobId] });
    this.changed();
  }

  /** Drop every job that is still waiting. */
  async clearWaiting(): Promise<void> {
    await this.#post('/queue', { clear: true });
    this.changed();
  }

  /** Forget every finished job. Saved outputs are not touched. */
  async clearFinished(): Promise<void> {
    await this.#post('/history', { clear: true });
    this.changed();
  }

  /** Read the workflow a finished job ran, or null when ComfyUI kept none. */
  async workflow(jobId: string): Promise<unknown> {
    const response = await this.api.fetchApi(`/jobs/${encodeURIComponent(jobId)}`);
    if (!response.ok) throw new Error(`ComfyUI answered ${String(response.status)} for the job`);
    const detail: unknown = await response.json();
    const workflow = isRecord(detail) && isRecord(detail.workflow) ? detail.workflow : null;
    const extra = workflow && isRecord(workflow.extra_data) ? workflow.extra_data : null;
    const info = extra && isRecord(extra.extra_pnginfo) ? extra.extra_pnginfo : null;
    return info && isRecord(info.workflow) ? info.workflow : null;
  }

  /** Whether the queue is pinned beside the canvas rather than opened from its button. */
  get docked(): boolean {
    return this.#setting(QUEUE_DOCKED_SETTING) === true;
  }

  /** Whether the pinned queue is showing. */
  get panelOpen(): boolean {
    const tabs = this.#store('sidebarTab');
    return this.docked && isRecord(tabs) && tabs.activeSidebarTabId === SIDEBAR_TAB;
  }

  /** Pin the queue beside the canvas, or return it to its button. */
  async setDocked(docked: boolean): Promise<void> {
    await this.#set(QUEUE_DOCKED_SETTING, docked);
    const tabs = this.#store('sidebarTab');
    if (!isRecord(tabs)) return;
    const open = tabs.activeSidebarTabId === SIDEBAR_TAB;
    // ComfyUI registers the panel a moment after the setting changes, so opening waits a turn.
    if (docked && !open) await Promise.resolve().then(() => this.#toggle(tabs, SIDEBAR_TAB));
  }

  /** Put the pinned queue away without unpinning it. */
  closePanel(): void {
    const tabs = this.#store('sidebarTab');
    if (isRecord(tabs) && tabs.activeSidebarTabId === SIDEBAR_TAB) this.#toggle(tabs, SIDEBAR_TAB);
  }

  get progressBar(): boolean {
    return this.#setting(QUEUE_PROGRESS_BAR_SETTING) !== false;
  }

  async setProgressBar(shown: boolean): Promise<void> {
    await this.#set(QUEUE_PROGRESS_BAR_SETTING, shown);
  }

  /** Show ComfyUI's panel of generated assets. */
  showAssets(): void {
    const tabs = this.#store('sidebarTab');
    if (isRecord(tabs) && tabs.activeSidebarTabId !== 'assets') this.#toggle(tabs, 'assets');
  }

  /** ComfyUI's identity of the workflow in the current tab, which its workflow filter compares. */
  get currentWorkflowId(): string | null {
    const workflows = this.#store('workflow');
    const active =
      isRecord(workflows) && isRecord(workflows.activeWorkflow) ? workflows.activeWorkflow : null;
    const state = active && isRecord(active.activeState) ? active.activeState : null;
    return state && typeof state.id === 'string' ? state.id : null;
  }

  /**
   * Put a job's output into the current workflow as a loader node, as ComfyUI's queue does.
   * Returns false when ComfyUI has no loader for that kind of media.
   */
  addToWorkflow(
    kind: 'image' | 'video' | 'audio',
    file: string,
    subfolder: string | null,
  ): boolean {
    const litegraph: unknown = Reflect.get(globalThis, 'LiteGraph');
    const create = isRecord(litegraph) ? litegraph.createNode : null;
    const graph = isRecord(this.app) ? this.app.graph : null;
    if (typeof create !== 'function' || !isRecord(graph) || typeof graph.add !== 'function')
      return false;
    const type = kind === 'image' ? 'LoadImage' : kind === 'video' ? 'LoadVideo' : 'LoadAudio';
    const node: unknown = Reflect.apply(create, litegraph, [type]);
    if (!isRecord(node)) return false;
    const widgets = Array.isArray(node.widgets) ? (node.widgets as unknown[]) : [];
    const widget = widgets.find(
      (candidate) => isRecord(candidate) && typeof candidate.value === 'string',
    );
    // ComfyUI addresses a file in its output folder by name and the folder's tag.
    if (isRecord(widget)) widget.value = `${subfolder ? `${subfolder}/` : ''}${file} [output]`;
    const canvas = isRecord(this.app) && isRecord(this.app.canvas) ? this.app.canvas : null;
    const area: unknown = canvas ? canvas.visible_area : null;
    if (area !== null && typeof area === 'object') {
      const [x, y, width, height] = [0, 1, 2, 3].map((index) => Number(Reflect.get(area, index)));
      if ([x, y, width, height].every((value) => Number.isFinite(value)))
        node.pos = [(x ?? 0) + (width ?? 0) / 2, (y ?? 0) + (height ?? 0) / 2];
    }
    Reflect.apply(graph.add, graph, [node]);
    return true;
  }

  /** Let ComfyUI present one job's error the way it does from its own queue. */
  reportError(error: unknown): void {
    const dialog = this.#store('dialog');
    const show = isRecord(dialog) ? dialog.showExecutionErrorDialog : null;
    if (typeof show === 'function') Reflect.apply(show, dialog, [error]);
  }

  async #post(route: string, body?: unknown): Promise<void> {
    const response = await this.api.fetchApi(route, {
      method: 'POST',
      ...(body === undefined
        ? {}
        : { headers: { 'Content-Type': 'application/json' }, body: JSON.stringify(body) }),
    });
    if (!response.ok) throw new Error(`ComfyUI answered ${String(response.status)} for ${route}`);
  }

  #store(name: string): unknown {
    const manager = isRecord(this.app) ? this.app.extensionManager : null;
    return isRecord(manager) ? manager[name] : null;
  }

  #setting(id: string): unknown {
    const settings = this.#store('setting');
    const get = isRecord(settings) ? settings.get : null;
    return typeof get === 'function' ? (Reflect.apply(get, settings, [id]) as unknown) : undefined;
  }

  async #set(id: string, value: unknown): Promise<void> {
    const settings = this.#store('setting');
    const set = isRecord(settings) ? settings.set : null;
    if (typeof set === 'function') await (Reflect.apply(set, settings, [id, value]) as unknown);
  }

  #toggle(tabs: Record<string, unknown>, id: string): void {
    const toggle = tabs.toggleSidebarTab;
    if (typeof toggle === 'function') Reflect.apply(toggle, tabs, [id]);
  }
}
