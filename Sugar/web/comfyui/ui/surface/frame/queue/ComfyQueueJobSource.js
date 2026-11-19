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
/** Read ComfyUI's jobs, the running job's progress and each job's workflow name. */
import { isRecord } from '../../../types/common.js';
import { NO_QUEUE_PROGRESS, } from './GenerationQueueRows.js';
const ACTIVE = '/jobs?status=in_progress,pending&limit=200&offset=0';
const HISTORY_LIMIT = 64;
const HISTORY = `/jobs?status=completed,failed,cancelled&limit=${String(HISTORY_LIMIT)}&offset=0`;
/** ComfyUI's events after which its job lists may have changed. */
const REFRESH_EVENTS = [
    'status',
    'execution_start',
    'execution_success',
    'execution_error',
    'execution_interrupted',
    'reconnected',
];
const STATUSES = {
    pending: 'pending',
    in_progress: 'running',
    completed: 'completed',
    failed: 'failed',
    cancelled: 'cancelled',
};
/**
 * Own the queue's view of ComfyUI's jobs.
 *
 * ComfyUI's job lists do not carry the workflow a job was queued from. The page that queues a job
 * records its workflow tab's name in the job's extra data, so the name of a waiting or running job
 * is read from ComfyUI's queue, and the name of a finished job from that job's own record, once
 * each. Reads that fail are reported and leave the last good snapshot in place.
 */
export class ComfyQueueJobSource {
    api;
    report;
    nodeName;
    urls;
    #names = new Map();
    #listeners = new Set();
    #onRefresh = () => void this.refresh();
    #onProgress = (event) => this.#progress(event);
    #onPreview = (event) => this.#previewed(event);
    #snapshot = { jobs: [], progress: NO_QUEUE_PROGRESS };
    #preview = null;
    #reading = false;
    #again = false;
    #started = false;
    constructor(api, report, 
    /** Name a node of the running workflow for the person, or null when it is not on the page. */
    nodeName = () => null, urls = URL) {
        this.api = api;
        this.report = report;
        this.nodeName = nodeName;
        this.urls = urls;
    }
    /** The latest snapshot, for a view that opens between changes. */
    get snapshot() {
        return this.#snapshot;
    }
    /** Follow ComfyUI's queue until the returned function is called. */
    subscribe(listener) {
        this.#listeners.add(listener);
        if (!this.#started) {
            this.#started = true;
            for (const type of REFRESH_EVENTS)
                this.api.addEventListener(type, this.#onRefresh);
            this.api.addEventListener('progress_state', this.#onProgress);
            this.api.addEventListener('b_preview_with_metadata', this.#onPreview);
            void this.refresh();
        }
        return () => {
            this.#listeners.delete(listener);
            if (this.#listeners.size > 0 || !this.#started)
                return;
            this.#started = false;
            for (const type of REFRESH_EVENTS)
                this.api.removeEventListener(type, this.#onRefresh);
            this.api.removeEventListener('progress_state', this.#onProgress);
            this.api.removeEventListener('b_preview_with_metadata', this.#onPreview);
            this.#forgetPreview();
        };
    }
    /** Read ComfyUI's jobs again; overlapping requests collapse into one further read. */
    async refresh() {
        if (this.#reading) {
            this.#again = true;
            return;
        }
        this.#reading = true;
        try {
            do {
                this.#again = false;
                const [active, history] = await Promise.all([this.#list(ACTIVE), this.#list(HISTORY)]);
                const records = [...active, ...history];
                await this.#learnNames(records);
                const jobs = records.flatMap((record) => this.#job(record));
                const running = jobs.some((job) => job.status === 'running');
                // Once nothing runs, what was said about the last job must not sit on the next one.
                if (!running)
                    this.#forgetPreview();
                this.#publish({ jobs, progress: running ? this.#snapshot.progress : NO_QUEUE_PROGRESS });
            } while (this.#again);
        }
        catch (error) {
            this.report(error);
        }
        finally {
            this.#reading = false;
        }
    }
    async #json(route) {
        const response = await this.api.fetchApi(route);
        if (!response.ok)
            throw new Error(`ComfyUI answered ${String(response.status)} for ${route}`);
        return (await response.json());
    }
    async #list(route) {
        const reply = await this.#json(route);
        const jobs = isRecord(reply) && Array.isArray(reply.jobs) ? reply.jobs : [];
        return jobs.filter(isRecord);
    }
    /** Learn the workflow name of every job not yet asked about. */
    async #learnNames(records) {
        const unknown = records.filter((record) => typeof record.id === 'string' && !this.#names.has(record.id));
        if (unknown.some((record) => record.status === 'pending' || record.status === 'in_progress')) {
            const queue = await this.#json('/queue');
            for (const key of ['queue_running', 'queue_pending']) {
                const entries = isRecord(queue) && Array.isArray(queue[key]) ? queue[key] : [];
                for (const entry of entries) {
                    if (!Array.isArray(entry) || typeof entry[1] !== 'string')
                        continue;
                    this.#names.set(entry[1], workflowName(entry[3]));
                }
            }
        }
        await Promise.all(unknown
            .filter((record) => !this.#names.has(record.id))
            .filter((record) => record.status !== 'pending' && record.status !== 'in_progress')
            .map(async (record) => {
            const id = record.id;
            try {
                const detail = await this.#json(`/jobs/${encodeURIComponent(id)}`);
                const workflow = isRecord(detail) && isRecord(detail.workflow) ? detail.workflow : null;
                this.#names.set(id, workflowName(workflow?.extra_data));
            }
            catch (error) {
                // One unreadable record must not hide the queue; the job is named as ComfyUI names it.
                this.#names.set(id, null);
                this.report(error);
            }
        }));
        // Forget jobs ComfyUI no longer lists, so the cache cannot grow without bound.
        const listed = new Set(records.map((record) => record.id));
        for (const id of [...this.#names.keys()])
            if (!listed.has(id))
                this.#names.delete(id);
    }
    #job(record) {
        const status = typeof record.status === 'string' ? STATUSES[record.status] : undefined;
        if (typeof record.id !== 'string' || !status)
            return [];
        const preview = isRecord(record.preview_output) ? record.preview_output : null;
        const error = isRecord(record.execution_error) ? record.execution_error : null;
        return [
            {
                id: record.id,
                status,
                priority: number(record.priority) ?? 0,
                createTime: number(record.create_time),
                startTime: number(record.execution_start_time),
                endTime: number(record.execution_end_time),
                workflowName: this.#names.get(record.id) ?? null,
                runNumber: null,
                workflowId: typeof record.workflow_id === 'string' ? record.workflow_id : null,
                outputFile: preview && typeof preview.filename === 'string' ? preview.filename : null,
                outputSubfolder: preview && typeof preview.subfolder === 'string' ? preview.subfolder : null,
                outputKind: mediaKind(preview),
                previewUrl: previewUrl(preview),
                outputsCount: number(record.outputs_count) ?? 0,
                error: error && typeof error.exception_message === 'string' ? error.exception_message : null,
                errorDetail: error,
            },
        ];
    }
    /** Total progress is the share of the job's nodes that are done, counting a node part way. */
    #progress(event) {
        const detail = Reflect.get(event, 'detail');
        const nodes = isRecord(detail) && isRecord(detail.nodes) ? Object.entries(detail.nodes) : [];
        if (nodes.length === 0)
            return;
        let done = 0;
        let node = null;
        let nodePercent = null;
        for (const [id, state] of nodes) {
            if (!isRecord(state))
                continue;
            const max = number(state.max) ?? 0;
            const share = max > 0 ? Math.min(1, Math.max(0, (number(state.value) ?? 0) / max)) : null;
            if (state.state === 'finished')
                done += 1;
            else if (state.state === 'running') {
                done += share ?? 0;
                if (node === null) {
                    const shown = typeof state.display_node_id === 'string' ? state.display_node_id : id;
                    node = this.nodeName(shown) ?? this.nodeName(id);
                    nodePercent = share === null ? null : Math.round(share * 100);
                }
            }
        }
        const percent = Math.round((done / nodes.length) * 100);
        const last = this.#snapshot.progress;
        if (percent === last.percent && node === last.node && nodePercent === last.nodePercent)
            return;
        this.#publish({ ...this.#snapshot, progress: { ...last, percent, node, nodePercent } });
    }
    /** Show the picture ComfyUI sends while the running job forms. */
    #previewed(event) {
        const detail = Reflect.get(event, 'detail');
        const blob = isRecord(detail) ? detail.blob : null;
        if (!(blob instanceof Blob))
            return;
        const url = this.urls.createObjectURL(blob);
        this.#forgetPreview();
        this.#preview = url;
        this.#publish({
            ...this.#snapshot,
            progress: { ...this.#snapshot.progress, previewUrl: url },
        });
    }
    #forgetPreview() {
        if (this.#preview !== null)
            this.urls.revokeObjectURL(this.#preview);
        this.#preview = null;
    }
    #publish(snapshot) {
        this.#snapshot = snapshot;
        for (const listener of this.#listeners)
            listener(snapshot);
    }
}
function number(value) {
    return typeof value === 'number' && Number.isFinite(value) ? value : null;
}
/** Read the workflow tab name the queueing page recorded in a job's extra data. */
function workflowName(extra) {
    const saving = isRecord(extra) ? extra.sugarcubes_output_saving : null;
    const name = isRecord(saving) ? saving.workflow_name : null;
    return typeof name === 'string' && name.trim() ? name.trim() : null;
}
/** Address a job's preview through ComfyUI's own view route. */
function previewUrl(preview) {
    if (!preview || typeof preview.filename !== 'string')
        return null;
    const query = new URLSearchParams({ filename: preview.filename });
    if (typeof preview.type === 'string')
        query.set('type', preview.type);
    if (typeof preview.subfolder === 'string')
        query.set('subfolder', preview.subfolder);
    return `/api/view?${query.toString()}`;
}
/** Name the kind of media a job produced, as ComfyUI labels its preview. */
function mediaKind(preview) {
    if (!preview)
        return null;
    const kind = typeof preview.mediaType === 'string' ? preview.mediaType : '';
    if (kind === 'images' || kind === 'image')
        return 'image';
    if (kind === 'video')
        return 'video';
    if (kind === 'audio')
        return 'audio';
    const name = typeof preview.filename === 'string' ? preview.filename.toLowerCase() : '';
    if (/\.(png|jpe?g|webp|gif|bmp)$/.test(name))
        return 'image';
    if (/\.(mp4|webm|mov|mkv)$/.test(name))
        return 'video';
    if (/\.(wav|mp3|flac|ogg)$/.test(name))
        return 'audio';
    return 'other';
}
