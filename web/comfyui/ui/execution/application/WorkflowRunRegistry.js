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
import { freezeWorkflowSubmissionContext } from '../domain/WorkflowSubmission.js';
/** Retain outstanding jobs independently of the visible tab without retaining graph objects or pixels. */
export class WorkflowRunRegistry {
    #runs = new Map();
    #orders = new Map();
    #closed = new Set();
    /** Reserve queue-entry order before transport can reorder response arrival. */
    reserve(context) {
        const order = (this.#orders.get(context.session.sessionId) ?? 0) + 1;
        this.#orders.set(context.session.sessionId, order);
        return Object.freeze({ ...freezeWorkflowSubmissionContext(context), order });
    }
    /** Register a validated queue acceptance once while preserving its captured submission order. */
    accept(context, promptId) {
        const existing = this.#runs.get(promptId);
        if (existing) {
            if (existing.session.sessionId !== context.session.sessionId ||
                existing.order !== context.order)
                throw new Error('An accepted job cannot belong to multiple workflow sessions.');
            return existing;
        }
        const run = Object.freeze({
            ...freezeWorkflowSubmissionContext(context),
            promptId,
            order: context.order,
            status: 'queued',
        });
        this.#runs.set(promptId, run);
        return run;
    }
    /** Resolve only a known accepted job rather than inferring from currently visible node IDs. */
    read(promptId) {
        return this.#runs.get(promptId) ?? null;
    }
    /** Publish immutable execution status while retaining exact source association for recovery. */
    setStatus(promptId, status) {
        const current = this.#runs.get(promptId);
        if (!current)
            return null;
        if (['complete', 'cancelled', 'error'].includes(current.status))
            return current;
        const run = Object.freeze({ ...current, status });
        this.#runs.set(promptId, run);
        return run;
    }
    /** Enumerate outstanding known jobs for one bounded reconnect reconciliation pass. */
    outstanding() {
        return Object.freeze([...this.#runs.values()]);
    }
    /** Retire a completed association once exact-history recovery has settled. */
    retire(promptId) {
        this.#runs.delete(promptId);
        this.#closed.delete(promptId);
    }
    /** Identify a known closed owner's late events without retaining source metadata or pixels. */
    isClosed(promptId) {
        return this.#closed.has(promptId);
    }
    /** Keep only an outstanding job identity when acceptance arrives after its owner has closed. */
    discard(promptId) {
        this.#runs.delete(promptId);
        this.#closed.add(promptId);
    }
    /** Release closed bindings completely; unknown late events remain unassigned and cannot revive them. */
    closeSession(sessionId) {
        for (const [id, run] of this.#runs)
            if (run.session.sessionId === sessionId)
                this.discard(id);
        this.#orders.delete(sessionId);
    }
    /** Release every known association with the owning integration lifetime. */
    dispose() {
        this.#runs.clear();
        this.#orders.clear();
        this.#closed.clear();
    }
}
