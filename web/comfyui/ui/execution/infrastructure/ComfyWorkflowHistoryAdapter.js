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
/** Recover exact known-job outputs through bounded, cancellable public history requests. */
import { isRecord } from '../../types/common.js';
import { decodeComfyMedia } from './ComfyMediaDecoder.js';
/** Share one two-request queue across workflows; never hydrate a global recent-history list. */
export class ComfyWorkflowHistoryAdapter {
    options;
    #api;
    #jobs = new Map();
    #queue = [];
    #active = 0;
    #disposed = false;
    /** Validate public transport before any known job can request reconciliation. */
    constructor(options) {
        this.options = options;
        if (!isRecord(options.api) || typeof options.api.fetchApi !== 'function')
            throw new TypeError('Comfy history transport is unavailable.');
        this.#api = options.api;
    }
    /** Deduplicate one outstanding exact-job request and refuse foreign or obsolete workflow generations. */
    reconcile(promptId) {
        if (this.#disposed)
            return Promise.resolve(0);
        const existing = this.#jobs.get(promptId);
        if (existing) {
            if (existing.controller &&
                !existing.terminalAtStart &&
                this.options.runs.read(promptId)?.status === 'complete')
                existing.rerunAfter = true;
            return existing.promise;
        }
        const run = this.options.runs.read(promptId);
        if (!run ||
            this.options.results.read(run.session.sessionId)?.context.generation !==
                run.session.generation)
            return Promise.resolve(0);
        let complete = null;
        const promise = new Promise((resolve) => {
            complete = resolve;
        });
        const job = {
            run,
            promise,
            controller: null,
            cancelled: false,
            terminalAtStart: false,
            rerunAfter: false,
            resolve(count) {
                if (!complete)
                    throw new Error('History completion is unavailable.');
                complete(count);
            },
        };
        this.#jobs.set(promptId, job);
        this.#queue.push(job);
        this.#pump();
        return promise;
    }
    /** Reconcile the finite set of already-associated outstanding IDs once after reconnection. */
    reconnect() {
        if (this.#disposed)
            return;
        for (const run of this.options.runs.outstanding())
            void this.reconcile(run.promptId);
    }
    /** Cancel queued and active requests belonging to a closed workflow without affecting another tab. */
    cancelSession(sessionId) {
        for (const job of this.#jobs.values())
            if (job.run.session.sessionId === sessionId)
                this.#cancel(job);
        this.#queue = this.#queue.filter((job) => !job.cancelled);
        this.#pump();
    }
    /** Abort transport and settle all callers before retiring this integration lifetime. */
    dispose() {
        if (this.#disposed)
            return;
        this.#disposed = true;
        for (const job of this.#jobs.values())
            this.#cancel(job);
        this.#queue = [];
    }
    /** Admit at most two active requests and release each slot only after its cancellation/timeout settles. */
    #pump() {
        while (!this.#disposed && this.#active < 2 && this.#queue.length) {
            const job = this.#queue.shift();
            if (!job || job.cancelled)
                continue;
            this.#active++;
            void this.#recover(job)
                .catch((error) => {
                this.options.logger.error('SugarCubes history recovery lifecycle failed.', {
                    promptId: job.run.promptId,
                    error,
                });
                job.resolve(0);
            })
                .finally(() => {
                this.#active--;
                if (this.#jobs.get(job.run.promptId) === job)
                    this.#jobs.delete(job.run.promptId);
                if (job.rerunAfter && !job.cancelled && !this.#disposed)
                    void this.reconcile(job.run.promptId);
                this.#pump();
            });
        }
    }
    /** Validate exact history provenance and compare source revisions before publishing recovered descriptors. */
    async #recover(job) {
        const controller = new AbortController();
        job.controller = controller;
        job.terminalAtStart = this.options.runs.read(job.run.promptId)?.status === 'complete';
        this.options.results.updateRecovery(job.run, 'loading');
        const revisions = new Map(job.run.sources.map((source) => [
            source.id,
            this.options.results.sourceRevision(job.run.session.sessionId, source.id),
        ]));
        const timeout = setTimeout(() => controller.abort(new Error('Comfy exact-job history timed out after 15 seconds.')), 15000);
        let rejectAbort = null;
        const aborted = new Promise((_resolve, reject) => {
            rejectAbort = reject;
        });
        const onAbort = () => {
            rejectAbort?.(controller.signal.reason);
        };
        controller.signal.addEventListener('abort', onAbort, { once: true });
        try {
            const payload = await Promise.race([this.#fetch(job, controller.signal), aborted]);
            if (job.cancelled || this.#disposed || this.options.runs.read(job.run.promptId) === null) {
                job.resolve(0);
                return;
            }
            const rawId = job.run.promptId.slice('prompt:'.length);
            const entry = isRecord(payload) && !Array.isArray(payload) ? payload[rawId] : null;
            if (job.terminalAtStart &&
                (!isRecord(entry) || !isRecord(entry.outputs) || Array.isArray(entry.outputs)))
                throw new Error('Comfy did not retain a valid exact history entry for this completed workflow job.');
            let count = 0;
            if (isRecord(entry) && isRecord(entry.outputs) && !Array.isArray(entry.outputs)) {
                for (const source of job.run.sources) {
                    const decoded = decodeComfyMedia(entry.outputs[source.executionPath]);
                    if (!decoded)
                        continue;
                    if (this.options.ingestor.final({
                        promptId: job.run.promptId,
                        executionPaths: [source.executionPath],
                        ...decoded,
                        merge: false,
                    }, revisions.get(source.id) ?? 0))
                        count++;
                }
            }
            this.options.results.updateRecovery(job.run, 'ready');
            if (isRecord(entry) && isRecord(entry.status) && entry.status.completed === true) {
                if (entry.status.status_str === 'success')
                    this.options.ingestor.recovered(job.run.promptId, 'complete');
                else if (entry.status.status_str === 'error')
                    this.options.ingestor.recovered(job.run.promptId, 'error');
            }
            job.resolve(count);
        }
        catch (error) {
            if (!job.cancelled && !this.#disposed) {
                this.options.results.updateRecovery(job.run, 'unavailable', 'Cached workflow outputs could not be recovered.');
                this.options.logger.error('SugarCubes exact-job history recovery failed.', {
                    promptId: job.run.promptId,
                    sessionId: job.run.session.sessionId,
                    error,
                });
            }
            job.resolve(0);
        }
        finally {
            clearTimeout(timeout);
            controller.signal.removeEventListener('abort', onAbort);
            if (job.terminalAtStart && this.options.runs.read(job.run.promptId)?.status === 'complete')
                this.options.runs.retire(job.run.promptId);
        }
    }
    /** Request only the associated job through the public Comfy transport, forwarding abort all the way to JSON parsing. */
    async #fetch(job, signal) {
        const fetchApi = this.#api.fetchApi;
        if (typeof fetchApi !== 'function')
            throw new TypeError('Comfy history transport is unavailable.');
        const id = job.run.promptId.slice('prompt:'.length);
        const response = await fetchApi.call(this.#api, `/history/${encodeURIComponent(id)}`, {
            signal,
        });
        if (!isRecord(response) || response.ok !== true || typeof response.json !== 'function')
            throw new Error('Comfy exact-job history request did not return a successful response.');
        const payload = await response.json.call(response);
        return payload;
    }
    /** Settle ownership cancellation without logging intentional workflow closure as a transport error. */
    #cancel(job) {
        job.cancelled = true;
        job.controller?.abort(new Error('Workflow history recovery cancelled.'));
        if (this.#jobs.get(job.run.promptId) === job)
            this.#jobs.delete(job.run.promptId);
        job.resolve(0);
    }
}
