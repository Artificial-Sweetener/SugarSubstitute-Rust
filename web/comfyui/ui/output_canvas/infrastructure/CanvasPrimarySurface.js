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
/** Retire inactive primary browser attachments while retaining per-workflow lightweight view state. */
/** Give exactly one active visible primary workflow ownership of browser pixels and ephemeral preview bytes. */
export class CanvasPrimarySurface {
    options;
    #current = null;
    #stop = null;
    #disposed = false;
    /** Inject attachment construction so lifecycle proofs exercise authority without borrowing platform decoders. */
    constructor(options) {
        this.options = options;
    }
    /** Report this primary attachment's bounded owned resources without exposing handles or workflow files. */
    diagnostics() {
        return {
            address: this.#current?.address ?? null,
            generation: this.#current?.generation ?? null,
            resources: this.#current?.attachment.diagnostics() ?? null,
        };
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
    /** Replace resources before a tab/generation/renderer/allocated-container transition can overlap ownership. */
    reconcile = () => {
        if (this.#disposed)
            return;
        const { coordinator, views } = this.options;
        const view = coordinator.active();
        const element = !this.options.document.hidden ? this.options.element() : null;
        const catalog = view && element ? coordinator.catalog(view) : null;
        const renderer = this.options.renderer();
        if (!view || !element || !catalog) {
            this.#release();
            return;
        }
        if (this.#current?.view === view &&
            this.#current.generation === catalog.generation &&
            this.#current.element === element &&
            this.#current.renderer === renderer) {
            this.#current.attachment.refresh();
            return;
        }
        this.#release();
        const address = views.prepareAttachment(view, this.options.window);
        views.commitAttachment(address);
        try {
            const accepts = () => !this.#disposed &&
                views.accepts(address) &&
                coordinator.active() === view &&
                !this.options.document.hidden &&
                this.options.element() === element &&
                this.options.renderer() === renderer;
            const attachment = this.options.mount(element, address, catalog, accepts);
            this.#current = {
                view,
                element,
                address,
                attachment,
                renderer,
                generation: catalog.generation,
            };
        }
        catch (error) {
            views.releaseAttachment(address);
            this.options.report(error);
        }
    };
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
        this.#release();
    }
    /** Clear routing before surface disposal so late decoder or preview callbacks cannot regain ownership. */
    #release() {
        const previous = this.#current;
        this.#current = null;
        previous?.attachment.dispose();
        if (previous)
            this.options.views.releaseAttachment(previous.address);
    }
}
