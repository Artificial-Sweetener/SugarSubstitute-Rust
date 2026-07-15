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
 * Own single-field input modal behavior in `web/comfyui/ui/dialogs/InputModal.js`.
 */

import { $el } from '/scripts/ui.js';
import { ModalShell } from './ModalShell.js';
import type { ModalAdapter } from './ModalShell.js';

export interface InputModalOptions {
  title?: string;
  message?: string | string[];
  label?: string;
  helperText?: string;
  placeholder?: string;
  initialValue?: string;
  confirmLabel?: string;
  cancelLabel?: string;
  confirmClassName?: string;
  normalizeValue?: (value: string) => string;
  validate?: ((value: string) => string | null | undefined) | null;
  allowEmpty?: boolean;
}

interface InputModalElements {
  input?: HTMLInputElement;
  helperEl?: HTMLElement;
}

/**
 * Coordinate single-field modal input behavior for SugarCubes.
 */
export class InputModal {
  private readonly shell: ModalShell;
  elements: InputModalElements;

  constructor({ adapter }: { adapter?: ModalAdapter | null } = {}) {
    this.shell = new ModalShell({
      adapter: adapter ?? null,
      variantClassName: 'sugarcubes-input-overlay',
      dialogClassName: 'sugarcubes-input-dialog',
    });
    this.elements = {};
  }

  open({
    title = 'Input',
    message = [],
    label = 'Value',
    helperText = '',
    placeholder = '',
    initialValue = '',
    confirmLabel = 'Save',
    cancelLabel = 'Cancel',
    confirmClassName = 'p-button-primary',
    normalizeValue = (value) => value,
    validate = null,
    allowEmpty = false,
  }: InputModalOptions = {}): Promise<string | null> {
    const field = $el('label.sugarcubes-modal__field');
    const labelEl = $el('span.sugarcubes-modal__field-label', { textContent: label });
    const input = $el('input.p-inputtext.p-component.sugarcubes-modal__text-input', {
      type: 'text',
      value: typeof initialValue === 'string' ? initialValue : '',
      placeholder: placeholder || '',
    }) as HTMLInputElement;
    const helperEl = $el('div.sugarcubes-modal__field-help', { textContent: helperText || '' });
    field.append(labelEl, input, helperEl);

    const updateConfirmState = () => {
      const normalized = normalizeValue(String(input.value || ''));
      const hasValue =
        typeof normalized === 'string' ? Boolean(normalized.trim()) : Boolean(normalized);
      this.shell.setConfirmEnabled(allowEmpty || hasValue);
      if (this.shell.elements.errorEl?.textContent) {
        this.shell.setError('');
      }
    };
    input.addEventListener('input', updateConfirmState);

    this.elements = { input, helperEl };

    const handleConfirm = () => {
      const normalized = normalizeValue(String(input.value || ''));
      const hasValue =
        typeof normalized === 'string' ? Boolean(normalized.trim()) : Boolean(normalized);
      if (!allowEmpty && !hasValue) {
        this.shell.setError(`${label} is required.`);
        return;
      }
      const validationError = typeof validate === 'function' ? validate(normalized) : '';
      if (validationError) {
        this.shell.setError(validationError);
        return;
      }
      this.shell.close(normalized);
    };

    const result = this.shell.open({
      title,
      description: message,
      body: field,
      confirmLabel,
      cancelLabel,
      confirmClassName,
      cancelResult: null,
      onConfirm: handleConfirm,
      initialFocus: () => input,
    });
    updateConfirmState();
    return result as Promise<string | null>;
  }

  close(result: unknown): void {
    this.shell.close(result);
  }
}
