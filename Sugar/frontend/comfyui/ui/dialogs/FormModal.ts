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
 * Own multi-field form modal behavior in `frontend/comfyui/ui/dialogs/FormModal.js`.
 */

import { $el } from '/scripts/ui.js';
import { ModalShell } from './ModalShell.js';
import type { ModalAdapter } from './ModalShell.js';

export type FormFieldValue = string | boolean;
export type FormValues = Record<string, FormFieldValue>;

export interface FormField {
  key: string;
  type?: 'text' | 'url' | 'checkbox';
  label?: string;
  initialValue?: string | boolean;
  placeholder?: string;
  helperText?: string;
  required?: boolean;
  normalizeValue?: (value: string) => string;
  validate?: (value: FormFieldValue, values: FormValues) => string | null | undefined;
}

export interface FormModalOptions {
  title?: string;
  message?: string | string[];
  fields?: FormField[];
  confirmLabel?: string;
  cancelLabel?: string;
  confirmClassName?: string;
}

interface FormFieldElements {
  field: FormField;
  input: HTMLInputElement;
  errorEl: HTMLElement;
}

function normalizeFieldValue(field: FormField, rawValue: FormFieldValue): FormFieldValue {
  if (field?.type === 'checkbox') {
    return Boolean(rawValue);
  }
  const value = typeof rawValue === 'string' ? rawValue : '';
  return typeof field?.normalizeValue === 'function' ? field.normalizeValue(value) : value;
}

/**
 * Coordinate form modal behavior for SugarCubes.
 */
export class FormModal {
  private readonly shell: ModalShell;
  private readonly fieldElements: Map<string, FormFieldElements>;

  constructor({ adapter }: { adapter?: ModalAdapter | null } = {}) {
    this.shell = new ModalShell({
      adapter: adapter ?? null,
      variantClassName: 'sugarcubes-form-overlay',
      dialogClassName: 'sugarcubes-form-dialog',
    });
    this.fieldElements = new Map();
  }

  open({
    title = 'Form',
    message = [],
    fields = [],
    confirmLabel = 'Save',
    cancelLabel = 'Cancel',
    confirmClassName = 'p-button-primary',
  }: FormModalOptions = {}): Promise<FormValues | null> {
    this.fieldElements.clear();
    const form = $el('form.sugarcubes-modal__form');

    for (const field of Array.isArray(fields) ? fields : []) {
      const key = typeof field?.key === 'string' ? field.key.trim() : '';
      if (!key) {
        continue;
      }
      const wrapperClass =
        field?.type === 'checkbox'
          ? 'sugarcubes-modal__field sugarcubes-modal__field--checkbox'
          : 'sugarcubes-modal__field';
      const wrapper = $el('label', { className: wrapperClass });
      const errorEl = $el('div.sugarcubes-modal__field-error');
      let input: HTMLInputElement;
      if (field?.type === 'checkbox') {
        input = $el('input', {
          type: 'checkbox',
          checked: Boolean(field.initialValue),
        }) as HTMLInputElement;
        const text = $el('span.sugarcubes-modal__checkbox-label', {
          textContent: field.label || key,
        });
        wrapper.append(input, text);
      } else {
        const labelEl = $el('span.sugarcubes-modal__field-label', {
          textContent: field.label || key,
        });
        input = $el('input.p-inputtext.p-component.sugarcubes-modal__text-input', {
          type: field?.type === 'url' ? 'url' : 'text',
          value: typeof field.initialValue === 'string' ? field.initialValue : '',
          placeholder: field.placeholder || '',
        }) as HTMLInputElement;
        wrapper.append(labelEl, input);
      }
      const helperEl = $el('div.sugarcubes-modal__field-help', {
        textContent: field.helperText || '',
      });
      wrapper.append(helperEl, errorEl);
      form.appendChild(wrapper);
      this.fieldElements.set(key, { field, input, errorEl });

      const clearError = () => {
        errorEl.textContent = '';
        if (this.shell.elements.errorEl?.textContent) {
          this.shell.setError('');
        }
      };
      input.addEventListener(field?.type === 'checkbox' ? 'change' : 'input', clearError);
    }

    const findFirstField = () => {
      for (const { input } of this.fieldElements.values()) {
        return input;
      }
      return null;
    };

    const handleConfirm = () => {
      const values: FormValues = {};
      let firstError = '';
      for (const { field, input, errorEl } of this.fieldElements.values()) {
        errorEl.textContent = '';
        const rawValue =
          field?.type === 'checkbox' ? Boolean(input?.checked) : String(input?.value || '');
        const normalized = normalizeFieldValue(field, rawValue);
        values[field.key] = normalized;
        if (field?.required) {
          const hasValue =
            field.type === 'checkbox'
              ? Boolean(normalized)
              : typeof normalized === 'string'
                ? Boolean(normalized.trim())
                : Boolean(normalized);
          if (!hasValue) {
            const message = `${field.label || field.key} is required.`;
            errorEl.textContent = message;
            firstError = firstError || message;
            continue;
          }
        }
        const validationError =
          typeof field?.validate === 'function' ? field.validate(normalized, values) : '';
        if (validationError) {
          errorEl.textContent = validationError;
          firstError = firstError || validationError;
        }
      }
      if (firstError) {
        this.shell.setError(firstError);
        return;
      }
      this.shell.close(values);
    };

    return this.shell.open({
      title,
      description: message,
      body: form,
      confirmLabel,
      cancelLabel,
      confirmClassName,
      cancelResult: null,
      onConfirm: handleConfirm,
      initialFocus: findFirstField,
    }) as Promise<FormValues | null>;
  }

  close(result: unknown): void {
    this.shell.close(result);
  }
}
