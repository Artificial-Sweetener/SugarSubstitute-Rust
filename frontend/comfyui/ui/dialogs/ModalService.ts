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
 * Own modal orchestration in `frontend/comfyui/ui/dialogs/ModalService.js`.
 */

import { ConfirmDialog } from './ConfirmDialog.js';
import { FormModal } from './FormModal.js';
import { InputModal } from './InputModal.js';
import { SelectionModal } from './SelectionModal.js';
import { CreatePersonalCubeModal } from './CreatePersonalCubeModal.js';
import { HistoricalVersionSaveModal } from './HistoricalVersionSaveModal.js';
import type { ConfirmDialogOptions } from './ConfirmDialog.js';
import type { FormModalOptions, FormValues } from './FormModal.js';
import type { InputModalOptions } from './InputModal.js';
import type { SelectionModalOptions } from './SelectionModal.js';
import type { CreatePersonalCubeModalOptions } from './CreatePersonalCubeModal.js';
import type { HistoricalVersionSaveOptions } from './HistoricalVersionSaveModal.js';
import type { ModalAdapter } from './ModalShell.js';
import type { PersonalCubeIdentity } from '../create/PersonalCubeIdentity.js';

/**
 * Coordinate reusable dialog entry points for SugarCubes.
 */
export class ModalService {
  readonly confirmDialog: ConfirmDialog;
  private readonly inputModal: InputModal;
  private readonly formModal: FormModal;
  private readonly selectionModal: SelectionModal;
  private readonly createPersonalCubeModal: CreatePersonalCubeModal;
  private readonly historicalVersionSaveModal: HistoricalVersionSaveModal;

  constructor({ adapter }: { adapter?: ModalAdapter | null } = {}) {
    const resolvedAdapter = adapter ?? null;
    this.confirmDialog = new ConfirmDialog({ adapter: resolvedAdapter });
    this.inputModal = new InputModal({ adapter: resolvedAdapter });
    this.formModal = new FormModal({ adapter: resolvedAdapter });
    this.selectionModal = new SelectionModal({ adapter: resolvedAdapter });
    this.createPersonalCubeModal = new CreatePersonalCubeModal({ adapter: resolvedAdapter });
    this.historicalVersionSaveModal = new HistoricalVersionSaveModal({ adapter: resolvedAdapter });
  }

  confirm(options: ConfirmDialogOptions = {}): Promise<boolean> {
    return this.confirmDialog.open(options);
  }

  alert(options: ConfirmDialogOptions = {}): Promise<boolean> {
    return this.confirmDialog.open({
      ...options,
      title: options.title || 'SugarCubes',
      confirmLabel: options.confirmLabel || 'OK',
      showCancel: false,
      confirmClassName: options.confirmClassName || 'sugarcubes-confirm__confirm',
      cancelResult: true,
    });
  }

  promptText(options: InputModalOptions = {}): Promise<string | null> {
    return this.inputModal.open(options);
  }

  openForm(options: FormModalOptions = {}): Promise<FormValues | null> {
    return this.formModal.open(options);
  }

  selectItem(options: SelectionModalOptions = {}): Promise<string | null> {
    return this.selectionModal.open(options);
  }

  openCreatePersonalCube(
    options: CreatePersonalCubeModalOptions = {},
  ): Promise<PersonalCubeIdentity | null> {
    return this.createPersonalCubeModal.open(options);
  }

  chooseHistoricalVersionSaveAction(options: HistoricalVersionSaveOptions = {}): Promise<unknown> {
    return this.historicalVersionSaveModal.open(options);
  }
}
