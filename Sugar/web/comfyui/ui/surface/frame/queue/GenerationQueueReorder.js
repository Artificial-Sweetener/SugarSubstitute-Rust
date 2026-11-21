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
/** Let a person drag the generation queue's waiting jobs into the order they should run. */
import { ReorderGesture, } from '../../../interaction/reorder/ReorderGesture.js';
import { strideProjection } from '../../../interaction/reorder/ReorderSlots.js';
import { GENERATION_QUEUE_ROW_GAP, GENERATION_QUEUE_ROW_HEIGHT, } from './GenerationQueueViewStyles.js';
const ROW = '.sc-generation-queue-row';
/** How far beside the list the pointer may stray and still move or drop a job. */
const REACH = 24;
/**
 * Own dragging in one queue view.
 *
 * Only waiting jobs change place: the running job and finished jobs keep theirs. A waiting row
 * lifts off the list and follows the pointer while a dashed placeholder marks where it will land;
 * the gesture itself is the one every reorderable list in SugarCubes shares.
 */
export class GenerationQueueReorder {
    options;
    #gesture;
    #overlay = null;
    constructor(options) {
        this.options = options;
        const { root, rows } = options;
        const beside = (x) => {
            const bounds = rows.getBoundingClientRect();
            return x >= bounds.left - REACH && x <= bounds.right + REACH;
        };
        this.#gesture = new ReorderGesture({
            document: root.ownerDocument,
            threshold: 6,
            captureElement: () => rows,
            admission: (jobId) => options.busy()
                ? 'defer'
                : options.waiting().includes(jobId) || options.carries?.(jobId)
                    ? 'ready'
                    : 'refuse',
            keys: () => options.waiting(),
            project: (card, current) => 
            // A job carried only towards another place has no slot of its own to choose.
            current.includes(card.key)
                ? strideProjection({
                    current,
                    movable: () => true,
                    key: card.key,
                    center: card.y + GENERATION_QUEUE_ROW_HEIGHT / 2,
                    // Waiting jobs are the list's first rows, so the list's start is theirs.
                    listStart: rows.getBoundingClientRect().top - rows.scrollTop,
                    stride: GENERATION_QUEUE_ROW_HEIGHT + GENERATION_QUEUE_ROW_GAP,
                    size: GENERATION_QUEUE_ROW_HEIGHT,
                })
                : null,
            projects: (x) => beside(x),
            accepts: (x, y) => {
                const bounds = rows.getBoundingClientRect();
                return beside(x) && y >= bounds.top && y <= bounds.bottom;
            },
            scroll: { viewport: () => rows, edge: 32, step: 8, lateral: REACH },
            ...(options.targets ? { targets: options.targets } : {}),
            lifted: () => undefined,
            moved: (card) => this.#follow(card),
            preview: (order) => options.preview(order),
            commit: (_jobId, order) => options.commit(order),
            cancelled: () => undefined,
        });
        rows.addEventListener('pointerdown', this.#press);
        rows.addEventListener('pointerdown', this.#gesture.beginInteraction, true);
        rows.addEventListener('pointercancel', this.#gesture.cancel);
        rows.addEventListener('lostpointercapture', this.#gesture.cancel);
        rows.addEventListener('click', this.#gesture.click, true);
    }
    /** Lift a press that waited for an earlier drop to be applied. */
    resume() {
        this.#gesture.resume();
    }
    /** Abandon a drag whose job is no longer waiting. */
    reconcile() {
        const card = this.#gesture.card;
        if (!card)
            return;
        if (!this.options.waiting().includes(card.key) && !this.options.carries?.(card.key))
            this.#gesture.clear();
        else
            this.#mark(card.key);
    }
    /** Release the list and remove anything in the air. */
    dispose() {
        const { rows } = this.options;
        this.#gesture.clear();
        rows.removeEventListener('pointerdown', this.#press);
        rows.removeEventListener('pointerdown', this.#gesture.beginInteraction, true);
        rows.removeEventListener('pointercancel', this.#gesture.cancel);
        rows.removeEventListener('lostpointercapture', this.#gesture.cancel);
        rows.removeEventListener('click', this.#gesture.click, true);
    }
    #press = (event) => {
        const target = event.target;
        if (!(target instanceof Element) || target.closest('.sc-generation-queue-row-action'))
            return;
        const row = target.closest(ROW);
        const jobId = row?.dataset.jobId;
        if (row && jobId)
            this.#gesture.press(event, jobId, row);
    };
    /**
     * Draw the lifted row under the pointer and mark the place it left. A finished job carried
     * towards the tab bar is never lifted: it cannot be rearranged, so the list shows no card in
     * the air and no placeholder; the tab bar shows where it will open.
     */
    #follow(card) {
        const { root, rows } = this.options;
        if (card && !this.options.waiting().includes(card.key))
            return;
        if (!card) {
            this.#overlay?.remove();
            this.#overlay = null;
            for (const row of rows.querySelectorAll(`${ROW}[data-dragging]`))
                row.removeAttribute('data-dragging');
            rows.removeAttribute('data-reordering');
            return;
        }
        if (!this.#overlay) {
            const source = this.#row(card.key);
            if (!source)
                return;
            // A copy of the row is carried; the row itself stays in the list as the placeholder.
            const overlay = source.cloneNode(true);
            overlay.classList.add('sc-generation-queue-lifted');
            overlay.removeAttribute('data-dragging');
            overlay.setAttribute('aria-hidden', 'true');
            overlay.inert = true;
            overlay.style.width = `${String(card.width)}px`;
            overlay.style.transformOrigin = `${String(card.grabX)}px ${String(card.grabY)}px`;
            // The view's root is in the same layer as the list, above the page and below nothing.
            root.append(overlay);
            this.#overlay = overlay;
            rows.setAttribute('data-reordering', '');
        }
        this.#overlay.style.left = `${String(card.x)}px`;
        this.#overlay.style.top = `${String(card.y)}px`;
        this.#mark(card.key);
    }
    #mark(jobId) {
        this.#row(jobId)?.setAttribute('data-dragging', '');
    }
    #row(jobId) {
        for (const row of this.options.rows.querySelectorAll(ROW))
            if (row.dataset.jobId === jobId)
                return row;
        return null;
    }
}
