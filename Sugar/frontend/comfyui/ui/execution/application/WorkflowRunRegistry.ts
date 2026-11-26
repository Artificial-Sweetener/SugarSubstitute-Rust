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
/** Own known-job association and remove routing metadata when jobs or bindings retire. */

import type { PromptId, WorkflowSessionId } from '../domain/ExecutionIdentity.js';
import type { WorkflowRun } from '../domain/WorkflowRun.js';
import type {
  WorkflowSubmissionContext,
  WorkflowOrderedSubmission,
} from '../domain/WorkflowSubmission.js';
import { freezeWorkflowSubmissionContext } from '../domain/WorkflowSubmission.js';
import type { WorkflowRunStatus } from '../domain/WorkflowOutput.js';

/** Retain outstanding job ownership and captured return plans independently of visible graph objects. */
export class WorkflowRunRegistry {
  readonly #runs = new Map<PromptId, WorkflowRun>();
  /** The order each run was submitted with, which its run order leaves when it starts out of turn. */
  readonly #submitted = new Map<PromptId, number>();
  readonly #orders = new Map<WorkflowSessionId, number>();
  /** The highest order of any run of the session that has started executing. */
  readonly #started = new Map<WorkflowSessionId, number>();
  readonly #closed = new Set<PromptId>();

  /** Reserve queue-entry order before transport can reorder response arrival. */
  reserve(context: WorkflowSubmissionContext): WorkflowOrderedSubmission {
    const order = (this.#orders.get(context.session.sessionId) ?? 0) + 1;
    this.#orders.set(context.session.sessionId, order);
    return Object.freeze({ ...freezeWorkflowSubmissionContext(context), order });
  }

  /** Expose the sole queue-order authority without requiring a transport receipt or active job. */
  latestOrder(session: WorkflowSessionId): number {
    return this.#orders.get(session) ?? 0;
  }

  /** Resume queue ordering above restored presentation without recreating a job or replaying execution. */
  restoreOrder(session: WorkflowSessionId, order: number): void {
    if (!Number.isSafeInteger(order) || order < 0)
      throw new RangeError('Restored workflow submission order is invalid.');
    this.#orders.set(session, Math.max(this.#orders.get(session) ?? 0, order));
  }

  /** Register a validated queue acceptance once while preserving its captured submission order. */
  accept(context: WorkflowOrderedSubmission, promptId: PromptId): WorkflowRun {
    const existing = this.#runs.get(promptId);
    if (existing) {
      if (
        existing.session.sessionId !== context.session.sessionId ||
        this.#submitted.get(promptId) !== context.order
      )
        throw new Error('An accepted job cannot belong to multiple workflow sessions.');
      return existing;
    }
    const run = Object.freeze({
      ...freezeWorkflowSubmissionContext(context),
      promptId,
      order: context.order,
      status: 'queued' as const,
    });
    this.#runs.set(promptId, run);
    this.#submitted.set(promptId, context.order);
    return run;
  }

  /** Resolve only a known accepted job rather than inferring from currently visible node IDs. */
  read(promptId: PromptId): WorkflowRun | null {
    return this.#runs.get(promptId) ?? null;
  }

  /**
   * Publish immutable execution status while retaining exact source association for recovery.
   *
   * A run is ordered by submission until it starts. ComfyUI can start waiting jobs in another
   * order (a reordered queue, a job queued at the front, or a later submission accepted first),
   * and every result owner treats a lower order as an older run. A run that starts after a run
   * with a higher order has started therefore takes the next order, so results follow the order
   * in which ComfyUI actually ran the jobs.
   */
  setStatus(promptId: PromptId, status: WorkflowRunStatus): WorkflowRun | null {
    const current = this.#runs.get(promptId);
    if (!current) return null;
    if (['complete', 'cancelled', 'error'].includes(current.status)) return current;
    const run = Object.freeze({
      ...current,
      status,
      order:
        status === 'running' && current.status === 'queued'
          ? this.#startOrder(current)
          : current.order,
    });
    this.#runs.set(promptId, run);
    return run;
  }

  /** Enumerate outstanding known jobs for one bounded reconnect reconciliation pass. */
  outstanding(): readonly WorkflowRun[] {
    return Object.freeze([...this.#runs.values()]);
  }

  /** Retire a completed association once exact-history recovery has settled. */
  retire(promptId: PromptId): void {
    this.#runs.delete(promptId);
    this.#submitted.delete(promptId);
    this.#closed.delete(promptId);
  }

  /** Identify a known closed owner's late events without retaining source metadata or pixels. */
  isClosed(promptId: PromptId): boolean {
    return this.#closed.has(promptId);
  }

  /** Keep only an outstanding job identity when acceptance arrives after its owner has closed. */
  discard(promptId: PromptId): void {
    this.#runs.delete(promptId);
    this.#submitted.delete(promptId);
    this.#closed.add(promptId);
  }

  /** Release closed bindings completely; unknown late events remain unassigned and cannot revive them. */
  closeSession(sessionId: WorkflowSessionId): void {
    for (const [id, run] of this.#runs) if (run.session.sessionId === sessionId) this.discard(id);
    this.#orders.delete(sessionId);
    this.#started.delete(sessionId);
  }

  /** Release every known association with the owning integration lifetime. */
  dispose(): void {
    this.#runs.clear();
    this.#submitted.clear();
    this.#orders.clear();
    this.#started.clear();
    this.#closed.clear();
  }

  /** Keep a starting run's order unless a run with a higher order already started before it. */
  #startOrder(run: WorkflowRun): number {
    const session = run.session.sessionId;
    const started = this.#started.get(session) ?? 0;
    // The next order sits above every reservation so far; a waiting job passed over this way is
    // renumbered again when it starts.
    const order =
      run.order > started ? run.order : Math.max(this.#orders.get(session) ?? 0, run.order) + 1;
    if (order !== run.order) this.#orders.set(session, order);
    this.#started.set(session, order);
    return order;
  }
}
