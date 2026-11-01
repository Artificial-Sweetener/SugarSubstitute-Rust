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
/** Retire inactive primary attachments while retaining per-workflow lightweight view state. */
import { CanvasPreparationTimeout } from '../domain/CanvasPreparation.js';
import { CanvasPreparationRecovery, sameCanvasPreparationTarget, } from './CanvasPreparationRecovery.js';
/** Give exactly one active visible primary workflow ownership of its surface and ephemeral previews. */
export class CanvasPrimarySurface {
    options;
    #current = null;
    #pending = null;
    #stop = null;
    #disposed = false;
    #retirement = null;
    /** Mount a fresh candidate even though the committed attachment matches the target. */
    #remount = false;
    #recovery;
    /** Inject attachment construction so lifecycle proofs exercise authority without borrowing platform decoders. */
    constructor(options) {
        this.options = options;
        this.#recovery = new CanvasPreparationRecovery({
            clock: options.clock,
            retry: (target) => {
                const current = this.#current;
                if (current && !sameCanvasPreparationTarget(current, target))
                    return;
                // A slow host may have finished meanwhile; a prepared attachment needs no retry.
                if (current && current.attachment.stalled?.() === false) {
                    this.#recovery.reset();
                    return;
                }
                this.#remount = true;
                this.reconcile();
            },
            exhausted: (target) => {
                const current = this.#current;
                if (current && sameCanvasPreparationTarget(current, target))
                    current.attachment.preparationFailed?.();
            },
        });
    }
    /** Report this primary attachment's bounded owned resources without exposing handles or workflow files. */
    diagnostics() {
        return {
            address: this.#current?.address ?? null,
            generation: this.#current?.generation ?? null,
            resources: this.#current?.attachment.diagnostics() ?? null,
        };
    }
    /**
     * Return the view's committed attachment as its renderer-neutral inspection destination, or
     * the staged candidate still preparing for that view when none is committed. A candidate
     * decides itself whether it can retain a reveal until it commits.
     */
    destination(view) {
        this.reconcile();
        const attachment = this.#current?.view === view
            ? this.#current.attachment
            : this.#pending?.view === view
                ? this.#pending.attachment
                : null;
        if (!attachment?.reveal)
            return null;
        const reveal = attachment.reveal.bind(attachment);
        return { reveal };
    }
    /** Observe scoped catalog boundaries and document visibility without another execution event listener. */
    setup() {
        if (this.#stop || this.#disposed)
            return;
        this.#stop = this.options.coordinator.subscribe((view) => {
            if (view === null || view === this.options.coordinator.active())
                this.reconcile();
        });
        this.options.document.addEventListener('visibilitychange', this.reconcile);
        this.reconcile();
    }
    /** Keep a same-view surface until its replacement is ready; never show a different workflow. */
    reconcile = () => {
        if (this.#disposed)
            return;
        const { coordinator, views } = this.options;
        const view = coordinator.active();
        const element = !this.options.document.hidden ? this.options.element() : null;
        const catalog = view && element ? coordinator.catalog(view) : null;
        const renderer = this.options.renderer();
        if (!view || !element || !catalog) {
            this.#cancelPending();
            this.#release();
            return;
        }
        if (!this.#remount &&
            this.#current?.view === view &&
            this.#current.generation === catalog.generation &&
            this.#current.element === element &&
            this.#current.renderer === renderer) {
            this.#cancelPending();
            this.#current.attachment.refresh();
            return;
        }
        if (this.#pending?.view === view &&
            this.#pending.generation === catalog.generation &&
            this.#pending.element === element &&
            this.#pending.renderer === renderer) {
            if (!this.#pending.mounting && !this.#retirement)
                this.#mount(this.#pending, catalog);
            else
                this.#pending.attachment?.refresh();
            this.#current?.attachment.refresh();
            return;
        }
        this.#cancelPending();
        if (this.#current &&
            (this.#current.view !== view ||
                this.#current.generation !== catalog.generation ||
                this.#current.element !== element))
            this.#release();
        const root = this.options.document.createElement('div');
        root.style.position = 'absolute';
        root.style.inset = '0';
        root.style.visibility = 'hidden';
        root.style.pointerEvents = 'none';
        let address;
        try {
            address = views.prepareAttachment(view, this.options.window);
            try {
                element.append(root);
            }
            catch (error) {
                views.cancelAttachment(address);
                throw error;
            }
        }
        catch (error) {
            root.remove();
            this.options.report(error);
            return;
        }
        this.#remount = false;
        const pending = {
            view,
            generation: catalog.generation,
            element,
            renderer,
            address,
            root,
            attachment: null,
            mounting: false,
        };
        this.#pending = pending;
        if (!this.#retirement)
            this.#mount(pending, catalog);
    };
    /** Construct only the candidate that still owns the staged registry lease. */
    #mount(pending, catalog) {
        if (this.#pending !== pending || this.#disposed || pending.mounting)
            return;
        pending.mounting = true;
        const { views, coordinator } = this.options;
        const { address, view, element, renderer, root } = pending;
        try {
            const accepts = () => !this.#disposed &&
                views.accepts(address) &&
                coordinator.active() === view &&
                coordinator.catalog(view)?.generation === catalog.generation &&
                !this.options.document.hidden &&
                this.options.element() === element;
            const acceptsPreparation = () => !this.#disposed &&
                this.#pending === pending &&
                views.acceptsPreparation(address) &&
                coordinator.active() === view &&
                coordinator.catalog(view)?.generation === catalog.generation &&
                !this.options.document.hidden &&
                this.options.element() === element &&
                this.options.renderer() === renderer;
            if (!acceptsPreparation()) {
                this.#cancelPending();
                return;
            }
            const attachment = this.options.mount(root, address, catalog, accepts, acceptsPreparation, () => this.#recover());
            if (attachment instanceof Promise)
                void attachment.then((loaded) => this.#adopt(pending, loaded), (error) => this.#failPending(pending, error));
            else
                this.#adopt(pending, attachment);
        }
        catch (error) {
            this.#failPending(pending, error);
        }
    }
    /** Refuse inactive preview bytes synchronously, before any object URL or decode is created. */
    wants(source) {
        return (this.#current?.attachment.wants(source.run.session.sessionId, source.run.session.generation, source.source.id) ?? false);
    }
    /** Deliver only the current attachment's qualified preview without retaining the Blob. */
    receive(preview, blob) {
        this.#current?.attachment.receive(preview, blob);
    }
    /** Retire subscriptions and pixels while leaving shared execution and other canvas views intact. */
    dispose() {
        if (this.#disposed)
            return;
        this.#disposed = true;
        this.#stop?.();
        this.#stop = null;
        this.options.document.removeEventListener('visibilitychange', this.reconcile);
        this.#recovery.reset();
        this.#cancelPending();
        this.#release();
    }
    /** Attach afresh at once, with a new retry budget, after a stalled attachment's request changed. */
    #recover() {
        if (this.#disposed)
            return;
        this.#recovery.reset();
        this.#remount = true;
        this.reconcile();
    }
    /** Adopt only the still-current constructed attachment, retiring late loads immediately. */
    #adopt(pending, attachment) {
        if (!attachment) {
            if (this.#pending === pending) {
                this.#cancelPending();
                this.reconcile();
            }
            return;
        }
        if (this.#pending !== pending) {
            void attachment.ready?.catch(() => { });
            try {
                this.#disposeAttachment(attachment);
            }
            catch (error) {
                this.options.report(error);
            }
            finally {
                pending.root.remove();
            }
            return;
        }
        pending.attachment = attachment;
        if (attachment.ready)
            void attachment.ready.then(() => this.#activate(pending), (error) => this.#failPending(pending, error));
        else
            this.#activate(pending);
    }
    /**
     * Commit only a still-current ready candidate, then retire the replaced surface. A candidate
     * committed after its preparation timed out keeps the retry budget running.
     */
    #activate(pending, recovering = false) {
        if (this.#pending !== pending)
            return;
        const { coordinator, views } = this.options;
        if (this.#disposed ||
            this.options.document.hidden ||
            coordinator.active() !== pending.view ||
            coordinator.catalog(pending.view)?.generation !== pending.generation ||
            this.options.element() !== pending.element ||
            this.options.renderer() !== pending.renderer ||
            !pending.attachment) {
            this.#cancelPending();
            this.reconcile();
            return;
        }
        try {
            views.commitAttachment(pending.address);
        }
        catch (error) {
            this.#failPending(pending, error);
            return;
        }
        const previous = this.#current;
        this.#pending = null;
        this.#current = { ...pending, attachment: pending.attachment };
        if (!recovering)
            this.#recovery.reset();
        pending.root.style.visibility = '';
        pending.root.style.pointerEvents = '';
        if (previous) {
            try {
                this.#retire(previous);
            }
            catch (error) {
                this.options.report(error);
            }
        }
        try {
            pending.attachment.refresh();
        }
        catch (error) {
            this.options.report(error);
        }
    }
    /**
     * Ignore failures from a canceled candidate and retain the previous committed attachment.
     *
     * A preparation timeout is recoverable: the host may only be slow. With nothing committed, the
     * timed-out candidate is committed so its loading state replaces a blank pane, and the same
     * target is retried afresh on a bounded backoff until it prepares or shows its error state.
     */
    #failPending(pending, error) {
        if (this.#pending !== pending)
            return;
        const timedOut = error instanceof CanvasPreparationTimeout;
        if (timedOut && !this.#current && pending.attachment)
            this.#activate(pending, true);
        else
            this.#cancelPending();
        this.options.report(error);
        if (timedOut)
            this.#recovery.failed(pending);
    }
    /** Cancel the exact staged transaction before disposing any candidate resources. */
    #cancelPending() {
        const pending = this.#pending;
        if (!pending)
            return;
        this.#pending = null;
        this.options.views.cancelAttachment(pending.address);
        try {
            if (pending.attachment)
                this.#disposeAttachment(pending.attachment);
        }
        catch (error) {
            this.options.report(error);
        }
        finally {
            pending.root.remove();
        }
    }
    /** Clear routing before surface disposal so late decoder or preview callbacks cannot regain ownership. */
    #release() {
        const previous = this.#current;
        this.#current = null;
        if (previous)
            this.#retire(previous);
    }
    /** Release only a surface's own DOM and lease, even after a replacement commits. */
    #retire(previous) {
        try {
            this.#disposeAttachment(previous.attachment);
        }
        finally {
            previous.root.remove();
            this.options.views.releaseAttachment(previous.address);
        }
    }
    /** Retain failed release as a fence: uncertain native custody cannot authorize another mount. */
    #disposeAttachment(attachment) {
        let release;
        try {
            release = attachment.dispose();
        }
        catch (error) {
            release = Promise.reject(error instanceof Error
                ? error
                : new Error('Canvas attachment release failed.', { cause: error }));
        }
        if (!release)
            return;
        const retirement = this.#retirement
            ? Promise.all([this.#retirement, release]).then(() => { })
            : release;
        this.#retirement = retirement;
        void retirement.then(() => {
            if (this.#retirement !== retirement)
                return;
            this.#retirement = null;
            try {
                if (this.#pending)
                    this.reconcile();
                else
                    this.#current?.attachment.refresh();
            }
            catch (error) {
                this.options.report(error);
            }
        }, (error) => {
            if (this.#pending)
                this.#failPending(this.#pending, error);
            else
                this.options.report(error);
        });
    }
}
