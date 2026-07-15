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
 * Own item-selection modal behavior in `web/comfyui/ui/dialogs/SelectionModal.js`.
 */
import { $el } from '/scripts/ui.js';
import { ModalShell } from './ModalShell.js';
/**
 * Coordinate list-selection modal behavior for SugarCubes.
 */
export class SelectionModal {
    shell;
    selectedValue;
    constructor({ adapter } = {}) {
        this.shell = new ModalShell({
            adapter: adapter ?? null,
            variantClassName: 'sugarcubes-selection-overlay',
            dialogClassName: 'sugarcubes-selection-dialog',
        });
        this.selectedValue = '';
    }
    open({ title = 'Select', message = [], items = [], confirmLabel = 'Select', cancelLabel = 'Cancel', emptyMessage = 'No options available.', } = {}) {
        this.selectedValue = '';
        const list = $el('div.sugarcubes-selection-dialog__list');
        const options = Array.isArray(items) ? items : [];
        if (!options.length) {
            list.appendChild($el('p.sugarcubes-selection-dialog__empty', { textContent: emptyMessage || '' }));
        }
        for (const item of options) {
            const option = $el('label.sugarcubes-selection-dialog__option');
            const radio = $el('input', {
                type: 'radio',
                name: 'sugarcubes-selection',
                value: typeof item?.value === 'string' ? item.value : '',
            });
            const labelWrap = $el('div.sugarcubes-selection-dialog__option-copy');
            const titleEl = $el('div.sugarcubes-selection-dialog__option-title', {
                textContent: item?.label || item?.value || '',
            });
            labelWrap.appendChild(titleEl);
            if (item?.description) {
                labelWrap.appendChild($el('div.sugarcubes-selection-dialog__option-description', {
                    textContent: item.description,
                }));
            }
            option.append(radio, labelWrap);
            radio.addEventListener('change', () => {
                this.selectedValue = radio.value;
                this.shell.setConfirmEnabled(Boolean(this.selectedValue));
                this.shell.setError('');
            });
            list.appendChild(option);
        }
        const handleConfirm = () => {
            if (!this.selectedValue) {
                this.shell.setError('Select an option to continue.');
                return;
            }
            this.shell.close(this.selectedValue);
        };
        const result = this.shell.open({
            title,
            description: message,
            body: list,
            confirmLabel,
            cancelLabel,
            confirmClassName: 'p-button-danger',
            cancelResult: null,
            onConfirm: handleConfirm,
            initialFocus: () => list.querySelector('input[type="radio"]'),
        });
        this.shell.setConfirmEnabled(false);
        return result;
    }
    close(result) {
        this.shell.close(result);
    }
}
