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
/** Own Cube browser metadata draft construction and persistence. */
import { deriveCubeIdFromDefaultAlias, normalizeDefaultAliasTitle } from '../core/CubeId.js';
import { deriveTargetModelCubeId, deriveTargetModelFromCubeId, normalizeSupportedModels, normalizeTargetModel, } from '../core/ModelTargets.js';
import { isRecord } from '../types/common.js';
import { hasApiError, readApiErrorMessage, readErrorMessage, } from './CubeBrowserContracts.js';
/** Coordinate browser-owned Cube metadata editing. */
export class CubeBrowserMetadataEditor {
    options;
    constructor(options) {
        this.options = options;
    }
    /** Toggle editing for the current writable Cube. */
    toggle() {
        const { store, view } = this.options;
        if (store.state.editing) {
            this.cancel();
            return;
        }
        const selected = this.options.getCube(store.state.selected);
        if (!selected)
            return;
        if (!selected.is_writable) {
            this.options.toast?.push('warn', 'Read-only cube', selected.write_block_reason?.trim() || 'This cube is read-only.');
            return;
        }
        store.setEditing(true, this.buildDraft(selected));
        view.editInputs = null;
        this.options.render();
    }
    /** Enter metadata editing for a known writable Cube. */
    begin(selected) {
        this.options.store.setEditing(true, this.buildDraft(selected));
        this.options.view.editInputs = null;
        this.options.render();
    }
    /** Cancel the active metadata draft. */
    cancel() {
        this.options.store.setEditing(false, null);
        this.options.view.editInputs = null;
        this.options.render();
    }
    /** Persist the active metadata draft and reconcile an identity rename. */
    async save() {
        const { store, view } = this.options;
        const selected = this.options.getCube(store.state.selected);
        const inputs = view.getEditInputs();
        if (!selected || !inputs)
            return;
        const newName = normalizeDefaultAliasTitle(String(inputs.name?.value || '')
            .split('/')
            .pop());
        const originalName = normalizeDefaultAliasTitle((store.state.editDraft?.original_name || this.resolveTitle(selected)).split('/').pop());
        const targetModel = normalizeTargetModel(inputs.target_model?.value) || this.deriveTargetModel(selected.cube_id);
        const routeAlias = targetModel
            ? `${targetModel}/${newName || originalName}`
            : newName || originalName;
        const metadata = {
            default_alias: routeAlias,
            author_url: inputs.author_url?.value ?? '',
            tags: this.parseList(inputs.tags?.value ?? ''),
            supported_models: normalizeSupportedModels(this.parseList(inputs.supported_models?.input?.value ?? ''), { targetModel }),
        };
        if (targetModel)
            metadata.target_model = targetModel;
        let activeCubeId = selected.cube_id || '';
        store.setBusy(true);
        this.options.render();
        try {
            const targetCubeId = targetModel
                ? deriveTargetModelCubeId({
                    sourceCubeId: selected.cube_id || '',
                    targetModel,
                    defaultAlias: newName || originalName,
                })
                : deriveCubeIdFromDefaultAlias(selected.cube_id || '', newName || originalName);
            if (targetCubeId && targetCubeId !== selected.cube_id) {
                const confirmed = await this.options.getActions().openConfirmDialog?.({
                    title: 'Rename SugarCube?',
                    message: [
                        'Changing the default alias renames this cube and updates its canonical ID.',
                        `Version ${inputs.version?.value?.trim() || selected.version || 'history'} and local flavors will be preserved.`,
                    ],
                    confirmLabel: 'Rename',
                });
                if (!confirmed)
                    return;
                activeCubeId = await this.rename(selected, routeAlias, targetCubeId);
                if (!activeCubeId)
                    return;
            }
            const payload = {
                cube_id: activeCubeId,
                description: inputs.description?.value ?? '',
                metadata,
            };
            const version = inputs.version?.value?.trim() || '';
            if (version)
                payload.version = version;
            const { response, data } = await this.options.api.updateMetadata(JSON.stringify(payload), {
                headers: { 'Content-Type': 'application/json' },
            });
            if (!response.ok || hasApiError(data)) {
                this.options.toast?.push('error', 'Metadata update failed', readApiErrorMessage(data, response.statusText || 'Update failed'));
                return;
            }
            store.setEditing(false, null);
            view.editInputs = null;
            const cubes = await this.options.refresh();
            const refreshed = this.options.getCube(activeCubeId, cubes);
            this.options.selectCube(this.options.getCubeKey(refreshed) || activeCubeId);
        }
        catch (error) {
            this.options.toast?.push('error', 'Metadata update failed', readErrorMessage(error));
        }
        finally {
            store.setBusy(false);
            this.options.render();
        }
    }
    /** Build the stable metadata edit draft shown by the browser view. */
    buildDraft(selected) {
        const editableName = this.resolveTitle(selected);
        const targetModel = normalizeTargetModel(selected.target_model) || this.deriveTargetModel(selected.cube_id);
        let derivedCubeId = selected.cube_id || '';
        try {
            derivedCubeId = targetModel
                ? deriveTargetModelCubeId({
                    sourceCubeId: selected.cube_id || '',
                    targetModel,
                    defaultAlias: normalizeDefaultAliasTitle(editableName) || editableName,
                })
                : deriveCubeIdFromDefaultAlias(selected.cube_id || '', normalizeDefaultAliasTitle(editableName) || editableName);
        }
        catch (_error) {
            derivedCubeId = selected.cube_id || '';
        }
        return {
            name: editableName,
            original_name: editableName,
            description: selected.description || '',
            current_cube_id: selected.cube_id || '',
            derived_cube_id: derivedCubeId,
            cube_id: selected.cube_id || '',
            version: selected.version || '',
            author_url: selected.author_url || '',
            tags: Array.isArray(selected.tags) ? selected.tags.slice() : [],
            target_model: targetModel,
            supported_models: Array.isArray(selected.supported_models)
                ? selected.supported_models.slice()
                : [],
        };
    }
    async rename(selected, defaultAlias, targetCubeId) {
        const { response, data } = await this.options.api.rename(JSON.stringify({
            cube_id: selected.cube_id || '',
            default_alias: defaultAlias,
            target_cube_id: targetCubeId,
        }), { headers: { 'Content-Type': 'application/json' } });
        if (!response.ok || hasApiError(data)) {
            this.options.toast?.push('error', 'Rename failed', readApiErrorMessage(data, response.statusText || 'Rename failed'));
            return '';
        }
        const renamedCube = isRecord(data.cube) ? data.cube : null;
        const cubeId = (typeof renamedCube?.cube_id === 'string' ? renamedCube.cube_id : '') || targetCubeId;
        this.options.getActions().reconcileCubeIdentity?.({
            previousCubeId: selected.cube_id || '',
            cubeId,
            defaultAlias: (typeof renamedCube?.default_alias === 'string' ? renamedCube.default_alias : '') ||
                defaultAlias,
        });
        return cubeId;
    }
    resolveTitle(selected) {
        if (selected.default_alias?.trim())
            return selected.default_alias.trim();
        if (typeof selected.metadata?.default_alias === 'string' &&
            selected.metadata.default_alias.trim()) {
            return selected.metadata.default_alias.trim();
        }
        return selected.name?.trim() || '';
    }
    deriveTargetModel(cubeId) {
        try {
            return normalizeTargetModel(deriveTargetModelFromCubeId(cubeId));
        }
        catch (_error) {
            return '';
        }
    }
    parseList(value) {
        return typeof value === 'string'
            ? value
                .split(',')
                .map((entry) => entry.trim())
                .filter(Boolean)
            : [];
    }
}
