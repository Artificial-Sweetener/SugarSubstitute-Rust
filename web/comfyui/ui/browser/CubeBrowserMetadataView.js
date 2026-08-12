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
/** Own read-only and editable Cube metadata presentation. */
import { $el } from '/scripts/ui.js';
import { deriveCubeIdFromDefaultAlias, normalizeDefaultAliasTitle } from '../core/CubeId.js';
import { TARGET_MODEL_OPTIONS, deriveTargetModelCubeId, deriveTargetModelFromCubeId, normalizeTargetModel, } from '../core/ModelTargets.js';
import { ModelAutocompleteControl } from '../controls/ModelAutocompleteControl.js';
/** Render metadata and own all stateful editor controls. */
export class CubeBrowserMetadataView {
    options;
    editInputs = null;
    identityInvalid = false;
    constructor(options) {
        this.options = options;
    }
    /** Return the active editor inputs for command serialization. */
    getEditInputs() {
        return this.editInputs;
    }
    /** Return whether the draft identity currently fails derivation. */
    isIdentityInvalid() {
        return this.identityInvalid;
    }
    /** Render catalog metadata as inert detail rows. */
    renderReadOnly(selected) {
        const { detailMeta, detailDescription, detailTitle } = this.options.getElements();
        if (!detailMeta || !detailDescription)
            return;
        if (detailTitle)
            detailTitle.textContent =
                selected.default_alias || selected.display_name || selected.name || '';
        const rows = [];
        const defaultAlias = selected.default_alias || selected.display_name || selected.name || '';
        if (defaultAlias)
            rows.push(`Default Alias: ${defaultAlias}`);
        if (isLocalCubeEntry(selected))
            rows.push('local');
        else if (selected.author)
            rows.push(`Author: ${selected.author}`);
        const versionUpdated = [];
        if (selected.version)
            versionUpdated.push(`Version: ${selected.version}`);
        if (selected.mtime)
            versionUpdated.push(`Updated: ${new Date(selected.mtime).toLocaleString()}`);
        if (versionUpdated.length)
            rows.push(versionUpdated.join(' | '));
        if (selected.target_model)
            rows.push(`Target Model: ${selected.target_model}`);
        if (selected.cube_id)
            rows.push(`ID: ${selected.cube_id}`);
        if (selected.lineage && typeof selected.lineage === 'object') {
            const lineage = selected.lineage;
            const parts = [];
            if (lineage.name)
                parts.push(lineage.name);
            if (lineage.version)
                parts.push(`v${lineage.version}`);
            if (lineage.id)
                parts.push(`(${lineage.id})`);
            if (lineage.author)
                parts.push(`by ${lineage.author}`);
            if (lineage.forked_at)
                parts.push(`forked ${lineage.forked_at}`);
            rows.push(`Forked from: ${parts.join(' ')}`.trim());
        }
        if (selected.supported_models?.length)
            rows.push(`Models: ${selected.supported_models.join(', ')}`);
        if (selected.author_url)
            rows.push(`Website: ${selected.author_url}`);
        if (selected.tags?.length)
            rows.push(`Tags: ${selected.tags.join(', ')}`);
        detailMeta.replaceChildren(...rows.map((row) => $el('div', row)));
        detailDescription.textContent = selected.description || '';
        this.dispose();
    }
    /** Render the complete writable metadata editor. */
    renderEditor(selected, state) {
        const elements = this.options.getElements();
        const { detailMeta, detailDescription, detailTitle } = elements;
        if (!detailMeta || !detailDescription)
            return;
        this.dispose();
        if (detailTitle) {
            const nameInput = $el('input', {
                type: 'text',
                value: draftName(state, selected),
                className: 'sugarcubes-browser__title-input',
            });
            detailTitle.replaceChildren(nameInput);
            detailTitle.title = 'Rename this cube';
            this.editInputs = { name: nameInput };
        }
        const draft = state.editDraft || createFallbackDraft(state, selected);
        const derived = deriveEditableCubeId(selected.cube_id, draft.name, draft.target_model);
        draft.derived_cube_id = derived.value;
        this.identityInvalid = Boolean(derived.error);
        const inputs = this.editInputs || {};
        detailMeta.replaceChildren();
        for (const field of [
            { key: 'current_cube_id', label: 'ID', readOnly: true },
            {
                key: 'derived_cube_id',
                label: 'New ID',
                readOnly: true,
                title: derived.error || 'Derived from Default Alias',
            },
            { key: 'version', label: 'Version', readOnly: false },
        ]) {
            const value = String(draft[field.key] || '');
            const input = $el('input', { type: 'text', value });
            if (field.readOnly) {
                input.readOnly = true;
                input.title = field.title || 'Current canonical cube identity';
            }
            detailMeta.appendChild($el('div.sugarcubes-browser__edit-field', [$el('label', field.label), input]));
            inputs[field.key] = input;
        }
        const targetModel = this.buildTargetModelEditor(draft.target_model);
        detailMeta.appendChild(targetModel.container);
        inputs.target_model = targetModel.input;
        const nameInput = inputs.name;
        const derivedInput = inputs.derived_cube_id;
        if (nameInput && derivedInput) {
            const updateIdentity = () => {
                const next = deriveEditableCubeId(selected.cube_id, nameInput.value, inputs.target_model?.value);
                derivedInput.value = next.value;
                derivedInput.title = next.error || 'Derived from Default Alias';
                this.identityInvalid = Boolean(next.error);
                if (elements.editSaveButton) {
                    elements.editSaveButton.disabled =
                        state.busy || !state.selected || !selected.is_writable || this.identityInvalid;
                }
            };
            nameInput.addEventListener('input', updateIdentity);
            inputs.target_model?.addEventListener('change', updateIdentity);
        }
        const models = this.buildModelsEditor(draft.supported_models, state);
        detailMeta.appendChild(models.container);
        inputs.supported_models = models.control;
        for (const [key, label] of [
            ['author_url', 'Website'],
            ['tags', 'Tags'],
        ]) {
            const raw = draft[key];
            const input = $el('input', {
                type: 'text',
                value: Array.isArray(raw) ? raw.join(', ') : String(raw || ''),
            });
            detailMeta.appendChild($el('div.sugarcubes-browser__edit-field', [$el('label', label), input]));
            inputs[key] = input;
        }
        const description = $el('textarea', {
            className: 'sugarcubes-browser__edit-textarea',
            value: draft.description || '',
            rows: 4,
        });
        const descriptionNodes = [description];
        inputs.description = description;
        if (selected.is_writable && selected.lineage && typeof selected.lineage === 'object') {
            const clear = $el('button.sugarcubes-browser__lineage-clear', {
                type: 'button',
                title: 'Remove fork lineage metadata',
                textContent: 'Clear lineage',
            });
            clear.addEventListener('click', () => this.options.getHandlers().onClearLineage?.(selected));
            descriptionNodes.push(clear);
        }
        detailDescription.replaceChildren(...descriptionNodes);
        this.editInputs = inputs;
    }
    /** Dispose stateful editor controls before replacing their DOM. */
    dispose() {
        this.editInputs?.supported_models?.dispose();
        this.editInputs = null;
    }
    buildTargetModelEditor(targetModel) {
        const input = $el('select', { className: 'sugarcubes-browser__target-model-select' });
        const normalized = normalizeTargetModel(targetModel);
        const options = normalized && !TARGET_MODEL_OPTIONS.includes(normalized)
            ? [normalized, ...TARGET_MODEL_OPTIONS]
            : TARGET_MODEL_OPTIONS;
        input.replaceChildren(...options.map((value) => $el('option', { value, textContent: value })));
        input.value = normalized || '';
        return {
            input,
            container: $el('div.sugarcubes-browser__edit-field', [$el('label', 'Target model'), input]),
        };
    }
    buildModelsEditor(models, state) {
        const control = new ModelAutocompleteControl({
            documentRef: this.options.documentRef ?? document,
            options: state.modelOptions,
            placeholder: 'SDXL, Flux .1 D',
            value: Array.isArray(models) ? models.filter(Boolean).join(', ') : '',
            legacyClassNames: {
                container: 'sugarcubes-browser__model-autocomplete',
                input: 'sugarcubes-browser__model-text-input',
                listbox: 'sugarcubes-browser__model-suggestions',
                suggestion: 'sugarcubes-browser__model-suggestion',
            },
        });
        return {
            container: $el('div.sugarcubes-browser__edit-field', [
                $el('label', 'Model(s)'),
                control.element,
            ]),
            control,
        };
    }
}
function isLocalCubeEntry(cube) {
    const sourceType = typeof cube.source?.type === 'string' ? cube.source.type.trim().toLowerCase() : '';
    return sourceType === 'local' || (cube.cube_id || '').trim().toLowerCase().startsWith('local/');
}
function draftName(state, selected) {
    return (state.editDraft?.name?.trim() ||
        selected.default_alias?.trim() ||
        (typeof selected.metadata?.default_alias === 'string'
            ? selected.metadata.default_alias.trim()
            : '') ||
        selected.display_name?.trim() ||
        selected.name?.trim() ||
        '');
}
function createFallbackDraft(state, selected) {
    const name = draftName(state, selected);
    const targetModel = selected.target_model || deriveTargetModelFromCubeIdSafe(selected.cube_id);
    return {
        name,
        original_name: name,
        description: selected.description || '',
        current_cube_id: selected.cube_id || '',
        derived_cube_id: deriveEditableCubeId(selected.cube_id, name).value,
        cube_id: selected.cube_id || '',
        version: selected.version || '',
        author_url: selected.author_url || '',
        tags: selected.tags || [],
        target_model: targetModel,
        supported_models: selected.supported_models || [],
    };
}
function deriveEditableCubeId(cubeId, defaultAlias, targetModel = '') {
    try {
        const alias = normalizeDefaultAliasTitle(defaultAlias);
        const model = normalizeTargetModel(targetModel);
        return {
            value: model
                ? deriveTargetModelCubeId({
                    sourceCubeId: typeof cubeId === 'string' ? cubeId : '',
                    targetModel: model,
                    defaultAlias: alias,
                })
                : deriveCubeIdFromDefaultAlias(typeof cubeId === 'string' ? cubeId : '', alias),
            error: '',
        };
    }
    catch (error) {
        return {
            value: typeof cubeId === 'string' ? cubeId : '',
            error: error instanceof Error ? error.message : String(error),
        };
    }
}
function deriveTargetModelFromCubeIdSafe(cubeId) {
    try {
        return normalizeTargetModel(deriveTargetModelFromCubeId(cubeId));
    }
    catch (_error) {
        return '';
    }
}
