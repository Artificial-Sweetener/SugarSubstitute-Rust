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
/** Own Substitute's separate immediate-selection detector thumbnail modal. */
import { ModalShell } from '../dialogs/ModalShell.js';
import { mountDetectorWall } from './vue/ModelDetectorVueRuntime.js';
/** Coordinate packaged illustration selection within the established SugarCubes modal lifetime. */
export class ModelDetectorDialog {
    #shell;
    #document;
    #item;
    #detectors;
    #controller = new AbortController();
    #open = false;
    /** Bind the standard SugarCubes modal and its authoritative preference port. */
    constructor(document, item, detectors) {
        this.#document = document;
        this.#item = item;
        this.#detectors = detectors;
        this.#shell = new ModalShell({
            adapter: { getDocument: () => document, getWindow: () => document.defaultView },
            variantClassName: 'sc-model-detector-overlay',
            dialogClassName: 'sc-model-detector-modal',
        });
    }
    /** Load packaged art and save only a deliberate activated illustration without changing model selection. */
    async open() {
        if (this.#open)
            return;
        this.#open = true;
        const body = this.#document.createElement('div');
        body.className = 'sc-model-detector-body';
        body.textContent = 'Loading detector thumbnails…';
        const result = this.#shell.open({
            title: `Choose detector thumbnail for ${this.#item.title}`,
            body,
            cancelLabel: 'Cancel',
            cancelResult: null,
        });
        if (this.#shell.elements.confirmButton)
            this.#shell.elements.confirmButton.hidden = true;
        const overlay = this.#shell.elements.overlay;
        const view = { dispose: null };
        let saving = false;
        /** Keep selection transactional and retain the chooser if persistence fails. */
        const select = async (asset) => {
            if (saving || this.#controller.signal.aborted)
                return;
            saving = true;
            this.#shell.setBusy(true);
            this.#shell.setError('');
            try {
                await this.#detectors.assign(this.#item.value, asset, this.#controller.signal);
                this.close();
            }
            catch (error) {
                if (!this.#controller.signal.aborted) {
                    console.error('SugarCubes detector thumbnail save failed.', error);
                    this.#shell.setError(error instanceof Error ? error.message : 'The detector thumbnail could not be saved.');
                }
            }
            finally {
                saving = false;
                this.#shell.setBusy(false);
            }
        };
        void this.#detectors
            .library(this.#controller.signal)
            .then((items) => {
            if (this.#controller.signal.aborted)
                return;
            body.replaceChildren();
            view.dispose = mountDetectorWall(body, items, this.#item.detectorAsset ?? '', (asset) => {
                void select(asset);
            });
        })
            .catch((error) => {
            if (this.#controller.signal.aborted)
                return;
            console.error('SugarCubes detector thumbnail library failed.', error);
            body.textContent = '';
            this.#shell.setError(error instanceof Error ? error.message : 'Detector thumbnails are unavailable.');
        });
        try {
            await result;
        }
        finally {
            this.#controller.abort();
            view.dispose?.();
            overlay?.remove();
            this.#open = false;
        }
    }
    /** Abort scoped work when the user closes or the originating model field unmounts. */
    close() {
        this.#controller.abort();
        this.#shell.close(null);
    }
}
