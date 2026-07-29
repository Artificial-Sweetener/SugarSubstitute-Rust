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
/** Collect the persisted metadata required when saving a SugarCube. */

import { $el } from '/scripts/ui.js';
import {
  DEFAULT_TARGET_MODEL,
  TARGET_MODEL_OPTIONS,
  defaultSupportedModelsForTarget,
  normalizeSupportedModels,
  normalizeTargetModel,
} from '../core/ModelTargets.js';
import { normalizeDefaultAliasTitle } from '../core/CubeId.js';
import type {
  CubeAuthoringDialogOptions,
  CubeAuthoringValues,
} from '../create/CubeAuthoringDialog.js';
import { ModalShell } from './ModalShell.js';
import type { ModalAdapter } from './ModalShell.js';
import type { CubeSaveDestination } from '../create/CubeAuthoringIdentity.js';

const CUSTOM_TARGET_MODEL_VALUE = '__sugarcubes_custom_target_model__';
const MODEL_SUGGESTIONS_ID = 'sugarcubes-cube-authoring-model-suggestions';

/** Render the complete Cube-authoring form before persistence. */
export class CubeAuthoringModal {
  private readonly shell: ModalShell;

  constructor({ adapter }: { adapter?: ModalAdapter | null } = {}) {
    this.shell = new ModalShell({
      adapter: adapter ?? null,
      variantClassName: 'sugarcubes-create-cube-overlay',
      dialogClassName: 'sugarcubes-create-cube-dialog',
    });
  }

  open({
    candidate,
    destinationLocked = false,
    modelSuggestions = [],
    deriveIdentity,
  }: CubeAuthoringDialogOptions = {}): Promise<CubeAuthoringValues | null> {
    const resolveIdentity = typeof deriveIdentity === 'function' ? deriveIdentity : null;
    const initialName =
      normalizeDefaultAliasTitle(candidate?.defaultAlias?.split('/').pop()) || 'SugarCube';
    const initialTargetModel = normalizeTargetModel(candidate?.targetModel) || DEFAULT_TARGET_MODEL;
    let supportedModelsTouched = false;
    const form = $el('form.sugarcubes-modal__form.sugarcubes-create-cube__form');
    const nameInput = this.createInput(initialName, 'Name');
    const targetModelSelect = this.createTargetModelSelect(initialTargetModel);
    const customTargetModelInput = this.createInput('', 'Enter target model');
    if (!TARGET_MODEL_OPTIONS.includes(initialTargetModel)) {
      customTargetModelInput.value = initialTargetModel;
    }
    const supportedModelsInput = this.createInput(
      (candidate?.supportedModels?.length
        ? candidate.supportedModels
        : defaultSupportedModelsForTarget(initialTargetModel)
      ).join(', '),
      'SDXL, SD 1.5',
    );
    supportedModelsInput.setAttribute('list', MODEL_SUGGESTIONS_ID);
    const modelSuggestionList = this.createModelSuggestionList([
      ...modelSuggestions,
      ...(candidate?.supportedModels ?? []),
    ]);
    const destinationSelect = this.createDestinationSelect(candidate?.destination);
    destinationSelect.disabled = destinationLocked;
    const descriptionInput = $el(
      'textarea.p-inputtextarea.p-inputtext.p-component.sugarcubes-create-cube__description',
      {
        value: candidate?.description || '',
        placeholder: 'Describe what this cube does.',
      },
    ) as HTMLTextAreaElement;
    const identityValue = $el('code.sugarcubes-create-cube__value');
    const preview = $el('div.sugarcubes-create-cube__preview');
    preview.append(this.previewRow('Cube ID', identityValue));
    if (candidate?.nodeIds) {
      preview.append(
        this.previewRow(
          'Selection',
          $el('span.sugarcubes-create-cube__value', {
            textContent: `${candidate.nodeIds.length} selected nodes`,
          }),
        ),
      );
    }
    const customTargetField = this.field('Custom target model', customTargetModelInput);
    form.append(
      this.field('Name', nameInput),
      this.field('Target model', targetModelSelect),
      customTargetField,
      this.field(
        'Supported models',
        $el('div.sugarcubes-create-cube__model-support', [
          supportedModelsInput,
          modelSuggestionList,
        ]),
        'Comma-separated model families this cube can run with.',
      ),
      this.field(
        'Save to',
        destinationSelect,
        destinationLocked
          ? 'The existing Cube remains in its current library.'
          : 'Personal cubes stay local; packs are authored directly.',
      ),
      this.field('Description', descriptionInput, 'Leave blank to save an empty description.'),
      preview,
      this.warnings(candidate?.warnings),
    );

    const readTargetModel = (): string =>
      targetModelSelect.value === CUSTOM_TARGET_MODEL_VALUE
        ? customTargetModelInput.value
        : targetModelSelect.value;
    const destination = (): CubeSaveDestination =>
      destinationSelect.value === 'pack'
        ? { kind: 'pack', owner: '', repo: '', repoRef: '' }
        : { kind: 'local' };
    const update = (): void => {
      const name = normalizeDefaultAliasTitle(nameInput.value);
      const targetModel = this.tryNormalizeTargetModel(readTargetModel());
      customTargetField.hidden = targetModelSelect.value !== CUSTOM_TARGET_MODEL_VALUE;
      customTargetModelInput.disabled = customTargetField.hidden;
      if (!supportedModelsTouched) {
        supportedModelsInput.value = targetModel
          ? defaultSupportedModelsForTarget(targetModel).join(', ')
          : '';
      }
      try {
        identityValue.textContent =
          name && targetModel
            ? candidate?.cubeId ||
              (destination().kind === 'pack'
                ? 'Choose an author pack after review'
                : 'Personal Cube — saved locally')
            : targetModel
              ? 'Name required'
              : 'Target model required';
        this.shell.setConfirmEnabled(Boolean(name && targetModel && resolveIdentity));
        this.shell.setError('');
      } catch (error) {
        identityValue.textContent = 'Invalid authoring details';
        this.shell.setConfirmEnabled(false);
        this.shell.setError(
          error instanceof Error ? error.message : 'Authoring details are invalid.',
        );
      }
    };
    nameInput.addEventListener('input', update);
    targetModelSelect.addEventListener('change', update);
    destinationSelect.addEventListener('change', update);
    customTargetModelInput.addEventListener('input', update);
    supportedModelsInput.addEventListener('input', () => {
      supportedModelsTouched = true;
      this.shell.setError('');
    });
    descriptionInput.addEventListener('input', () => this.shell.setError(''));
    const result = this.shell.open({
      title: 'Save SugarCube',
      description: [
        candidate?.cubeId
          ? 'Review the Cube metadata before saving changes.'
          : 'Review the Cube metadata and destination before its first save.',
      ],
      body: form,
      confirmLabel: 'Save Cube',
      cancelLabel: 'Cancel',
      cancelResult: null,
      onConfirm: () => {
        const name = normalizeDefaultAliasTitle(nameInput.value);
        const targetModel = this.tryNormalizeTargetModel(readTargetModel());
        if (!name || !targetModel || !resolveIdentity) {
          this.shell.setError(!name ? 'Name is required.' : 'Target model is required.');
          return;
        }
        this.shell.setBusy(true);
        void resolveIdentity(name, targetModel, destination())
          .then((identity) => {
            this.shell.close({
              ...identity,
              targetModel,
              supportedModels: normalizeSupportedModels(supportedModelsInput.value, {
                targetModel,
              }),
              description: descriptionInput.value.trim(),
              destination: destination(),
            });
          })
          .catch((error: unknown) => {
            this.shell.setBusy(false);
            this.shell.setError(
              error instanceof Error ? error.message : 'Authoring details are invalid.',
            );
          });
      },
      initialFocus: () => nameInput,
    });
    update();
    return result as Promise<CubeAuthoringValues | null>;
  }

  private createInput(value: string, placeholder: string): HTMLInputElement {
    return $el('input.p-inputtext.p-component.sugarcubes-modal__text-input', {
      type: 'text',
      value,
      placeholder,
    }) as HTMLInputElement;
  }

  private createTargetModelSelect(initialTargetModel: string): HTMLSelectElement {
    const select = $el(
      'select.p-inputtext.p-component.sugarcubes-modal__text-input',
    ) as HTMLSelectElement;
    const isCustom = !TARGET_MODEL_OPTIONS.includes(initialTargetModel);
    select.replaceChildren(
      ...TARGET_MODEL_OPTIONS.map((model) => $el('option', { value: model, textContent: model })),
      $el('option', { value: CUSTOM_TARGET_MODEL_VALUE, textContent: 'A different model' }),
    );
    select.value = isCustom ? CUSTOM_TARGET_MODEL_VALUE : initialTargetModel;
    return select;
  }

  /** Build native browser suggestions from Comfy's loaded Cube model catalog. */
  private createModelSuggestionList(values: readonly string[]): HTMLDataListElement {
    const list = $el('datalist', { id: MODEL_SUGGESTIONS_ID }) as HTMLDataListElement;
    const unique = new Set(values.map((value) => value.trim()).filter(Boolean));
    list.replaceChildren(
      ...[...unique].map((value) => $el('option', { value, textContent: value })),
    );
    return list;
  }

  private createDestinationSelect(
    destination: CubeSaveDestination['kind'] | undefined,
  ): HTMLSelectElement {
    const select = $el(
      'select.p-inputtext.p-component.sugarcubes-modal__text-input',
    ) as HTMLSelectElement;
    select.replaceChildren(
      $el('option', { value: 'local', textContent: 'Personal (local)' }),
      $el('option', { value: 'pack', textContent: 'Author-owned Cube Pack' }),
    );
    select.value = destination === 'pack' ? 'pack' : 'local';
    return select;
  }

  private tryNormalizeTargetModel(value: unknown): string {
    try {
      return normalizeTargetModel(value);
    } catch (_error) {
      return '';
    }
  }

  private field(label: string, input: HTMLElement, helperText = ''): HTMLElement {
    return $el('label.sugarcubes-modal__field', [
      $el('span.sugarcubes-modal__field-label', { textContent: label }),
      input,
      $el('span.sugarcubes-modal__field-help', { textContent: helperText }),
    ]);
  }

  private previewRow(label: string, value: Node): HTMLElement {
    return $el('div.sugarcubes-create-cube__preview-row', [
      $el('span.sugarcubes-create-cube__label', { textContent: label }),
      value,
    ]);
  }

  private warnings(values: unknown): HTMLElement {
    const messages = (Array.isArray(values) ? values : []).filter(
      (value): value is string => typeof value === 'string' && Boolean(value.trim()),
    );
    const list = $el('ul.sugarcubes-create-cube__warnings');
    list.replaceChildren(...messages.map((message) => $el('li', { textContent: message })));
    list.hidden = !messages.length;
    return list;
  }
}
