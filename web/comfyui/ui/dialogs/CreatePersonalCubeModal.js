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
 * Present the zero-setup personal cube creation confirmation.
 */

import { $el } from '/scripts/ui.js';
import { normalizeDefaultAliasTitle } from '../core/CubeId.js';
import { ModalShell } from './ModalShell.js';

/** Render the compact name-only personal cube modal. */
export class CreatePersonalCubeModal {
  constructor({ adapter } = {}) {
    this.shell = new ModalShell({
      adapter,
      variantClassName: 'sugarcubes-create-personal-overlay',
      dialogClassName: 'sugarcubes-create-personal-dialog',
    });
  }

  open({ candidate, deriveIdentity } = {}) {
    const resolveIdentity = typeof deriveIdentity === 'function' ? deriveIdentity : () => null;
    const initialName =
      normalizeDefaultAliasTitle(
        String(candidate?.defaultAlias || '')
          .split('/')
          .pop(),
      ) || 'SugarCube';
    const form = $el('form.sugarcubes-modal__form.sugarcubes-create-personal__form');
    const nameInput = $el('input.p-inputtext.p-component.sugarcubes-modal__text-input', {
      type: 'text',
      value: initialName,
    });
    const nameField = $el('label.sugarcubes-modal__field', [
      $el('span.sugarcubes-modal__field-label', { textContent: 'Name' }),
      nameInput,
      $el('span.sugarcubes-modal__field-help', {
        textContent: 'Saved privately in your personal SugarCubes library.',
      }),
    ]);
    const identityValue = $el('code.sugarcubes-create-cube__value');
    const selectionValue = $el('span.sugarcubes-create-cube__value', {
      textContent: `${candidate?.nodeIds?.length || 0} nodes, ${candidate?.markerIds?.length || 0} markers`,
    });
    const preview = $el('div.sugarcubes-create-cube__preview', [
      this.buildPreviewRow('Personal ID', identityValue),
      this.buildPreviewRow('Selection', selectionValue),
    ]);
    const warnings = this.buildWarnings(candidate?.warnings);
    form.append(nameField, preview, warnings);

    const update = () => {
      const name = normalizeDefaultAliasTitle(nameInput.value);
      try {
        const identity = name ? resolveIdentity(name) : null;
        identityValue.textContent = identity?.cubeId || 'Name required';
        this.shell.setConfirmEnabled(Boolean(name && identity?.cubeId));
        this.shell.setError('');
      } catch (error) {
        identityValue.textContent = 'Invalid name';
        this.shell.setConfirmEnabled(false);
        this.shell.setError(error?.message || 'Name is invalid.');
      }
    };
    nameInput.addEventListener('input', update);

    const result = this.shell.open({
      title: 'Create Personal SugarCube',
      description: [
        'Create this cube locally. Sharing details can be added when you move it to a pack.',
      ],
      body: form,
      confirmLabel: 'Create',
      cancelLabel: 'Cancel',
      cancelResult: null,
      onConfirm: () => {
        const name = normalizeDefaultAliasTitle(nameInput.value);
        const identity = name ? resolveIdentity(name) : null;
        if (!identity?.cubeId) {
          this.shell.setError('Name is required.');
          return;
        }
        this.shell.close(identity);
      },
      initialFocus: () => nameInput,
    });
    update();
    return result;
  }

  buildPreviewRow(label, valueNode) {
    return $el('div.sugarcubes-create-cube__preview-row', [
      $el('span.sugarcubes-create-cube__label', { textContent: label }),
      valueNode,
    ]);
  }

  buildWarnings(values) {
    const warnings = (Array.isArray(values) ? values : []).filter(
      (value) => typeof value === 'string' && value.trim(),
    );
    const list = $el('ul.sugarcubes-create-cube__warnings');
    list.replaceChildren(...warnings.map((warning) => $el('li', { textContent: warning })));
    list.hidden = !warnings.length;
    return list;
  }
}
