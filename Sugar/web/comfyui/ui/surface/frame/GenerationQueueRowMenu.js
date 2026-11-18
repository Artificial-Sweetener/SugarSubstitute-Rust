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
/** Offer the generation queue's own row menu on the rows of Comfy's job list. */
import { ContextMenu } from '../../menus/ContextMenu.js';
import { isRecord } from '../../types/common.js';
const ROW = '[data-testid="job-assets-list"] .h-12 > div';
/** The row menu's one action, named as SugarSubstitute named it. */
export const USE_JOB_SEEDS = 'Use Seeds in Current Workflow';
/**
 * Add a right-click menu to a job's row, whether the job is waiting, running, finished or failed.
 *
 * Comfy's rows keep their own buttons and menu; a right-click opens the action the generation
 * queue has always offered there. Comfy marks each row with its job's identifier. A Comfy that
 * does not is read by the picture a finished job made, and then the action is offered when
 * exactly one job made a picture of that name.
 */
export class GenerationQueueRowMenu {
    options;
    #menu;
    #disposed = false;
    constructor(options) {
        this.options = options;
        this.#menu = new ContextMenu(options.document);
        options.document.addEventListener('contextmenu', this.#open, true);
    }
    /** Stop offering the menu. */
    dispose() {
        this.#disposed = true;
        this.options.document.removeEventListener('contextmenu', this.#open, true);
    }
    /** Open the menu for the row under the pointer. */
    #open = (event) => {
        if (this.#disposed || !(event.target instanceof Element))
            return;
        const row = event.target.closest(ROW);
        if (!row)
            return;
        event.preventDefault();
        event.stopPropagation();
        const jobId = this.#jobOf(row);
        this.#menu.open({
            label: 'Generation queue job',
            anchor: { kind: 'pointer', x: event.clientX, y: event.clientY },
            entries: [
                {
                    label: USE_JOB_SEEDS,
                    disabled: jobId === null,
                    select: () => {
                        if (jobId !== null)
                            void this.#useSeeds(jobId).catch(this.options.report);
                    },
                },
            ],
        });
    };
    /** Read the row's job identifier, or find the one job whose picture names the row. */
    #jobOf(row) {
        const marked = row.closest('[data-job-id]')?.dataset.jobId;
        if (marked)
            return marked;
        const title = row.querySelector('.flex-col > div')?.textContent.trim() ?? '';
        const tasks = this.#queue()?.tasks;
        if (!title || !Array.isArray(tasks))
            return null;
        const ids = new Set();
        for (const task of tasks) {
            if (!isRecord(task))
                continue;
            const preview = task.previewOutput;
            const id = task.jobId;
            if (isRecord(preview) && preview.filename === title && typeof id === 'string' && id)
                ids.add(id);
        }
        return ids.size === 1 ? ([...ids][0] ?? null) : null;
    }
    /** Read the job's record as text, have its seeds read exactly, and apply them. */
    async #useSeeds(jobId) {
        const api = isRecord(this.options.app) ? this.options.app.api : null;
        const fetchApi = isRecord(api) ? api.fetchApi : null;
        if (typeof fetchApi !== 'function')
            throw new Error('The Comfy API is unavailable.');
        const call = (path, init) => Reflect.apply(fetchApi, api, [path, init]);
        const history = await call(`/history/${encodeURIComponent(jobId)}`);
        if (!history.ok)
            throw new Error('The job record could not be read.');
        const historyText = await history.text();
        // Comfy answers with an empty record for a job that has not finished; its prompt is then
        // still in the queue.
        let described = { history_text: historyText };
        if (historyText.replace(/\s/g, '') === '{}') {
            const queue = await call('/queue');
            if (!queue.ok)
                throw new Error('The queue could not be read.');
            described = { queue_text: await queue.text(), job_id: jobId };
        }
        const reply = await call('/sugarcubes/v2/queue/job-seeds', {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify(described),
        });
        const body = reply.ok ? await reply.json() : null;
        const listed = isRecord(body) && Array.isArray(body.seeds) ? body.seeds : [];
        const seeds = listed.flatMap((seed) => isRecord(seed) &&
            typeof seed.alias === 'string' &&
            typeof seed.node_symbol === 'string' &&
            typeof seed.input_name === 'string' &&
            typeof seed.digits === 'string'
            ? [
                {
                    alias: seed.alias,
                    nodeSymbol: seed.node_symbol,
                    inputName: seed.input_name,
                    digits: seed.digits,
                },
            ]
            : []);
        const adopted = seeds.length ? this.options.adoptSeeds(seeds) : 0;
        if (adopted)
            this.options.notify('success', 'Seeds taken from the job', `${String(adopted)} seed ${adopted === 1 ? 'field' : 'fields'} in this workflow now hold the job's seeds.`);
        else
            this.options.notify('error', 'No seeds taken', 'This workflow has no Cube matching the ones that job ran.');
    }
    /** Narrow Comfy's job store on each use; it is created as Comfy starts. */
    #queue() {
        const root = this.options.document.querySelector('#vue-app');
        const vue = root ? Reflect.get(root, '__vue_app__') : null;
        const config = isRecord(vue) ? vue.config : null;
        const globals = isRecord(config) ? config.globalProperties : null;
        const pinia = isRecord(globals) ? globals.$pinia : null;
        const stores = isRecord(pinia) ? pinia._s : null;
        const queue = stores instanceof Map ? stores.get('queue') : null;
        return isRecord(queue) ? queue : null;
    }
}
