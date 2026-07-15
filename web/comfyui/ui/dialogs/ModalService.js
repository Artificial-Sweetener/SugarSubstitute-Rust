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
 * Own modal orchestration in `web/comfyui/ui/dialogs/ModalService.js`.
 */
import { ConfirmDialog } from './ConfirmDialog.js';
import { FormModal } from './FormModal.js';
import { InputModal } from './InputModal.js';
import { SelectionModal } from './SelectionModal.js';
import { CreatePersonalCubeModal } from './CreatePersonalCubeModal.js';
import { HistoricalVersionSaveModal } from './HistoricalVersionSaveModal.js';
/**
 * Coordinate reusable dialog entry points for SugarCubes.
 */
export class ModalService {
    confirmDialog;
    inputModal;
    formModal;
    selectionModal;
    createPersonalCubeModal;
    historicalVersionSaveModal;
    constructor({ adapter } = {}) {
        const resolvedAdapter = adapter ?? null;
        this.confirmDialog = new ConfirmDialog({ adapter: resolvedAdapter });
        this.inputModal = new InputModal({ adapter: resolvedAdapter });
        this.formModal = new FormModal({ adapter: resolvedAdapter });
        this.selectionModal = new SelectionModal({ adapter: resolvedAdapter });
        this.createPersonalCubeModal = new CreatePersonalCubeModal({ adapter: resolvedAdapter });
        this.historicalVersionSaveModal = new HistoricalVersionSaveModal({ adapter: resolvedAdapter });
    }
    confirm(options = {}) {
        return this.confirmDialog.open(options);
    }
    alert(options = {}) {
        return this.confirmDialog.open({
            ...options,
            title: options.title || 'SugarCubes',
            confirmLabel: options.confirmLabel || 'OK',
            showCancel: false,
            confirmClassName: options.confirmClassName || 'sugarcubes-confirm__confirm',
            cancelResult: true,
        });
    }
    promptText(options = {}) {
        return this.inputModal.open(options);
    }
    openForm(options = {}) {
        return this.formModal.open(options);
    }
    selectItem(options = {}) {
        return this.selectionModal.open(options);
    }
    openCreatePersonalCube(options = {}) {
        return this.createPersonalCubeModal.open(options);
    }
    chooseHistoricalVersionSaveAction(options = {}) {
        return this.historicalVersionSaveModal.open(options);
    }
}
