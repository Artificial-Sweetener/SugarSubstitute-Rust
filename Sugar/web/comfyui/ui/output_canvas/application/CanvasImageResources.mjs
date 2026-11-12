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
/** Own visible image slots, latest preview queues and atomic display replacement. */
import { CanvasImageSlot, canvasImageIntent, canvasResourceStatus, canvasMemoryAdmissionStatus, } from './CanvasImageSlot.mjs';
/** Keep browser resource policy bounded without storing payloads in workflow/session results. */
export class CanvasImageResources {
    options;
    #slots = new Map();
    #measured = new Map();
    #previewDimensions = new Map();
    #listeners = new Set();
    #wake = null;
    #active = true;
    #disposed = false;
    #drops = 0;
    /** Bind one attachment's admissions, elapsed time and presentation notification. */
    constructor(options) {
        this.options = options;
    }
    /** Observe admission completion with explicit surface-owned teardown. */
    subscribe(listener) {
        this.#listeners.add(listener);
        return () => {
            this.#listeners.delete(listener);
        };
    }
    /** Evaluate unique slot counts for visible and staged interests through one policy owner. */
    slotsFit(images, preview = null) {
        const named = new Set(images.map((image) => image.slot));
        const additional = preview && !named.has(preview.slot) ? preview.role : null;
        return (named.size === images.length &&
            images.filter((image) => image.input.role === 'main').length +
                Number(additional === 'main') <=
                2 &&
            images.filter((image) => image.input.role === 'thumbnail').length +
                Number(additional === 'thumbnail') <=
                8);
    }
    /** Replace the complete visible interest set, retaining at most one queued input per slot. */
    update(images, retry = false, preview = null) {
        if (!this.#active || this.#disposed)
            return;
        const named = new Set(images.map((image) => image.slot));
        const additional = preview && !named.has(preview.slot) ? preview.role : null;
        if (!this.slotsFit(images, preview))
            throw new RangeError('Canvas visible image slots exceed their bounds.');
        const retained = additional && preview ? this.#retainPreview(preview) : null;
        if (retained)
            named.add(retained);
        this.#reuse(images);
        for (const [name, slot] of this.#slots)
            if (!named.has(name)) {
                this.#drops += slot.release();
                this.#slots.delete(name);
            }
        for (const { slot: name, input } of images) {
            let slot = this.#slots.get(name);
            if (!slot) {
                slot = new CanvasImageSlot(input, this.options.budget);
                slot.queued = input;
                this.#slots.set(name, slot);
            }
            this.#drops += slot.update(input, retry);
        }
        this.#prune();
        this.#pump();
        this.#notify();
    }
    /** Move one owned preview atomically, refusing different runs/sources and invalid batch positions. */
    #retainPreview(interest) {
        if (interest.selection.batchIndex !== 0 ||
            (interest.role === 'thumbnail' && !this.options.budget.thumbnailsAllowed))
            return null;
        const current = this.#slots.get(interest.slot);
        if (current?.wanted.preview &&
            current.wanted.lane === interest.lane &&
            current.wanted.role === interest.role &&
            current.wanted.selection.sourceId === interest.selection.sourceId &&
            current.wanted.selection.batchIndex === 0)
            return interest.slot;
        const owner = [...this.#slots].find(([, slot]) => slot.displayed?.frame.preview &&
            slot.displayed.lane === interest.lane &&
            slot.displayed.frame.selection.sourceId === interest.selection.sourceId &&
            slot.displayed.frame.selection.batchIndex === 0);
        const displayed = owner?.[1].displayed;
        if (!owner ||
            !displayed ||
            !this.options.budget.reclassify(displayed.reservation, interest.role === 'thumbnail'))
            return null;
        owner[1].takeDisplay();
        this.#drops += owner[1].release();
        this.#slots.delete(owner[0]);
        if (current && current !== owner[1])
            this.#drops += current.release();
        const intent = canvasImageIntent({
            key: displayed.frame.key,
            runOrder: displayed.frame.runOrder,
            selection: interest.selection,
            lane: interest.lane,
            fallbackKey: null,
            preview: true,
            role: interest.role,
        });
        const target = new CanvasImageSlot(intent, this.options.budget);
        target.adopt(displayed);
        this.#slots.set(interest.slot, target);
        return interest.slot;
    }
    /** Relinquish one obsolete logical lane without clearing its valid comparison partner. */
    releaseSlot(name) {
        const slot = this.#slots.get(name);
        if (!slot)
            return;
        this.#drops += slot.release();
        this.#slots.delete(name);
        this.#prune();
        this.#pump();
        this.#notify();
    }
    /** Retire obsolete transient lanes before deferred layout can present a different run or source. */
    retirePreviewsExcept(lane) {
        let changed = false;
        for (const [name, slot] of this.#slots) {
            if (!slot.wanted.preview || slot.wanted.lane === lane)
                continue;
            this.#drops += slot.release();
            this.#slots.delete(name);
            changed = true;
        }
        if (!changed)
            return;
        this.#prune();
        this.#pump();
        this.#notify();
    }
    /** Resume the privately queued latest input after an explicit current-selection allowance changes. */
    admissionChanged() {
        if (!this.options.budget.thumbnailsAllowed)
            for (const slot of this.#slots.values())
                if (slot.wanted.role === 'thumbnail') {
                    this.#drops += slot.cancel();
                    slot.dropDisplay();
                    slot.status = canvasResourceStatus('budget', 'Overview images are suspended during large-image inspection.');
                }
        this.#pump();
        this.#notify();
    }
    /** Offer bytes only to an already visible slot without rebuilding unrelated final/overview interests. */
    offer(name, input) {
        const slot = this.#slots.get(name);
        if (!this.#active || this.#disposed || !slot || slot.wanted.role !== input.role)
            return false;
        this.#drops += slot.update(input, false);
        this.#prune();
        this.#pump();
        this.#notify();
        return true;
    }
    /** Move uniquely requested final handles before removing outgoing slots or starting replacement loads. */
    #reuse(images) {
        const moves = images
            .flatMap(({ slot: name, input }) => {
            if (input.preview || images.filter((value) => value.input.key === input.key).length !== 1)
                return [];
            const owner = [...this.#slots].find(([old, slot]) => old !== name &&
                slot.wanted.role === input.role &&
                slot.displayed?.frame.key === input.key &&
                !slot.displayed.frame.preview);
            return owner ? [{ name, input, owner: owner[1] }] : [];
        })
            .map((move) => ({ ...move, display: move.owner.takeDisplay() }));
        for (const move of moves) {
            if (!move.display)
                continue;
            let target = this.#slots.get(move.name);
            if (!target) {
                target = new CanvasImageSlot(move.input, this.options.budget);
                this.#slots.set(move.name, target);
            }
            this.#drops += target.adopt(move.display);
        }
    }
    /** Release every payload, frame and pending load when the attachment becomes inactive. */
    active(active) {
        if (this.#disposed || this.#active === active)
            return;
        this.#active = active;
        if (!active) {
            this.#wake?.();
            this.#wake = null;
            for (const slot of this.#slots.values())
                this.#drops += slot.release();
            this.#slots.clear();
            this.#measured.clear();
            this.#previewDimensions.clear();
        }
        this.#notify();
    }
    /** Read immutable display/status values without exposing release authority to presentation. */
    read() {
        return Object.freeze([...this.#slots].map(([name, slot]) => Object.freeze({ slot: name, frame: slot.displayed?.frame ?? null, status: slot.status })));
    }
    /** Read bounded lightweight geometry after a provisional decode requires explicit approval. */
    dimensions(key) {
        return this.#measured.get(key) ?? this.#previewDimensions.get(key) ?? null;
    }
    /** Identify admitted preview bytes, including pending decoding, without exposing payloads. */
    hasPreviewInterest(lane) {
        return [...this.#slots.values()].some((slot) => slot.wanted.preview && slot.wanted.lane === lane);
    }
    /** Count owned resources at lifecycle boundaries without running a telemetry loop. */
    diagnostics() {
        return Object.freeze({
            displayed: [...this.#slots.values()].filter((slot) => slot.displayed).length,
            pendingDecodes: this.options.decodes.pending,
            queuedInputs: [...this.#slots.values()].filter((slot) => slot.queued).length,
            previewDrops: this.#drops,
            estimatedBytes: this.options.budget.estimatedBytes,
            unknownLoads: this.options.budget.unknownLoads,
            activeDeadlines: this.options.decodes.pending + (this.#wake ? 1 : 0),
        });
    }
    /** Release this attachment immediately, with late decode delivery blocked by lease identity. */
    dispose() {
        this.active(false);
        this.#disposed = true;
        this.options.decodes.dispose();
        this.#measured.clear();
        this.#listeners.clear();
    }
    /** Prefer selected final images over previews/thumbnails and schedule only finite queued work. */
    #pump() {
        this.#wake?.();
        this.#wake = null;
        if (!this.#active || this.#disposed)
            return;
        let delay = Infinity;
        const slots = [...this.#slots].sort(([, a], [, b]) => priority(a.wanted) - priority(b.wanted));
        for (const [name, slot] of slots) {
            const input = slot.queued;
            if (!input || slot.pending || this.options.decodes.pending >= 2)
                continue;
            const remaining = input.preview ? slot.lastPreview + 1000 / 15 - this.options.clock.now() : 0;
            if (remaining > 0) {
                delay = Math.min(delay, remaining);
                continue;
            }
            const size = this.#measured.get(input.key) ??
                (input.preview ? this.#previewDimensions.get(input.lane) : null) ??
                input.dimensions;
            // An approved oversized outgoing image is released before staging its replacement.
            if (slot.displayed && this.options.budget.oversized)
                slot.dropDisplay();
            let reservation = this.options.budget.reserve(size, input.role === 'thumbnail');
            if (!reservation && size && slot.displayed) {
                slot.dropDisplay();
                reservation = this.options.budget.reserve(size, input.role === 'thumbnail');
            }
            if (!reservation && !size && this.options.budget.unknownLoads > 0) {
                slot.status = canvasResourceStatus('loading', 'Waiting for image dimensions…');
                continue;
            }
            if (!reservation) {
                slot.status = canvasMemoryAdmissionStatus(size, this.options.budget.estimatedBytes, input.role === 'main');
                continue;
            }
            slot.queued = null;
            slot.status = canvasResourceStatus('loading', input.preview ? 'Loading source preview…' : 'Loading image…');
            if (input.preview)
                slot.lastPreview = this.options.clock.now();
            const requested = canvasImageIntent(input);
            const admitted = reservation;
            try {
                const lease = this.options.decodes.start(input, {
                    ready: (image) => this.#ready(name, slot, requested, admitted, image),
                    failed: (error) => this.#failed(name, slot, requested, admitted, error),
                });
                if (!lease) {
                    this.options.budget.release(reservation);
                    slot.queued = input;
                    continue;
                }
                slot.pending = { intent: requested, reservation, lease };
            }
            catch (error) {
                this.#failed(name, slot, requested, reservation, error);
            }
        }
        if (Number.isFinite(delay))
            this.#wake = this.options.clock.later(delay, () => {
                this.#wake = null;
                this.#pump();
                this.#notify();
            });
    }
    /** Commit an attributable completed image atomically, then release its outgoing handle. */
    #ready(name, slot, requested, reservation, image) {
        slot.pending = null;
        const current = this.#slots.get(name) === slot &&
            this.#active &&
            (slot.wanted.key === requested.key ||
                (requested.preview && slot.wanted.preview && requested.lane === slot.wanted.lane));
        if (!current) {
            image.release();
            this.options.budget.release(reservation);
            if (requested.preview)
                this.#drops++;
            this.#pump();
            return;
        }
        const admitted = this.options.budget.measure(reservation, image.dimensions);
        const dimensions = Object.freeze({ ...image.dimensions });
        this.#measured.set(requested.key, dimensions);
        if (requested.preview)
            this.#previewDimensions.set(requested.lane, dimensions);
        if (!admitted) {
            image.release();
            this.options.budget.release(reservation);
            slot.status = canvasMemoryAdmissionStatus(dimensions, this.options.budget.estimatedBytes, requested.role === 'main');
        }
        else {
            slot.dropDisplay();
            slot.displayed = {
                lane: requested.lane,
                image,
                reservation,
                frame: Object.freeze({
                    key: requested.key,
                    runOrder: requested.runOrder,
                    previewLane: requested.preview ? requested.lane : null,
                    selection: requested.selection,
                    preview: requested.preview,
                    handle: image.handle,
                    dimensions: image.dimensions,
                }),
            };
            if (requested.preview)
                slot.lastPreview = this.options.clock.now();
            slot.status = canvasResourceStatus('ready', requested.preview ? 'Source preview' : '');
        }
        this.#prune();
        this.#pump();
        this.#notify();
    }
    /** Keep a valid comparison partner independent while reporting a recoverable failed lane. */
    #failed(name, slot, requested, reservation, error) {
        this.options.budget.release(reservation);
        slot.pending = null;
        if (this.#slots.get(name) !== slot || !this.#active)
            return;
        const displayed = slot.displayed;
        if (displayed &&
            displayed.frame.key !== requested.fallbackKey &&
            displayed.lane !== requested.lane)
            slot.dropDisplay();
        slot.status = canvasResourceStatus('error', 'The image could not be loaded. Retry to request it again.', true);
        this.options.logger.error('SugarCubes canvas image loading failed.', {
            slot: name,
            key: requested.key,
            error,
        });
        this.#prune();
        this.#pump();
        this.#notify();
    }
    /** Retain dimensions only for visible, incoming, queued or explicitly blocked current inputs. */
    #prune() {
        const keys = new Set([...this.#slots.values()].flatMap((slot) => [
            slot.wanted.key,
            ...(slot.pending ? [slot.pending.intent.key] : []),
            ...(slot.displayed ? [slot.displayed.frame.key] : []),
        ]));
        for (const key of this.#measured.keys())
            if (!keys.has(key))
                this.#measured.delete(key);
        const lanes = new Set([...this.#slots.values()].flatMap((slot) => [
            ...(slot.wanted.preview ? [slot.wanted.lane] : []),
            ...(slot.pending?.intent.preview ? [slot.pending.intent.lane] : []),
        ]));
        for (const lane of this.#previewDimensions.keys())
            if (!lanes.has(lane))
                this.#previewDimensions.delete(lane);
    }
    /** Contain presentation listener failures without corrupting admitted image ownership. */
    #notify() {
        try {
            this.options.changed();
        }
        catch (error) {
            this.options.logger.error('SugarCubes canvas resource presentation failed.', {
                slot: 'presentation',
                key: 'snapshot',
                error,
            });
        }
        for (const listener of this.#listeners) {
            try {
                listener();
            }
            catch (error) {
                this.options.logger.error('SugarCubes canvas resource subscriber failed.', {
                    slot: 'subscriber',
                    key: 'snapshot',
                    error,
                });
            }
        }
    }
}
/** Keep selected finals first when several visible lanes compete for decode admissions. */
function priority(input) {
    return input.role === 'thumbnail' ? 2 : input.preview ? 1 : 0;
}
