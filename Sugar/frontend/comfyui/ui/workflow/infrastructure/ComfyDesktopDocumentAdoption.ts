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
/** Give a restored tab the document its checkpoint saved when Comfy brought back an older one. */
import type { SessionDocument } from '../../../../../integrations/contracts/SessionProtocol.generated.js';
import { isRecord, type UnknownRecord } from '../../types/common.js';
import type { WorkflowSessionContext } from '../../execution/domain/WorkflowSession.js';
import type { WorkflowSessionStore } from '../../execution/application/WorkflowSessionStore.js';
import type { ComfyWorkflowSessionAdapter } from '../../execution/infrastructure/ComfyWorkflowSessionAdapter.js';
import { DesktopSessionRefusal } from '../domain/DesktopSession.js';
import type { ComfyDesktopDocuments } from './ComfyDesktopDocuments.js';
import type { ComfyDraftTimes } from './ComfyWorkflowDraftTimes.js';

export interface ComfyDesktopDocumentAdoptionOptions {
  readonly app: unknown;
  readonly sessions: WorkflowSessionStore;
  readonly adapter: Pick<ComfyWorkflowSessionAdapter, 'readHostWorkflow' | 'readLoadSequence'>;
  readonly documents: Pick<ComfyDesktopDocuments, 'capture'>;
  /** When Comfy last kept its own copy of a workflow, as the previous session left it. */
  readonly draftTimes: ComfyDraftTimes;
}

/**
 * Decide which copy of a restored workflow is the newer one, and make the tab show it.
 *
 * Comfy reopens its tabs from drafts it keeps in page storage, which the browser writes to disk
 * lazily: a page that died without closing comes back with drafts older than its last edits. The
 * desktop checkpoint is written durably within moments of every edit, so it is the newer copy
 * unless Comfy's own copy is known to be later: a draft or a saved file written after the
 * checkpoint was captured. Comfy still owns the tabs; only a matched tab's document is replaced.
 */
export class ComfyDesktopDocumentAdoption {
  readonly #app: UnknownRecord;

  /** Validate the graph-load capability once; without it no document can be adopted. */
  constructor(private readonly options: ComfyDesktopDocumentAdoptionOptions) {
    if (!isRecord(options.app) || typeof options.app.loadGraphData !== 'function')
      throw new TypeError('Comfy desktop graph loading is unavailable.');
    this.#app = options.app;
  }

  /**
   * Replace the tab's document with the checkpoint's when the checkpoint is the newer copy.
   *
   * Returns whether the tab changed. A tab Comfy is opening for the person is left to that load.
   */
  async adopt(
    document: SessionDocument,
    context: WorkflowSessionContext,
    capturedAt: number,
    current: () => boolean,
  ): Promise<boolean> {
    const workflow = this.options.adapter.readHostWorkflow(context.sessionId);
    if (!isRecord(workflow) || workflow.isLoading === true) return false;
    const active = this.options.sessions.read().active?.sessionId === context.sessionId;
    if (!active && !isRecord(workflow.changeTracker)) {
      // Comfy applies its draft when it builds the tracker; build it before replacing its state.
      await invoke(workflow, 'load');
      assertCurrent(current);
    }
    if (this.options.documents.capture(context, 0).graphJson === document.graphJson) return false;
    const draft = this.options.draftTimes(text(workflow.path) ?? '');
    if (draft === 'unreadable') return false;
    const saved = workflow.isTemporary === true ? null : time(workflow.lastModified);
    if (capturedAt <= Math.max(draft ?? -Infinity, saved ?? -Infinity)) return false;
    const graph = parseGraph(document.graphJson);
    const view = { scale: document.view.scale, offset: [document.view.x, document.view.y] };
    if (active) {
      await this.#load(graph, workflow, current);
      const canvas = this.#app.canvas;
      if (isRecord(canvas) && isRecord(canvas.ds)) Object.assign(canvas.ds, view);
    }
    const tracker = workflow.changeTracker;
    if (!isRecord(tracker)) throw new TypeError('Comfy did not track the adopted workflow.');
    if (!active) {
      tracker.activeState = graph;
      tracker.ds = view;
      workflow.content = document.graphJson;
    }
    // Comfy's load makes the loaded graph its own baseline; the saved baseline says what is unsaved.
    tracker.initialState = parseGraph(document.baselineGraphJson ?? document.graphJson);
    workflow.isModified = document.dirty;
    return true;
  }

  /** Admit one owned public load and refuse any competing load before more restore effects. */
  async #load(
    graph: UnknownRecord,
    workflow: UnknownRecord,
    current: () => boolean,
  ): Promise<void> {
    const before = this.options.adapter.readLoadSequence();
    await invoke(this.#app, 'loadGraphData', graph, true, true, workflow);
    assertCurrent(current);
    if (
      this.options.adapter.readLoadSequence() !== before + 1 ||
      this.options.sessions.read().loading
    )
      throw new DesktopSessionRefusal(
        'stale',
        'Comfy desktop materialization was superseded by another graph load.',
      );
  }
}

/** Call a qualified exposed method with its owning receiver. */
function invoke(owner: UnknownRecord, name: string, ...args: readonly unknown[]): unknown {
  if (typeof owner[name] !== 'function')
    throw new TypeError(`Comfy desktop capability ${name} is unavailable.`);
  return Reflect.apply(owner[name], owner, args);
}

/** Refuse late effects after cancellation or a competing user load. */
function assertCurrent(current: () => boolean): void {
  if (!current())
    throw new DesktopSessionRefusal('cancelled', 'Comfy desktop materialization was superseded.');
}

/** Parse a graph the checkpoint admission already validated. */
function parseGraph(value: string): UnknownRecord {
  const graph: unknown = JSON.parse(value);
  if (!isRecord(graph) || Array.isArray(graph))
    throw new TypeError('Comfy graph must be an object.');
  return graph;
}

/** Keep a nonempty string; anything else is no path. */
function text(value: unknown): string | null {
  return typeof value === 'string' && value.length > 0 ? value : null;
}

/** Keep a finite time; anything else is no time. */
function time(value: unknown): number | null {
  return typeof value === 'number' && Number.isFinite(value) ? value : null;
}
