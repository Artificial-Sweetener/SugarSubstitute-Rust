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
import { DEFAULT_TARGET_MODEL, defaultSupportedModelsForTarget, normalizeSupportedModels, normalizeTargetModel, } from '../core/ModelTargets.js';
import { normalizeDefaultAliasTitle } from '../core/CubeId.js';
import { ComfySettingsAutocompleteControl, ComfySettingsSingleSelectControl, InstalledComfySettingsSelectRenderer, } from '../controls/ComfySettingsSelect.js';
import { CUSTOM_TARGET_MODEL_VALUE, isCustomTargetModel, supportedModelSuggestions, targetModelSelectOptions, } from '../controls/CubeModelSelection.js';
import { CubeDestinationControl } from '../controls/CubeDestinationControl.js';
import { ModalShell } from './ModalShell.js';
import { CubeAuthoringPreview } from './CubeAuthoringPreview.js';
import { CreateModalGraphNavigator } from './CreateModalGraphNavigator.js';
/** Render the complete Cube-authoring form before persistence. */
export class CubeAuthoringModal {
    shell;
    navigator;
    settingsSelectRenderer;
    constructor({ adapter, settingsSelectRenderer = null, } = {}) {
        this.shell = new ModalShell({
            adapter: adapter ?? null,
            variantClassName: 'sugarcubes-create-cube-overlay',
            dialogClassName: 'sugarcubes-create-cube-dialog',
        });
        this.navigator = new CreateModalGraphNavigator({ adapter: adapter ?? null });
        this.settingsSelectRenderer = settingsSelectRenderer;
    }
    open({ candidate, destinationLocked = false, destinations = [], modelSuggestions = [], onCreateDestination, deriveIdentity, } = {}) {
        const resolveIdentity = typeof deriveIdentity === 'function' ? deriveIdentity : null;
        const initialName = normalizeDefaultAliasTitle(candidate?.defaultAlias?.split('/').pop()) || 'SugarCube';
        const initialTargetModel = normalizeTargetModel(candidate?.targetModel) || DEFAULT_TARGET_MODEL;
        let supportedModelsTouched = false;
        const form = $el('form.sugarcubes-modal__form.sugarcubes-create-cube__form');
        const renderer = this.settingsSelectRenderer ?? new InstalledComfySettingsSelectRenderer(form.ownerDocument);
        const nameInput = this.createInput(initialName, 'Name');
        let updateTargetModel = () => undefined;
        const targetModelControl = new ComfySettingsSingleSelectControl(form.ownerDocument, renderer, {
            ariaLabel: 'Target model',
            options: targetModelSelectOptions(modelSuggestions),
            value: isCustomTargetModel(initialTargetModel, modelSuggestions)
                ? CUSTOM_TARGET_MODEL_VALUE
                : initialTargetModel,
            onChange: () => updateTargetModel(),
        });
        targetModelControl.element.classList.add('sugarcubes-create-cube__target-model-select');
        const customTargetModelInput = this.createInput('', 'Enter target model');
        if (isCustomTargetModel(initialTargetModel, modelSuggestions)) {
            customTargetModelInput.value = initialTargetModel;
        }
        const initialSupportedModels = candidate?.supportedModels?.length
            ? candidate.supportedModels
            : defaultSupportedModelsForTarget(initialTargetModel);
        const supportedModelsControl = new ComfySettingsAutocompleteControl(form.ownerDocument, renderer, {
            ariaLabel: 'Supported models',
            options: supportedModelSuggestions(modelSuggestions, initialSupportedModels),
            placeholder: 'Type a model family and press Enter',
            values: initialSupportedModels,
            onChange: () => {
                supportedModelsTouched = true;
                this.shell.setError('');
            },
        });
        supportedModelsControl.element.classList.add('sugarcubes-create-cube__model-support');
        const destinationControl = new CubeDestinationControl({
            destinations,
            documentRef: form.ownerDocument,
            locked: destinationLocked,
            onChange: () => update(),
            renderer,
            ...(candidate?.destination ? { candidateDestination: candidate.destination } : {}),
            ...(onCreateDestination ? { onCreateDestination } : {}),
            onError: (message) => this.shell.setError(message),
        });
        const descriptionInput = $el('textarea.p-inputtextarea.p-inputtext.p-component.sugarcubes-create-cube__description', {
            value: candidate?.description || '',
            placeholder: 'Describe what this cube does.',
        });
        const preview = new CubeAuthoringPreview(form.ownerDocument, candidate);
        const customTargetField = this.field('Custom target model', customTargetModelInput);
        customTargetField.classList.add('sugarcubes-create-cube__custom-target-model');
        form.append(this.field('Name', nameInput), this.field('Target model', targetModelControl.element), customTargetField, this.field('Supported models', supportedModelsControl.element, 'Choose suggestions or type a custom model family and press Enter.'), this.field('Save to', destinationControl.element, destinationLocked
            ? 'The existing Cube remains in its current library.'
            : 'Personal cubes stay local; packs are authored directly.'), preview.element, this.field('Description', descriptionInput, 'Leave blank to save an empty description.'), this.warnings(candidate?.warnings));
        const readTargetModel = () => targetModelControl.value() === CUSTOM_TARGET_MODEL_VALUE
            ? customTargetModelInput.value
            : targetModelControl.value();
        const update = () => {
            const name = normalizeDefaultAliasTitle(nameInput.value);
            const targetModel = this.tryNormalizeTargetModel(readTargetModel());
            customTargetField.hidden = targetModelControl.value() !== CUSTOM_TARGET_MODEL_VALUE;
            customTargetModelInput.disabled = customTargetField.hidden;
            try {
                const destination = destinationControl.destination();
                const identity = name && targetModel && destination && resolveIdentity
                    ? resolveIdentity(name, targetModel, destination)
                    : null;
                preview.update({
                    destination: destinationControl.presentation(),
                    identity,
                    name,
                    targetModel,
                });
                this.shell.setConfirmEnabled(Boolean(identity?.cubeId));
                this.shell.setError('');
            }
            catch (error) {
                preview.update({
                    destination: destinationControl.presentation(),
                    identity: null,
                    name,
                    targetModel,
                });
                this.shell.setConfirmEnabled(false);
                this.shell.setError(error instanceof Error ? error.message : 'Authoring details are invalid.');
            }
        };
        nameInput.addEventListener('input', update);
        updateTargetModel = () => {
            const targetModel = this.tryNormalizeTargetModel(readTargetModel());
            if (!supportedModelsTouched) {
                const values = targetModel ? defaultSupportedModelsForTarget(targetModel) : [];
                supportedModelsControl.update({
                    options: supportedModelSuggestions(modelSuggestions, [
                        ...(candidate?.supportedModels ?? []),
                        ...values,
                    ]),
                    values,
                });
            }
            update();
        };
        customTargetModelInput.addEventListener('input', updateTargetModel);
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
            confirmClassName: 'p-button-primary',
            cancelResult: null,
            allowOverlayClose: false,
            onConfirm: () => {
                const name = normalizeDefaultAliasTitle(nameInput.value);
                const targetModel = this.tryNormalizeTargetModel(readTargetModel());
                const destination = destinationControl.destination();
                if (!name || !targetModel || !destination || !resolveIdentity) {
                    this.shell.setError(!name
                        ? 'Name is required.'
                        : !targetModel
                            ? 'Target model is required.'
                            : 'Choose a save destination.');
                    return;
                }
                try {
                    const identity = resolveIdentity(name, targetModel, destination);
                    this.shell.close({
                        ...identity,
                        targetModel,
                        supportedModels: normalizeSupportedModels(supportedModelsControl.values(), {
                            targetModel,
                        }),
                        description: descriptionInput.value.trim(),
                        destination,
                    });
                }
                catch (error) {
                    this.shell.setError(error instanceof Error ? error.message : 'Authoring details are invalid.');
                }
            },
            initialFocus: () => nameInput,
        });
        update();
        this.navigator.attach(this.shell.elements.overlay, this.shell.elements.dialog);
        return result.finally(() => {
            this.navigator.detach();
            supportedModelsControl.dispose();
            targetModelControl.dispose();
            destinationControl.dispose();
        });
    }
    createInput(value, placeholder) {
        return $el('input.p-inputtext.p-component.sugarcubes-modal__text-input', {
            type: 'text',
            value,
            placeholder,
        });
    }
    tryNormalizeTargetModel(value) {
        try {
            return normalizeTargetModel(value);
        }
        catch (_error) {
            return '';
        }
    }
    field(label, input, helperText = '') {
        return $el('label.sugarcubes-modal__field', [
            $el('span.sugarcubes-modal__field-label', { textContent: label }),
            input,
            $el('span.sugarcubes-modal__field-help', { textContent: helperText }),
        ]);
    }
    warnings(values) {
        const messages = (Array.isArray(values) ? values : []).filter((value) => typeof value === 'string' && Boolean(value.trim()));
        if (!messages.length) {
            return $el('div.sugarcubes-create-cube__warnings', {
                textContent: 'No blocking warnings detected.',
            });
        }
        return $el('ul.sugarcubes-create-cube__warnings', messages.map((message) => $el('li', { textContent: message })));
    }
}
