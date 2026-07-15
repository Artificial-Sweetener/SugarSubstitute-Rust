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
 * Own the interactive personal-to-pack promotion use case.
 */
import { normalizeDefaultAliasTitle, parseCanonicalCubeId } from '../core/CubeId.js';
import { normalizeSupportedModels, normalizeTargetModel } from '../core/ModelTargets.js';
import { isRecord } from '../types/common.js';
function readApiError(data) {
    const error = isRecord(data.error) ? data.error : {};
    return typeof error.message === 'string' ? error.message : '';
}
/** Coordinate pack selection, sharing metadata, backend promotion, and graph reconciliation. */
export class CubePromotionService {
    api;
    dialogs;
    toast;
    packService;
    identityReconciler;
    cubeBrowser;
    constructor({ api, dialogs, toast, packService, identityReconciler, cubeBrowser, } = {}) {
        this.api = api ?? null;
        this.dialogs = dialogs ?? null;
        this.toast = toast ?? null;
        this.packService = packService ?? null;
        this.identityReconciler = identityReconciler ?? null;
        this.cubeBrowser = cubeBrowser ?? null;
    }
    /** Return whether one browser cube is eligible for explicit promotion. */
    canPromote(cube) {
        try {
            const parsed = parseCanonicalCubeId(cube?.cube_id);
            return Boolean(parsed.sourceKind === 'local' && parsed.namespace === 'personal' && cube?.is_writable);
        }
        catch (_error) {
            return false;
        }
    }
    /** Run the complete promotion flow for one selected personal cube. */
    async promote(cube) {
        if (!this.canPromote(cube)) {
            this.toast?.push('warn', 'Promotion unavailable', 'Select a writable personal cube first.');
            return null;
        }
        try {
            if (!this.packService || !this.api || !cube?.cube_id) {
                throw new Error('Promotion services are unavailable.');
            }
            const destination = await this.packService.chooseWritablePack();
            if (!destination) {
                return null;
            }
            const values = await this.collectSharingDetails(cube, destination);
            if (!values) {
                return null;
            }
            const request = this.buildRequest(cube, destination, values);
            const { response, data } = await this.api.promote(JSON.stringify(request), {
                headers: { 'Content-Type': 'application/json' },
            });
            if (!response.ok || data.error) {
                throw new Error(readApiError(data) || response.statusText || 'Promotion failed');
            }
            const promotedCube = isRecord(data.cube) ? data.cube : {};
            const targetCubeId = String(promotedCube.cube_id || '').trim();
            if (!targetCubeId) {
                throw new Error('Promotion response did not include the managed cube identity.');
            }
            const defaultAlias = String(promotedCube.default_alias || '').trim();
            this.identityReconciler?.reconcile?.({
                previousCubeId: cube.cube_id,
                cubeId: targetCubeId,
                ...(defaultAlias ? { defaultAlias } : {}),
            });
            await this.cubeBrowser?.refresh?.({ force: true });
            this.cubeBrowser?.selectCube?.(targetCubeId, { focus: false, silent: true });
            if (data.status === 'cleanup_pending') {
                this.toast?.push('warn', 'SugarCube moved; cleanup pending', 'The managed pack copy is safe. SugarCubes will finish retiring the personal copy on retry.');
            }
            else {
                this.toast?.push('success', 'SugarCube moved to pack', `${targetCubeId} keeps version ${data.version || cube.version || 'history'}.`);
            }
            return data;
        }
        catch (error) {
            this.toast?.push('error', 'Promotion failed', error instanceof Error ? error.message : String(error));
            return null;
        }
    }
    /** Collect only the metadata required to publish into a managed pack. */
    async collectSharingDetails(cube, destination) {
        const initialName = normalizeDefaultAliasTitle(String(cube?.display_name || cube?.default_alias || cube?.name || 'SugarCube')
            .split('/')
            .pop());
        return ((await this.dialogs?.openForm?.({
            title: 'Move SugarCube to Pack',
            message: [
                `Publish into ${destination.repoRef}. The personal history and version will follow the cube.`,
            ],
            confirmLabel: 'Move to Pack',
            fields: [
                {
                    key: 'name',
                    label: 'Pack name',
                    initialValue: initialName,
                    required: true,
                    normalizeValue: (value) => normalizeDefaultAliasTitle(value),
                },
                {
                    key: 'target_model',
                    label: 'Target model',
                    initialValue: normalizeTargetModel(cube?.target_model) || 'Any',
                    required: true,
                    normalizeValue: (value) => value.trim(),
                },
                {
                    key: 'supported_models',
                    label: 'Also supports',
                    initialValue: (Array.isArray(cube?.supported_models) ? cube.supported_models : []).join(', '),
                    helperText: 'Optional comma-separated model families.',
                },
                {
                    key: 'description',
                    label: 'Description',
                    initialValue: typeof cube?.description === 'string' ? cube.description : '',
                },
            ],
        })) ?? null);
    }
    /** Build the explicit backend promotion command from confirmed values. */
    buildRequest(cube, destination, values) {
        const targetModel = normalizeTargetModel(values?.target_model);
        if (!targetModel) {
            throw new Error('Target model is required.');
        }
        return {
            source_cube_id: cube.cube_id ?? '',
            destination: { owner: destination.owner, repo: destination.repo },
            name: normalizeDefaultAliasTitle(values?.name),
            target_model: targetModel,
            supported_models: normalizeSupportedModels(values?.supported_models, {
                targetModel,
            }),
            description: typeof values?.description === 'string' ? values.description : '',
        };
    }
}
