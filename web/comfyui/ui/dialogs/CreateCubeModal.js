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
 * Own the marker-driven create-cube confirmation modal.
 */
import { $el } from '/scripts/ui.js';
import { ModalShell } from './ModalShell.js';
import { CreateModalGraphNavigator } from './CreateModalGraphNavigator.js';
import { normalizeDefaultAliasTitle } from '../core/CubeId.js';
import { DEFAULT_TARGET_MODEL, TARGET_MODEL_OPTIONS, defaultSupportedModelsForTarget, normalizeSupportedModels, normalizeTargetModel, } from '../core/ModelTargets.js';
const CUSTOM_TARGET_MODEL_VALUE = '__sugarcubes_custom_target_model__';
const CUSTOM_TARGET_MODEL_LABEL = 'A different model';
function normalizeString(value) {
    return typeof value === 'string' ? value.trim() : '';
}
function splitCubeId(cubeId) {
    const trimmed = normalizeString(cubeId);
    const parts = trimmed.split('/');
    return {
        cubeId: trimmed,
        filename: parts.at(-1) ?? '',
        source: parts.length > 1 ? parts.slice(0, -1).join('/') : '',
    };
}
function defaultDestination() {
    return {
        key: 'local/personal',
        sourceKind: 'local',
        namespace: 'personal',
        label: 'local',
        detail: 'personal',
        writable: true,
    };
}
/**
 * Render the create-cube modal and return the user-confirmed payload.
 */
export class CreateCubeModal {
    shell;
    navigator;
    constructor({ adapter = null } = {}) {
        this.shell = new ModalShell({
            adapter,
            variantClassName: 'sugarcubes-create-cube-overlay',
            dialogClassName: 'sugarcubes-create-cube-dialog',
        });
        this.navigator = new CreateModalGraphNavigator({ adapter });
    }
    open({ candidate, deriveCubeId, destinations = [], onCreateDestination = null, } = {}) {
        const initialName = normalizeDefaultAliasTitle(normalizeString(candidate?.defaultAlias).split('/').pop()) ||
            'SugarCube';
        const deriveId = deriveCubeId ?? (() => '');
        const initialTargetModel = normalizeTargetModel(candidate?.targetModel) || DEFAULT_TARGET_MODEL;
        let supportedModelsTouched = false;
        let destinationEntries = (Array.isArray(destinations) ? destinations : []).filter(Boolean);
        if (!destinationEntries.some((entry) => !entry?.action)) {
            destinationEntries = [defaultDestination(), ...destinationEntries];
        }
        let selectedDestination = destinationEntries.find((entry) => !entry?.action && entry?.key === candidate?.destinationKey) ||
            destinationEntries.find((entry) => !entry?.action) ||
            defaultDestination();
        const form = $el('form.sugarcubes-modal__form.sugarcubes-create-cube__form');
        const nameInput = $el('input.p-inputtext.p-component.sugarcubes-modal__text-input', {
            type: 'text',
            value: initialName,
        });
        const descriptionInput = $el('textarea.p-inputtextarea.p-inputtext.p-component.sugarcubes-create-cube__description', {
            value: normalizeString(candidate?.description),
            placeholder: 'Describe what this cube does.',
        });
        const targetModelSelect = $el('select.p-inputtext.p-component.sugarcubes-modal__text-input.sugarcubes-create-cube__target-model-select');
        const customTargetModelInput = $el('input.p-inputtext.p-component.sugarcubes-modal__text-input.sugarcubes-create-cube__custom-target-model', {
            type: 'text',
            placeholder: 'Enter target model',
        });
        const supportedModelsInput = $el('input.p-inputtext.p-component.sugarcubes-modal__text-input.sugarcubes-create-cube__supported-models', {
            type: 'text',
            value: (Array.isArray(candidate?.supportedModels)
                ? candidate.supportedModels
                : defaultSupportedModelsForTarget(initialTargetModel)).join(', '),
        });
        const defaultAliasValue = $el('span.sugarcubes-create-cube__value');
        const idValue = $el('code.sugarcubes-create-cube__value');
        const filenameValue = $el('span.sugarcubes-create-cube__value');
        const targetModelValue = $el('span.sugarcubes-create-cube__value');
        const sourceValue = $el('span.sugarcubes-create-cube__value');
        const countValue = $el('span.sugarcubes-create-cube__value', {
            textContent: `${candidate?.nodeIds?.length || 0} nodes, ${candidate?.markerIds?.length || 0} markers`,
        });
        const destinationSelect = $el('select.p-inputtext.p-component.sugarcubes-modal__text-input.sugarcubes-create-cube__destination-select');
        const nameField = this.buildTextField('Name', nameInput);
        const targetModelField = this.buildTextField('Target model', targetModelSelect);
        const customTargetModelField = this.buildTextField('Custom target model', customTargetModelInput, {
            helperText: 'Use one path-safe model family name.',
        });
        const supportedModelsField = this.buildTextField('Supported models', supportedModelsInput, {
            helperText: 'Comma-separated model families this cube can run with.',
        });
        const destinationField = this.buildTextField('Save to', destinationSelect);
        const descriptionField = this.buildTextField('Description', descriptionInput, {
            helperText: 'Leave blank to save an empty description.',
        });
        const preview = $el('div.sugarcubes-create-cube__preview', [
            this.buildPreviewRow('Default alias', defaultAliasValue),
            this.buildPreviewRow('Cube id', idValue),
            this.buildPreviewRow('Filename', filenameValue),
            this.buildPreviewRow('Target model', targetModelValue),
            this.buildPreviewRow('Destination', sourceValue),
            this.buildPreviewRow('Selection', countValue),
        ]);
        const warningList = this.buildWarningList(candidate?.warnings);
        form.append(nameField, targetModelField, customTargetModelField, supportedModelsField, destinationField, preview, descriptionField, warningList);
        const readTargetModelInput = () => targetModelSelect.value === CUSTOM_TARGET_MODEL_VALUE
            ? customTargetModelInput.value
            : targetModelSelect.value;
        const readTargetModelForPreview = () => {
            try {
                return normalizeTargetModel(readTargetModelInput());
            }
            catch (_error) {
                return '';
            }
        };
        const updateDerivedValues = () => {
            const next = normalizeDefaultAliasTitle(nameInput.value) || 'SugarCube';
            const targetModel = readTargetModelForPreview();
            if (!targetModel) {
                defaultAliasValue.textContent = next;
                idValue.textContent = 'Target model required';
                filenameValue.textContent = 'cube.cube';
                targetModelValue.textContent = 'Target model required';
                sourceValue.textContent = selectedDestination?.key || 'local/personal';
                return;
            }
            const defaultAlias = `${targetModel}/${next}`;
            const { cubeId, filename, source } = splitCubeId(deriveId(defaultAlias, selectedDestination, targetModel));
            defaultAliasValue.textContent = defaultAlias;
            idValue.textContent = cubeId;
            filenameValue.textContent = filename || 'cube.cube';
            targetModelValue.textContent = targetModel;
            sourceValue.textContent = source || 'local/personal';
        };
        const selectDestination = (destination) => {
            if (!destination || destination.action) {
                return;
            }
            selectedDestination = destination;
            this.shell.setError('');
            renderDestinationOptions();
            updateDerivedValues();
        };
        const formatDestinationOption = (destination) => {
            const label = normalizeString(destination?.label) || 'Cube pack';
            const detail = normalizeString(destination?.detail);
            return detail ? `${label} (${detail})` : label;
        };
        const renderDestinationOptions = () => {
            const options = destinationEntries.map((destination) => $el('option', {
                value: normalizeString(destination?.key),
                textContent: formatDestinationOption(destination),
            }));
            destinationSelect.replaceChildren(...options);
            destinationSelect.value = selectedDestination?.key || '';
        };
        const renderTargetModelOptions = () => {
            const usesCustomInitialModel = !TARGET_MODEL_OPTIONS.includes(initialTargetModel);
            const modelOptions = [...TARGET_MODEL_OPTIONS];
            const options = modelOptions.map((targetModel) => $el('option', {
                value: targetModel,
                textContent: targetModel,
            }));
            options.push($el('option', {
                value: CUSTOM_TARGET_MODEL_VALUE,
                textContent: CUSTOM_TARGET_MODEL_LABEL,
            }));
            targetModelSelect.replaceChildren(...options);
            targetModelSelect.value = usesCustomInitialModel
                ? CUSTOM_TARGET_MODEL_VALUE
                : initialTargetModel;
            customTargetModelInput.value = usesCustomInitialModel ? initialTargetModel : '';
        };
        const syncCustomTargetModelField = () => {
            const usesCustomTargetModel = targetModelSelect.value === CUSTOM_TARGET_MODEL_VALUE;
            customTargetModelField.style.display = usesCustomTargetModel ? '' : 'none';
            customTargetModelInput.disabled = !usesCustomTargetModel;
        };
        const handleDestinationChange = async () => {
            const destination = destinationEntries.find((entry) => entry?.key === destinationSelect.value);
            if (destination?.action === 'create-pack') {
                const previous = selectedDestination;
                destinationSelect.disabled = true;
                try {
                    const created = typeof onCreateDestination === 'function' ? await onCreateDestination() : null;
                    if (created) {
                        if (!destinationEntries.some((entry) => entry?.key === created.key)) {
                            const actionIndex = destinationEntries.findIndex((entry) => entry?.action === destination.action);
                            const insertIndex = actionIndex >= 0 ? actionIndex : destinationEntries.length;
                            destinationEntries.splice(insertIndex, 0, created);
                        }
                        selectDestination(created);
                    }
                    else {
                        selectedDestination = previous;
                        renderDestinationOptions();
                        updateDerivedValues();
                    }
                }
                catch (error) {
                    selectedDestination = previous;
                    this.shell.setError(error instanceof Error ? error.message : 'Failed to create cube pack.');
                    renderDestinationOptions();
                    updateDerivedValues();
                }
                finally {
                    destinationSelect.disabled = false;
                }
                return;
            }
            selectDestination(destination);
        };
        destinationSelect.addEventListener('change', () => {
            void handleDestinationChange();
        });
        renderDestinationOptions();
        renderTargetModelOptions();
        syncCustomTargetModelField();
        nameInput.addEventListener('input', () => {
            this.shell.setError('');
            updateDerivedValues();
        });
        targetModelSelect.addEventListener('change', () => {
            this.shell.setError('');
            syncCustomTargetModelField();
            const targetModel = readTargetModelForPreview();
            if (!supportedModelsTouched) {
                supportedModelsInput.value = targetModel
                    ? defaultSupportedModelsForTarget(targetModel).join(', ')
                    : '';
            }
            updateDerivedValues();
            if (targetModelSelect.value === CUSTOM_TARGET_MODEL_VALUE) {
                customTargetModelInput.focus();
            }
        });
        customTargetModelInput.addEventListener('input', () => {
            this.shell.setError('');
            const targetModel = readTargetModelForPreview();
            if (!supportedModelsTouched) {
                supportedModelsInput.value = targetModel
                    ? defaultSupportedModelsForTarget(targetModel).join(', ')
                    : '';
            }
            updateDerivedValues();
        });
        supportedModelsInput.addEventListener('input', () => {
            supportedModelsTouched = true;
            this.shell.setError('');
        });
        descriptionInput.addEventListener('input', () => this.shell.setError(''));
        updateDerivedValues();
        const handleConfirm = () => {
            const cubeName = normalizeDefaultAliasTitle(nameInput.value);
            if (!cubeName) {
                this.shell.setError('Name is required.');
                return;
            }
            if (!selectedDestination || selectedDestination.action) {
                this.shell.setError('Choose a cube pack before saving.');
                return;
            }
            let targetModel = '';
            try {
                targetModel = normalizeTargetModel(readTargetModelInput());
            }
            catch (error) {
                this.shell.setError(error instanceof Error ? error.message : 'Target model is invalid.');
                return;
            }
            if (!targetModel) {
                this.shell.setError('Target model is required.');
                return;
            }
            const defaultAlias = `${targetModel}/${cubeName}`;
            const cubeId = deriveId(defaultAlias, selectedDestination, targetModel);
            if (!normalizeString(cubeId)) {
                this.shell.setError('Cube id could not be derived from the target model and name.');
                return;
            }
            this.shell.close({
                defaultAlias,
                cubeName,
                cubeId,
                targetModel,
                supportedModels: normalizeSupportedModels(supportedModelsInput.value, {
                    targetModel,
                }),
                description: normalizeString(descriptionInput.value),
            });
        };
        const result = this.shell.open({
            title: 'Create SugarCube',
            description: ['Review the cube candidate before saving it.'],
            body: form,
            confirmLabel: 'Create Cube',
            cancelLabel: 'Cancel',
            confirmClassName: 'p-button-primary',
            cancelResult: null,
            allowOverlayClose: false,
            onConfirm: handleConfirm,
            initialFocus: () => nameInput,
        });
        this.navigator.attach(this.shell.elements.overlay, this.shell.elements.dialog);
        return result.finally(() => this.navigator.detach());
    }
    buildTextField(label, input, { helperText = '' } = {}) {
        const wrapper = $el('label.sugarcubes-modal__field');
        const labelEl = $el('span.sugarcubes-modal__field-label', { textContent: label });
        const helper = $el('div.sugarcubes-modal__field-help', { textContent: helperText });
        wrapper.append(labelEl, input, helper);
        return wrapper;
    }
    buildPreviewRow(label, valueEl) {
        const labelEl = $el('span.sugarcubes-create-cube__label', { textContent: label });
        return $el('div.sugarcubes-create-cube__row', [labelEl, valueEl]);
    }
    buildWarningList(warnings) {
        const entries = (Array.isArray(warnings) ? warnings : [])
            .map((value) => normalizeString(value))
            .filter(Boolean);
        if (!entries.length) {
            return $el('div.sugarcubes-create-cube__warnings', {
                textContent: 'No blocking warnings detected.',
            });
        }
        const items = entries.map((entry) => $el('li', { textContent: entry }));
        return $el('ul.sugarcubes-create-cube__warnings', items);
    }
}
