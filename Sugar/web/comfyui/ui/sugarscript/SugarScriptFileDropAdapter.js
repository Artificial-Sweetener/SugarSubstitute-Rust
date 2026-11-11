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
/** Accept `.sugar` files through Comfy's canvas drop surface without an editor UI. */
const MAX_SOURCE_BYTES = 1_000_000;
/** Own capture-phase interception only for a single bounded `.sugar` file. */
export class SugarScriptFileDropAdapter {
    #document;
    #importSource;
    #feedback;
    #readErrorMessage;
    #onDragOver = (event) => this.#handleDragOver(event);
    #onDrop = (event) => this.#handleDrop(event);
    #active = false;
    /** Bind document, import use case, and host feedback. */
    constructor(options) {
        this.#document = options.document;
        this.#importSource = options.importSource;
        this.#feedback = options.feedback ?? null;
        this.#readErrorMessage = options.readErrorMessage;
    }
    /** Start idempotent file interception without changing other Comfy drops. */
    setup() {
        if (this.#active)
            return;
        this.#active = true;
        this.#document.addEventListener('dragover', this.#onDragOver, true);
        this.#document.addEventListener('drop', this.#onDrop, true);
    }
    /** Release document listeners when the host lifecycle ends. */
    dispose() {
        if (!this.#active)
            return;
        this.#active = false;
        this.#document.removeEventListener('dragover', this.#onDragOver, true);
        this.#document.removeEventListener('drop', this.#onDrop, true);
    }
    /** Advertise copy behavior only when the payload is one `.sugar` file. */
    #handleDragOver(event) {
        const file = readSugarFile(event.dataTransfer);
        if (!file)
            return;
        event.preventDefault();
        if (event.dataTransfer)
            event.dataTransfer.dropEffect = 'copy';
    }
    /** Read and import one bounded source file while suppressing Comfy's unknown-file path. */
    #handleDrop(event) {
        const file = readSugarFile(event.dataTransfer);
        if (!file)
            return;
        event.preventDefault();
        event.stopImmediatePropagation();
        void this.#importFile(file);
    }
    /** Contain file read and compilation failures at the user-visible host boundary. */
    async #importFile(file) {
        try {
            if (file.size > MAX_SOURCE_BYTES)
                throw new Error('SugarScript source exceeds 1 MB.');
            const source = await file.text();
            await this.#importSource(source);
            this.#feedback?.push?.('success', 'SugarScript workflow imported', file.name);
        }
        catch (error) {
            this.#feedback?.push?.('error', 'SugarScript import failed', this.#readErrorMessage(error));
        }
    }
}
/** Return exactly one `.sugar` file and leave every other drop untouched. */
function readSugarFile(transfer) {
    const files = transfer ? [...transfer.files] : [];
    if (files.length !== 1)
        return null;
    const file = files[0];
    return file?.name.toLocaleLowerCase().endsWith('.sugar') ? file : null;
}
