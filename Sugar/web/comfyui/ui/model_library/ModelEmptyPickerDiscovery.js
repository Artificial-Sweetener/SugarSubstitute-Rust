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
/** Own eligible empty-field discovery across native widget refresh and renderer lifetimes. */
import { comfyModelTarget } from './ComfyModelDiscoveryContext.js';
import { ModelDiscoveryDialog } from './ModelDiscoveryDialog.js';
import { queryModelCatalog } from './ModelCompatibility.js';
/** Retain the modal at host scope so native choice refresh can remount fields safely. */
export class ModelEmptyPickerDiscovery {
    #options;
    #dialog = null;
    #origin = null;
    /** Bind host readiness, explicit cube ownership, and the authoritative discovery services. */
    constructor(options) {
        this.#options = options;
    }
    /** Offer Find models only after a warm empty native field resolves to a supported context. */
    bind(root, binding, ready) {
        const controller = new AbortController();
        const snapshot = this.#options.snapshot();
        if (!snapshot || !this.#eligible(binding, snapshot))
            return () => controller.abort();
        const compatibility = this.#options.compatibility(binding, snapshot);
        const target = compatibility?.family ||
            compatibility?.architecture ||
            comfyModelTarget(binding, this.#options.cubes());
        void this.#options.discovery
            .context(binding.kind, target, controller.signal)
            .then((context) => {
            if (!context ||
                controller.signal.aborted ||
                !root.isConnected ||
                !this.#eligible(binding, this.#options.snapshot()))
                return;
            ready(() => this.#open(root, binding, target));
        })
            .catch((error) => {
            if (!controller.signal.aborted)
                this.#options.reportError(error);
        });
        return () => {
            controller.abort();
            ready(null);
        };
    }
    /** Close an originating node's modal after actual node removal, rather than a field-option refresh. */
    reconcile() {
        if (this.#origin && !this.#origin.isConnected)
            this.close();
    }
    /** Cancel the active interaction when enhanced pickers or Nodes 2.0 are deactivated. */
    close() {
        this.#dialog?.close();
        this.#dialog = null;
        this.#origin = null;
    }
    /** Start an exclusive modal with a receiver that re-resolves the refreshed native field. */
    #open(root, binding, target) {
        if (this.#dialog || binding.disabled() || !this.#eligible(binding, this.#options.snapshot()))
            return;
        this.#origin = root;
        const dialog = new ModelDiscoveryDialog(this.#options.document, binding.kind, target, this.#options.discovery, this.#options.credentials, (filename) => this.#options.sessions.get(root, binding).select(filename), this.#options.sessions.get(root, binding));
        this.#dialog = dialog;
        void dialog
            .open()
            .catch(this.#options.reportError)
            .finally(() => {
            if (this.#dialog === dialog) {
                this.#dialog = null;
                this.#origin = null;
            }
        });
    }
    /** Offer compatible discovery even when unrelated models exist in the host category. */
    #eligible(binding, snapshot) {
        if (!snapshot || snapshot.scanning)
            return false;
        const compatibility = this.#options.compatibility(binding, snapshot);
        if (!compatibility)
            return !binding.options.length;
        return !queryModelCatalog(snapshot.items, {
            kind: binding.kind,
            compatibleWith: compatibility,
        }).some((item) => binding.options.includes(item.value));
    }
}
