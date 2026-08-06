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
/** Create Cube host affordances only after Comfy has mounted its canvas. */
import { CubeAffordanceHostIntegration, } from './CubeAffordanceHostIntegration.js';
/** Own the late host lifecycle independently of Comfy's module evaluation order. */
export class CubeAffordanceHostLifecycle {
    #getDocument;
    #getCanvas;
    #createIntegration;
    #integration = null;
    /** Defer host-surface capture until Comfy invokes graph configuration. */
    constructor(options) {
        this.#getDocument = options.getDocument;
        this.#getCanvas = options.getCanvas;
        this.#createIntegration =
            options.createIntegration ??
                ((document, canvas) => new CubeAffordanceHostIntegration({
                    document,
                    canvas,
                    controller: options.controller,
                    logger: options.logger,
                }));
    }
    /** Bind one graph runtime after Comfy's canvas is available. */
    attach(runtime) {
        this.#requireIntegration().attach(runtime);
    }
    /** Return additive node-menu items only after the host integration is live. */
    getNodeMenuItems(node) {
        return this.#integration?.getNodeMenuItems(node) ?? [];
    }
    /** Adapt missing-node labels through the graph-bound integration. */
    adaptMissingNodes(missingNodes) {
        return this.#integration?.adaptMissingNodes(missingNodes) ?? 0;
    }
    /** Reconcile mounted Vue surfaces after host-driven state changes. */
    refresh() {
        this.#integration?.refresh();
    }
    /** Release the lazily created integration at extension teardown. */
    dispose() {
        this.#integration?.dispose();
        this.#integration = null;
    }
    /** Construct exactly one integration against Comfy's finalized host surfaces. */
    #requireIntegration() {
        if (this.#integration)
            return this.#integration;
        const document = this.#getDocument();
        const canvas = this.#getCanvas();
        if (!document || !canvas) {
            throw new Error('Comfy Cube affordance host surfaces are unavailable.');
        }
        this.#integration = this.#createIntegration(document, canvas);
        return this.#integration;
    }
}
