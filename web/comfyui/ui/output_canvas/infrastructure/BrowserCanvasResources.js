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
/** Translate visible workflow descriptors into bounded browser image admissions. */
import { canvasPreviewLane, } from '../application/CanvasDisplayPlan.js';
import { CANVAS_IMAGE_BYTE_LIMIT } from '../application/CanvasImageBudget.js';
/** Keep ephemeral byte delivery separate from catalog updates and final-file loading. */
export class BrowserCanvasResources {
    resources;
    budget;
    allowances;
    url;
    #plan = null;
    #budgetScope = '';
    #previewStarted = false;
    #overviewImages = [];
    #overviewPreviewSlot = null;
    #disposed = false;
    #active = true;
    #refreshNeeded = false;
    /** Use existing authoritative resource/approval owners and a host-qualified final URL adapter. */
    constructor(resources, budget, allowances, url) {
        this.resources = resources;
        this.budget = budget;
        this.allowances = allowances;
        this.url = url;
    }
    /** Replace resource interests only at a logical display change, preserving same-run pending previews. */
    reconcile(plan, retry = false) {
        if (this.#disposed)
            return;
        if (this.#plan &&
            (this.#plan.view.viewId !== plan.view.viewId ||
                this.#plan.view.sessionId !== plan.view.sessionId))
            throw new TypeError('Browser image resources belong to another view.');
        const replaced = this.#plan !== null && this.#plan.generation !== plan.generation;
        const changed = this.#refreshNeeded || this.#plan?.signature !== plan.signature;
        this.#plan = plan;
        if (!this.#active)
            return;
        this.#refreshNeeded = false;
        if (replaced)
            this.resources.update([]);
        this.#configureBudget();
        if (!changed && !retry)
            return;
        this.#previewStarted = false;
        if (plan.view.mode === 'overview')
            return;
        this.#overviewImages = [];
        this.#overviewPreviewSlot = null;
        const images = [];
        if (plan.view.mode === 'image') {
            for (const [name, slot] of [
                ['primary', plan.primary],
                ['secondary', plan.secondary],
            ]) {
                const input = slot ? this.#final(slot) : null;
                if (input)
                    images.push({ slot: name, input });
            }
        }
        // Deleted sources relinquish their displayed lane before ordinary replacement can retain it.
        for (const snapshot of this.resources.read())
            if (snapshot.frame && !plan.availableSources.includes(snapshot.frame.selection.sourceId))
                this.resources.releaseSlot(snapshot.slot);
        this.resources.update(images, retry);
    }
    /** Admit only the overview's intersecting final-image cells through the same byte/decode limits. */
    overview(cells) {
        const plan = this.#plan;
        if (!plan || plan.view.mode !== 'overview' || !this.#active || this.#disposed)
            return;
        const images = [];
        this.#overviewPreviewSlot = null;
        for (const cell of cells) {
            if (cell.preview)
                this.#overviewPreviewSlot = cell.key;
            const input = this.#final({
                selection: cell.selection,
                item: cell.item,
                sourcePresent: plan.availableSources.includes(cell.selection.sourceId),
            });
            if (input)
                images.push({ slot: cell.key, input: { ...input, role: 'thumbnail' } });
        }
        this.#overviewImages = images;
        this.resources.update(images);
    }
    /** Refuse stale/foreign bytes before handing their ownership to a visible resource slot. */
    receive(preview, blob) {
        const plan = this.#plan;
        const current = plan?.preview;
        const slot = plan?.primary;
        if (this.#disposed ||
            !this.#active ||
            !plan ||
            !current ||
            !slot ||
            preview.token !== current.token ||
            canvasPreviewLane(preview) !== canvasPreviewLane(current))
            return;
        const lane = canvasPreviewLane(preview);
        const input = {
            key: preview.token,
            runOrder: preview.runOrder,
            selection: slot.selection,
            lane,
            fallbackKey: slot.item?.id ?? null,
            dimensions: this.resources.dimensions(lane) ??
                preview.dimensions ??
                this.allowances.dimensions(plan.view.viewId, plan.generation, lane),
            role: plan.view.mode === 'overview' ? 'thumbnail' : 'main',
            preview: true,
            payload: { kind: 'preview', blob },
        };
        this.#previewStarted = true;
        const target = plan.view.mode === 'overview' ? this.#overviewPreviewSlot : 'primary';
        if (!target)
            return;
        if (!this.resources.offer(target, input))
            this.resources.update([
                ...this.#overviewImages.filter((image) => image.slot !== target),
                { slot: target, input },
            ]);
    }
    /** Approve exactly the current logical image/pair and resume privately queued work. */
    admit() {
        const plan = this.#plan;
        if (!plan || this.#disposed)
            return;
        const images = this.#approvalImages();
        if (!images.length)
            return;
        this.allowances.grant(plan.view.viewId, plan.generation, images);
        this.#configureBudget();
        this.resources.admissionChanged();
        if (!plan.preview)
            this.reconcile(plan, true);
    }
    /** Remember measured approved geometry without resetting another unknown partner's admission epoch. */
    measured() {
        const plan = this.#plan;
        if (!plan)
            return;
        for (const snapshot of this.resources.read()) {
            const frame = snapshot.frame;
            if (!frame)
                continue;
            const key = frame.preview && plan.preview ? canvasPreviewLane(plan.preview) : frame.key;
            this.allowances.measure(plan.view.viewId, plan.generation, key, frame.dimensions);
        }
    }
    /** Release hidden allocation pixels and rebuild current final interests on visible reactivation. */
    active(active) {
        if (this.#disposed || this.#active === active)
            return;
        this.#active = active;
        this.#refreshNeeded = true;
        this.resources.active(active);
        if (active && this.#plan)
            this.reconcile(this.#plan);
    }
    /** Report whether a current preview has received bytes, independently of final fallback visibility. */
    get previewStarted() {
        return this.#previewStarted;
    }
    /** Release this attachment's images and bytes while keeping view-scoped lightweight approval. */
    dispose() {
        if (this.#disposed)
            return;
        this.#disposed = true;
        this.#plan = null;
        this.resources.dispose();
    }
    /** Build a full-fidelity request from an exact selected final. */
    #final(slot) {
        const plan = this.#plan;
        const item = slot.item;
        if (!plan || !slot.sourcePresent || !item || item.mediaKind !== 'image')
            return null;
        return {
            key: item.id,
            runOrder: item.provenance.runOrder,
            selection: slot.selection,
            lane: item.id,
            fallbackKey: null,
            dimensions: this.resources.dimensions(item.id) ??
                item.dimensions ??
                this.allowances.dimensions(plan.view.viewId, plan.generation, item.id),
            role: 'main',
            preview: false,
            payload: { kind: 'file', url: this.url(item) },
        };
    }
    /** Preserve multiplicity for an explicitly selected duplicate A/B image. */
    #approvalImages() {
        const plan = this.#plan;
        if (!plan || plan.view.mode !== 'image')
            return [];
        const images = [];
        if (plan.preview) {
            const key = canvasPreviewLane(plan.preview);
            images.push({ key, dimensions: this.resources.dimensions(key) ?? plan.preview.dimensions });
        }
        else if (plan.primary?.item?.mediaKind === 'image') {
            const item = plan.primary.item;
            images.push({
                key: item.id,
                dimensions: this.resources.dimensions(item.id) ?? item.dimensions,
            });
        }
        if (plan.secondary?.item?.mediaKind === 'image') {
            const item = plan.secondary.item;
            images.push({
                key: item.id,
                dimensions: this.resources.dimensions(item.id) ?? item.dimensions,
            });
        }
        return images;
    }
    /** Configure one budget epoch per logical set/approval, never per successfully measured frame. */
    #configureBudget() {
        const plan = this.#plan;
        if (!plan)
            return;
        const keys = this.#approvalImages().map((image) => image.key);
        const allowance = this.allowances.read(plan.view.viewId, plan.generation, keys);
        const scope = JSON.stringify([plan.generation, keys.sort(), allowance?.approvalId ?? null]);
        if (scope === this.#budgetScope)
            return;
        this.#budgetScope = scope;
        this.budget.allowance(allowance?.bytes ?? CANVAS_IMAGE_BYTE_LIMIT, allowance?.unknownLoads ?? 0);
    }
}
