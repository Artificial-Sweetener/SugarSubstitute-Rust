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
/** Coordinate accepted jobs, bounded early final events and exact recovery requests. */
import { freezeWorkflowMedia } from '../domain/WorkflowMedia.js';
/** Keep execution attribution independent of whichever canvas or graph is currently mounted. */
export class WorkflowExecutionIngestor {
    options;
    #early = new Map();
    #earlyFinals = 0;
    #cancelExpiry = null;
    #disposed = false;
    /** Bind focused state owners, a deterministic clock and a single history-recovery collaborator. */
    constructor(options) {
        this.options = options;
    }
    /** Capture submission order while leaving its authoritative sequence with the run registry. */
    submitting(context) {
        if (this.#disposed)
            throw new Error('Workflow execution ingestion is disposed.');
        const submission = this.options.runs.reserve(context);
        this.options.generation?.publish({ type: 'submitted', submission });
        return submission;
    }
    /** Publish acceptance and replay attributable early finals before terminal recovery is requested. */
    accepted(context, promptId) {
        if (this.#disposed)
            return;
        const current = this.options.results.read(context.session.sessionId);
        if (!current || current.context.generation !== context.session.generation) {
            this.rejected(context);
            this.options.runs.discard(promptId);
            if (this.#early.get(promptId)?.terminal)
                this.options.runs.retire(promptId);
            this.#removeEarly(promptId);
            return;
        }
        const run = this.options.runs.accept(context, promptId);
        this.options.generation?.publish({ type: 'accepted', submission: context, run });
        this.options.results.updateRun(run);
        this.#expire();
        const early = this.#early.get(promptId);
        if (!early)
            return;
        this.#removeEarly(promptId);
        if (early.started) {
            const started = this.options.runs.setStatus(promptId, 'running');
            if (started)
                this.options.results.updateRun(started);
        }
        for (const event of early.finals)
            this.final(event);
        if (early.terminal)
            this.terminal(promptId, early.terminal);
    }
    /** Resolve an exact known job and path only while its workflow generation remains valid. */
    resolveSource(promptId, executionPaths) {
        if (this.#disposed)
            return null;
        const run = this.options.runs.read(promptId);
        if (!run)
            return null;
        const session = this.options.results.read(run.session.sessionId);
        if (!session || session.context.generation !== run.session.generation)
            return null;
        for (const path of executionPaths) {
            const source = run.sources.find((candidate) => candidate.executionPath === path);
            if (source)
                return { run, source };
        }
        return null;
    }
    /** Attribute a final or hold it within the finite early-event limits until acceptance arrives. */
    final(event, expectedRevision) {
        if (this.#disposed)
            return false;
        if (this.options.runs.isClosed(event.promptId))
            return false;
        const attributed = this.resolveSource(event.promptId, event.executionPaths);
        if (attributed?.source.transientOnly)
            return false;
        if (!attributed) {
            if (!this.options.runs.read(event.promptId)) {
                const early = this.#stage(event.promptId);
                if (this.#earlyFinals >= 64) {
                    this.options.logger.warn('SugarCubes early output limit reached; known jobs use exact history recovery.', {
                        promptId: event.promptId,
                        limit: 64,
                    });
                    return false;
                }
                early.finals.push(event);
                this.#earlyFinals++;
            }
            return false;
        }
        const retained = this.options.results.retain({
            ...attributed,
            payload: event.payload,
            containsImages: event.containsImages,
            merge: event.merge,
            delivery: event.delivery,
            ...(expectedRevision !== undefined ? { expectedRevision } : {}),
        });
        if (retained && event.containsImages)
            this.options.imageAccepted?.(attributed.run.session.sessionId);
        if (retained || expectedRevision === undefined)
            this.options.generation?.publish({
                type: 'output',
                run: attributed.run,
                source: attributed.source,
                payload: freezeWorkflowMedia(event.payload),
                retained,
            });
        return retained;
    }
    /** Publish a job-qualified start, staging early transport events until queue acceptance. */
    started(promptId) {
        if (this.#disposed)
            return;
        if (this.options.runs.isClosed(promptId))
            return;
        const run = this.options.runs.setStatus(promptId, 'running');
        if (run)
            this.options.results.updateRun(run);
        else
            this.#stage(promptId).started = true;
    }
    /** Publish running status only for an explicitly attributable job and execution path. */
    executing(promptId, executionPaths) {
        if (this.#disposed)
            return;
        const attributed = this.resolveSource(promptId, executionPaths);
        if (!attributed || ['complete', 'cancelled', 'error'].includes(attributed.run.status))
            return;
        const run = this.options.runs.setStatus(promptId, 'running');
        if (run)
            this.options.results.updateRun(run);
    }
    /** Set terminal status, retain the last valid finals and reconcile only this known successful job. */
    terminal(promptId, status) {
        if (this.#disposed)
            return;
        if (this.options.runs.isClosed(promptId)) {
            this.options.runs.retire(promptId);
            return;
        }
        const current = this.options.runs.read(promptId);
        if (!current) {
            this.#stage(promptId).terminal = status;
            return;
        }
        if (['complete', 'cancelled', 'error'].includes(current.status))
            return;
        const run = this.options.runs.setStatus(promptId, status);
        if (run) {
            this.options.results.updateRun(run);
            this.options.generation?.publish({ type: 'terminal', run });
        }
        if (status === 'complete')
            this.options.recover(promptId);
        else
            this.retire(promptId);
    }
    /** Recover cached output nodes without treating global history as a workflow association. */
    cached(promptId) {
        if (this.#disposed)
            return;
        if (this.options.runs.read(promptId))
            this.options.recover(promptId);
    }
    /** Commit a terminal status proven by exact history without recursively scheduling another recovery. */
    recovered(promptId, status) {
        if (this.#disposed)
            return;
        const run = this.options.runs.setStatus(promptId, status);
        if (!run)
            return;
        this.options.results.updateRun(run);
        this.options.generation?.publish({ type: 'terminal', run });
        this.retire(promptId);
    }
    /** Release submission-owned destinations when preparation or queue acceptance fails. */
    rejected(submission) {
        this.options.generation?.publish({ type: 'rejected', submission });
    }
    /** Retire exact recovery ownership only after its final output transitions have been published. */
    retire(promptId) {
        this.options.generation?.publish({ type: 'settled', promptId });
        this.options.runs.retire(promptId);
    }
    /** Clear transient previews after a disconnect while preserving known finals and jobs for recovery. */
    disconnected() {
        for (const run of this.options.runs.outstanding())
            this.options.results.clearPreview(run.session.sessionId);
    }
    /** Drop closed workflow metadata while retaining minimal outstanding late-event rejection identities. */
    closeSession(sessionId) {
        this.options.generation?.publish({ type: 'closed', sessionId });
        this.options.runs.closeSession(sessionId);
    }
    /** Release the finite early queue and its sole timer with the integration lifetime. */
    dispose() {
        if (this.#disposed)
            return;
        this.#disposed = true;
        this.#cancelExpiry?.();
        this.#cancelExpiry = null;
        this.#early.clear();
        this.#earlyFinals = 0;
        this.disconnected();
    }
    /** Admit at most sixteen unknown jobs and schedule one expiry rather than polling. */
    #stage(promptId) {
        this.#expire();
        const existing = this.#early.get(promptId);
        if (existing)
            return existing;
        if (this.#early.size >= 16) {
            const oldest = this.#early.keys().next().value;
            if (oldest)
                this.#removeEarly(oldest);
            this.options.logger.warn('SugarCubes early job limit reached; evicting the oldest unassigned metadata.', { limit: 16 });
        }
        const early = {
            expires: this.options.clock.now() + 30000,
            finals: [],
            started: false,
            terminal: null,
        };
        this.#early.set(promptId, early);
        this.#scheduleExpiry();
        return early;
    }
    /** Remove all expired metadata without retaining any preview bytes. */
    #expire() {
        for (const [id, early] of this.#early)
            if (early.expires <= this.options.clock.now())
                this.#removeEarly(id);
    }
    /** Release one early job's final count before replay, eviction or expiry. */
    #removeEarly(promptId) {
        this.#earlyFinals -= this.#early.get(promptId)?.finals.length ?? 0;
        this.#early.delete(promptId);
        if (this.#early.size === 0) {
            this.#cancelExpiry?.();
            this.#cancelExpiry = null;
        }
    }
    /** Maintain one deadline for the oldest staged job, rescheduling only after it expires. */
    #scheduleExpiry() {
        if (this.#cancelExpiry || !this.#early.size)
            return;
        const first = this.#early.values().next().value;
        if (!first)
            return;
        this.#cancelExpiry = this.options.clock.later(Math.max(0, first.expires - this.options.clock.now()), () => {
            this.#cancelExpiry = null;
            this.#expire();
            this.#scheduleExpiry();
        });
    }
}
