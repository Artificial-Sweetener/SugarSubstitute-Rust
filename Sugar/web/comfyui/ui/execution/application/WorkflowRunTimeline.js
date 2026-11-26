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
 * Own one workflow's place on ComfyUI's execution timeline.
 *
 * ComfyUI runs one job at a time, in an order only it decides. A job that is waiting in the queue
 * has no place on the timeline: it changes nothing a canvas shows until ComfyUI starts it. The
 * frontier is the newest job that started, and every result owner measures "older" against it.
 */
/** Whether `status` ends a run. */
function settles(status) {
    return status === 'complete' || status === 'cancelled' || status === 'error';
}
/** Track the frontier run, the job executing now and the jobs still waiting for one workflow. */
export class WorkflowRunTimeline {
    #waiting = new Set();
    #executing = null;
    #frontier = 0;
    /** How the frontier run ended, or null while it has not. */
    #settled = null;
    /** The order of the newest run ComfyUI started for this workflow. */
    get frontier() {
        return this.#frontier;
    }
    /** Running while a job executes, queued while only waiting jobs remain, else how the last ended. */
    get status() {
        if (this.#executing)
            return 'running';
        if (this.#waiting.size)
            return 'queued';
        return this.#settled ?? 'idle';
    }
    /**
     * Place a run's reported status on the timeline.
     *
     * A queued run only joins the waiting jobs. A run that ends without ever starting, such as one
     * deleted from the queue, only leaves them. A status of a run older than the frontier, or a
     * repeat for a frontier run that already ended, changes nothing.
     */
    follow(run) {
        const id = run.promptId;
        if (run.status === 'queued') {
            if (this.#waiting.has(id) || this.#executing === id)
                return 'ignored';
            this.#waiting.add(id);
            return 'status';
        }
        const waited = this.#waiting.delete(id);
        if (waited && settles(run.status))
            return 'status';
        if (run.order < this.#frontier || (run.order === this.#frontier && this.#settled)) {
            if (this.#executing !== id)
                return waited ? 'status' : 'ignored';
            this.#executing = null;
            return 'status';
        }
        this.#executing =
            run.status === 'running' ? id : this.#executing === id ? null : this.#executing;
        this.#frontier = run.order;
        this.#settled = settles(run.status) ? run.status : null;
        return 'frontier';
    }
    /**
     * Whether a preview of run `id` at `order` may show: only a started run's, at or beyond the
     * frontier, and not after the frontier run ended.
     */
    admits(id, order) {
        return (!this.#waiting.has(id) &&
            (order > this.#frontier || (order === this.#frontier && !this.#settled)));
    }
    /** Record an admitted preview as proof that its run is the one executing. */
    previewed(id, order) {
        this.#executing = id;
        if (order > this.#frontier) {
            this.#frontier = order;
            this.#settled = null;
        }
    }
    /** Resume above restored results without marking any job as waiting or executing. */
    restore(order) {
        this.#waiting.clear();
        this.#executing = null;
        this.#frontier = order;
        this.#settled = null;
    }
}
