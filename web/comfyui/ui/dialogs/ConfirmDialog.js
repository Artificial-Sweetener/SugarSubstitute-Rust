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
/**
 * Own the SugarCubes dialog presentation layer in `web/comfyui/ui/dialogs/ConfirmDialog.js`.
 */
import { $el } from '/scripts/ui.js';
import { ModalShell } from './ModalShell.js';
/**
 * Coordinate confirm dialog behavior for the SugarCubes UI.
 */
export class ConfirmDialog {
    shell;
    elements;
    constructor({ adapter } = {}) {
        this.shell = new ModalShell({
            adapter: adapter ?? null,
            variantClassName: 'sugarcubes-confirm-overlay',
            dialogClassName: 'sugarcubes-confirm-dialog',
        });
        this.elements = this.shell.elements;
    }
    open({ title, message, confirmLabel, cancelLabel = 'Cancel', showCancel = true, confirmClassName = 'p-button-danger sugarcubes-confirm__confirm', cancelResult = false, } = {}) {
        const messageEl = $el('div.sugarcubes-confirm__message');
        this.renderMessage(messageEl, message);
        const promise = this.shell.open({
            title: title || 'Confirm',
            body: messageEl,
            confirmLabel: confirmLabel || 'Confirm',
            cancelLabel,
            confirmClassName,
            cancelResult,
            showCancel,
            onConfirm: () => this.shell.close(true),
            initialFocus: () => this.shell.elements.confirmButton,
        });
        this.elements = this.shell.elements;
        return promise;
    }
    close(result) {
        this.shell.close(Boolean(result));
    }
    renderMessage(messageEl, message) {
        const lines = (Array.isArray(message) ? message : [message])
            .map((value) => (typeof value === 'string' ? value : ''))
            .filter((value) => value.length);
        const nodes = lines.map((value) => $el('p', { textContent: value }));
        messageEl.replaceChildren(...nodes);
    }
}
