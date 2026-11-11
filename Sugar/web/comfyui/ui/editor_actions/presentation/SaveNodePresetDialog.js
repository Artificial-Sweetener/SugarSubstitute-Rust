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
/** Collect a preset name and save scope through the shared SugarCubes modal shell. */
import { ModalShell } from '../../dialogs/ModalShell.js';
/** Keep naming and scope selection together, as in Substitute's save-preset dialog. */
export class SaveNodePresetDialog {
    #document;
    #shell;
    /** Bind modal lifetime to the same document as the editor surface. */
    constructor(document) {
        this.#document = document;
        this.#shell = new ModalShell({
            adapter: { getDocument: () => document, getWindow: () => document.defaultView },
            dialogClassName: 'sc-save-node-preset',
        });
    }
    /** Return validated form state only after Save; cancellation leaves the library untouched. */
    async open(title, scopes) {
        const form = this.#document.createElement('form');
        form.className = 'sugarcubes-modal__form';
        const name = this.#document.createElement('input');
        name.className = 'p-inputtext p-component sugarcubes-modal__text-input';
        name.type = 'text';
        const select = this.#document.createElement('select');
        select.className = 'p-inputtext p-component sugarcubes-modal__text-input';
        for (const [index, scope] of scopes.entries()) {
            const option = this.#document.createElement('option');
            option.value = String(index);
            option.textContent = scope.scope === 'global' ? 'Global' : `Base model: ${scope.label}`;
            select.append(option);
        }
        for (const [caption, field] of [
            ['Name', name],
            ['Save under', select],
        ]) {
            const label = this.#document.createElement('label');
            label.className = 'sugarcubes-modal__field';
            const text = this.#document.createElement('span');
            text.className = 'sugarcubes-modal__field-label';
            text.textContent = caption;
            label.append(text, field);
            form.append(label);
        }
        const update = () => this.#shell.setConfirmEnabled(Boolean(name.value.trim()) && scopes[Number(select.value)] !== undefined);
        name.addEventListener('input', update);
        select.addEventListener('change', update);
        form.addEventListener('submit', (event) => {
            event.preventDefault();
            this.#shell.elements.confirmButton?.click();
        });
        const result = this.#shell.open({
            title,
            body: form,
            confirmLabel: 'Save',
            cancelLabel: 'Cancel',
            cancelResult: null,
            initialFocus: () => name,
            onConfirm: () => {
                const association = scopes[Number(select.value)], value = name.value.trim();
                if (association && value)
                    this.#shell.close({ name: value, association });
            },
        });
        update();
        return (await result);
    }
    /** Cancel pending form state during editor shutdown. */
    close() {
        this.#shell.close(null);
    }
}
