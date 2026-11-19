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
/** Show one generated asset, and one job's details, in SugarCubes dialogs. */
import { ConfirmDialog } from '../../../dialogs/ConfirmDialog.js';
import { ModalShell } from '../../../dialogs/ModalShell.js';
import { jobDetailLines } from './GenerationQueueJobDetails.js';
import { localClock } from './GenerationQueueRows.js';
const STYLE_ID = 'sc-generation-queue-asset-styles';
/**
 * The queue's two dialogs. The asset viewer shows a finished job's result at the size the window
 * allows; the details dialog lists what ComfyUI knows about a job.
 */
export class GenerationQueueDialogs {
    document;
    #viewer;
    #details;
    constructor(document) {
        this.document = document;
        const adapter = { getDocument: () => document, getWindow: () => document.defaultView };
        this.#viewer = new ModalShell({ adapter, dialogClassName: 'sc-generation-queue-asset-dialog' });
        this.#details = new ConfirmDialog({ adapter });
    }
    /** Show a finished job's image, video or sound. Returns false when it has none to show. */
    inspect(job) {
        if (job.status !== 'completed' || !job.previewUrl)
            return false;
        this.#styles();
        const media = this.#media(job, job.previewUrl);
        void this.#viewer.open({
            title: job.outputFile ?? 'Generated asset',
            body: media,
            confirmLabel: 'Close',
            showCancel: false,
            onConfirm: () => this.#viewer.close(null),
        });
        return true;
    }
    /** List what ComfyUI knows about a job. */
    details(job, jobs) {
        void this.#details.open({
            title: 'Job details',
            message: [...jobDetailLines(job, jobs, localClock())],
            confirmLabel: 'Close',
            showCancel: false,
            cancelResult: true,
        });
    }
    /** Close anything still open. */
    dispose() {
        this.#viewer.close(null);
    }
    #media(job, url) {
        const { document } = this;
        if (job.outputKind === 'video' || job.outputKind === 'audio') {
            const player = document.createElement(job.outputKind);
            player.className = 'sc-generation-queue-asset';
            player.controls = true;
            player.src = url;
            return player;
        }
        const image = document.createElement('img');
        image.className = 'sc-generation-queue-asset';
        image.alt = job.outputFile ?? 'Generated asset';
        image.src = url;
        return image;
    }
    #styles() {
        if (this.document.getElementById(STYLE_ID))
            return;
        const style = this.document.createElement('style');
        style.id = STYLE_ID;
        style.textContent = `
      .sugarcubes-modal.sc-generation-queue-asset-dialog { width:auto; max-width:calc(100vw - 48px); }
      .sc-generation-queue-asset { display:block; max-width:calc(100vw - 112px); max-height:calc(100vh - 220px);
        margin:0 auto; object-fit:contain; border-radius:4px; }
    `;
        this.document.head.append(style);
    }
}
