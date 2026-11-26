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
/**
 * Retire known waiting jobs that ComfyUI no longer holds.
 *
 * A job deleted from ComfyUI's queue produces no execution event, so its workflow would report a
 * waiting job forever. ComfyUI does report how many jobs remain; when that is fewer than the jobs
 * known here, the queue is read once and every known waiting job found in neither the queue nor
 * the job history is ended as cancelled.
 */
import { isRecord } from '../../types/common.js';
/** Read ComfyUI's queue only when its own count proves a known job is gone. */
export class ComfyWorkflowQueueReconciler {
    options;
    #reading = false;
    #again = false;
    /** Bind the run owners and the public Comfy transport. */
    constructor(options) {
        this.options = options;
    }
    /** Receive ComfyUI's count of jobs still running or waiting. */
    remaining(count) {
        const known = this.options.runs
            .outstanding()
            .filter((run) => run.status === 'queued' || run.status === 'running').length;
        if (count >= known)
            return;
        if (this.#reading) {
            this.#again = true;
            return;
        }
        this.#reading = true;
        void this.#reconcile()
            .catch((error) => {
            this.options.logger.error('SugarCubes queue reconciliation failed.', { error });
        })
            .finally(() => {
            this.#reading = false;
            if (!this.#again)
                return;
            this.#again = false;
            this.remaining(count);
        });
    }
    /** End each job that was waiting before the read and is in neither the queue nor the history. */
    async #reconcile() {
        const waiting = this.#waiting();
        if (!waiting.length)
            return;
        const queue = await this.#json('/queue');
        const held = new Set();
        if (isRecord(queue))
            for (const key of ['queue_running', 'queue_pending'])
                for (const entry of Array.isArray(queue[key]) ? queue[key] : [])
                    if (Array.isArray(entry) && typeof entry[1] === 'string')
                        held.add(entry[1]);
        for (const id of waiting) {
            const raw = id.slice('prompt:'.length);
            if (held.has(raw) || !this.#waiting().includes(id))
                continue;
            // A job that already ran is in the history: its own events, not this owner, settle it.
            const history = await this.#json(`/history/${encodeURIComponent(raw)}`);
            if (isRecord(history) && isRecord(history[raw]))
                continue;
            if (this.#waiting().includes(id))
                this.options.ingestor.terminal(id, 'cancelled');
        }
    }
    /** The known jobs ComfyUI has not started. */
    #waiting() {
        return this.options.runs
            .outstanding()
            .filter((run) => run.status === 'queued')
            .map((run) => run.promptId);
    }
    /** Read one JSON document through the public Comfy transport. */
    async #json(path) {
        const api = this.options.api;
        if (!isRecord(api) || typeof api.fetchApi !== 'function')
            throw new TypeError('Comfy queue transport is unavailable.');
        const controller = new AbortController();
        const timeout = setTimeout(() => controller.abort(new Error(`Comfy ${path} timed out after 15 seconds.`)), 15000);
        try {
            const response = await api.fetchApi.call(api, path, { signal: controller.signal });
            if (!isRecord(response) || response.ok !== true || typeof response.json !== 'function')
                throw new Error(`Comfy ${path} request did not return a successful response.`);
            const payload = await response.json.call(response);
            return payload;
        }
        finally {
            clearTimeout(timeout);
        }
    }
}
