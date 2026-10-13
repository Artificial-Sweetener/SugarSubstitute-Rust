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
/** Translate public workflow-store objects into explicit in-memory session bindings. */

import { isRecord, type UnknownRecord } from '../../types/common.js';
import { workflowSessionId, type WorkflowSessionId } from '../domain/ExecutionIdentity.js';
import type { WorkflowSessionBinding, WorkflowLoadToken } from '../domain/WorkflowSession.js';
import type { WorkflowSessionStore } from '../application/WorkflowSessionStore.js';

export interface ComfyWorkflowSessionAdapterOptions {
  readonly app: unknown;
  readonly sessions: WorkflowSessionStore;
  readonly nextKey: () => string;
  readonly logger: Pick<Console, 'error'>;
}

/** Bind identities to host workflow objects and retire closed browser bindings explicitly. */
export class ComfyWorkflowSessionAdapter {
  readonly #workflow: UnknownRecord;
  readonly #identities = new WeakMap<object, WorkflowSessionId>();
  readonly #open = new Map<WorkflowSessionId, UnknownRecord>();
  #unsubscribe: (() => void) | null = null;
  #active = false;

  /** Validate the host's public store without importing private Pinia implementations. */
  constructor(private readonly options: ComfyWorkflowSessionAdapterOptions) {
    const app = options.app;
    const manager = isRecord(app) && isRecord(app.extensionManager) ? app.extensionManager : null;
    if (!manager || !isRecord(manager.workflow))
      throw new TypeError('Comfy workflow store is unavailable.');
    this.#workflow = manager.workflow;
  }

  /** Observe synchronous store commitment and explicit open/close transitions. */
  install(): void {
    if (this.#active) return;
    const subscribe = this.#workflow.$subscribe;
    if (typeof subscribe !== 'function')
      throw new TypeError('Comfy workflow subscription is unavailable.');
    this.#active = true;
    try {
      const unsubscribe: unknown = subscribe.call(this.#workflow, () => this.refresh(), {
        detached: true,
        flush: 'sync',
      });
      if (typeof unsubscribe !== 'function')
        throw new TypeError('Comfy workflow unsubscribe is unavailable.');
      this.#unsubscribe = () => {
        unsubscribe();
      };
      this.refresh();
    } catch (error: unknown) {
      this.#active = false;
      this.#unsubscribe?.();
      this.#unsubscribe = null;
      throw error;
    }
  }

  /** Resynchronize after a fully observed graph load without inferring ownership from graph IDs. */
  refresh(): boolean {
    if (!this.#active) return false;
    try {
      const workflows = this.#workflow.openWorkflows;
      if (!Array.isArray(workflows) || !workflows.every(isRecord))
        throw new TypeError('Comfy open workflows are malformed.');
      const bindings: WorkflowSessionBinding[] = [];
      const nextOpen = new Map<WorkflowSessionId, UnknownRecord>();
      let selected: WorkflowSessionId | null = null;
      for (const workflow of workflows) {
        let sessionId = this.#identities.get(workflow);
        if (!sessionId) {
          sessionId = workflowSessionId(this.options.nextKey());
          this.#identities.set(workflow, sessionId);
        }
        nextOpen.set(sessionId, workflow);
        bindings.push({
          sessionId,
          title: text(workflow.filename) || text(workflow.title) || 'Untitled workflow',
          path: text(workflow.path),
        });
        if (workflow === this.#workflow.activeWorkflow) selected = sessionId;
      }
      const previousOpen = new Map(this.#open);
      this.#open.clear();
      for (const [id, workflow] of nextOpen) this.#open.set(id, workflow);
      const closed = this.options.sessions.synchronize(bindings, selected);
      for (const sessionId of closed) {
        const previous = previousOpen.get(sessionId);
        if (previous) this.#identities.delete(previous);
      }
      return true;
    } catch (error: unknown) {
      this.options.sessions.suspend();
      this.options.logger.error('SugarCubes workflow session observation failed.', { error });
      return false;
    }
  }

  /** Distinguish native history, exact stored-state activation, and actual graph replacement. */
  loadKind(args: readonly unknown[]): WorkflowLoadToken['kind'] {
    if (this.#isHistoryRestore(args)) return 'history';
    const target = args[3];
    const tracker =
      isRecord(target) && isRecord(target.changeTracker) ? target.changeTracker : null;
    if (
      args[1] === true &&
      args[2] === true &&
      tracker &&
      isRecord(args[0]) &&
      args[0] === tracker.activeState &&
      [...this.#open.values()].some((workflow) => workflow.changeTracker === tracker)
    )
      return 'activate';
    return 'replace';
  }

  /** Qualify history against the active workflow's restoring tracker rather than graph identity. */
  #isHistoryRestore(args: readonly unknown[]): boolean {
    const target = args[3];
    const active = this.#workflow.activeWorkflow;
    if (args[1] !== false || args[2] !== false || !isRecord(target) || !isRecord(active))
      return false;
    const tracker = active.changeTracker;
    // Comfy passes the raw workflow while Pinia exposes its proxy; the tracker is markRaw.
    return (
      isRecord(tracker) && target.changeTracker === tracker && tracker._restoringState === true
    );
  }

  /** Read only the named workflow's host-owned output snapshot for validated restoration. */
  readStoredOutputs(sessionId: WorkflowSessionId): unknown {
    const workflow = this.#open.get(sessionId);
    const tracker = workflow && isRecord(workflow.changeTracker) ? workflow.changeTracker : null;
    return tracker?.nodeOutputs;
  }

  /** Read the same named workflow's serialized topology for validating restoration source membership. */
  readStoredWorkflow(sessionId: WorkflowSessionId): unknown {
    const workflow = this.#open.get(sessionId);
    const tracker = workflow && isRecord(workflow.changeTracker) ? workflow.changeTracker : null;
    return tracker?.activeState;
  }

  /** Stop this adapter's subscription while leaving session/resource teardown to their owners. */
  dispose(): void {
    if (!this.#active) return;
    this.#active = false;
    this.#unsubscribe?.();
    this.#unsubscribe = null;
    this.#open.clear();
  }
}

/** Normalize display metadata without using it as an identity or association key. */
function text(value: unknown): string {
  return typeof value === 'string' ? value.trim() : '';
}
