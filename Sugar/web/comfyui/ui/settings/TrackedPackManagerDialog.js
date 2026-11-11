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
 * Own the tracked pack manager dialog shell in `frontend/comfyui/ui/settings/TrackedPackManagerDialog.js`.
 */
import { ModalShell } from '../dialogs/ModalShell.js';
/**
 * Coordinate the tracked pack manager dialog lifecycle.
 */
export class TrackedPackManagerDialog {
    shell;
    isOpen;
    constructor({ adapter } = {}) {
        this.shell = new ModalShell({
            adapter: adapter ?? null,
            variantClassName: 'sugarcubes-pack-manager-overlay',
            dialogClassName: 'sugarcubes-pack-manager-dialog',
        });
        this.isOpen = false;
    }
    open({ title = 'Manage tracked packs', description = [], body = null, footerMeta = [], } = {}) {
        this.isOpen = true;
        const result = this.shell.open({
            title,
            description,
            body,
            footerMeta,
            confirmLabel: 'Done',
            cancelLabel: 'Close',
            confirmClassName: 'p-button-primary',
            cancelResult: false,
            onConfirm: () => this.close(true),
            initialFocus: () => this.shell.elements.cancelButton || this.shell.elements.confirmButton,
        });
        void result.finally(() => {
            this.isOpen = false;
        });
        return result;
    }
    update({ body = null, footerMeta = [] } = {}) {
        this.shell.setBody(body);
        this.shell.setFooterMeta(footerMeta);
    }
    setBusy(busy) {
        this.shell.setBusy(busy);
    }
    setError(message) {
        this.shell.setError(message);
    }
    close(result = false) {
        this.shell.close(result);
        this.isOpen = false;
    }
}
