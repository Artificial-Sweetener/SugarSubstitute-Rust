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
 * Own the SugarCubes graph integration layer in `frontend/comfyui/ui/graph/BaselineStore.js`.
 */
/**
 * Coordinate baseline store behavior for the SugarCubes UI.
 */
export class BaselineStore {
    definitionImplementationHashByCubeId;
    definitionStatusByCubeId;
    localImplementationHashByInstanceId;
    localCosmeticHashByInstanceId;
    constructor() {
        this.definitionImplementationHashByCubeId = new Map();
        this.definitionStatusByCubeId = new Map();
        this.localImplementationHashByInstanceId = new Map();
        this.localCosmeticHashByInstanceId = new Map();
    }
    setDefinition(cubeId, entry = {}) {
        if (!cubeId) {
            return;
        }
        const definition = entry && typeof entry === 'object' ? entry : {};
        const hash = typeof definition.hash === 'string' ? definition.hash : '';
        const status = typeof definition.status === 'string' ? definition.status : '';
        if (hash) {
            this.definitionImplementationHashByCubeId.set(cubeId, hash);
        }
        else {
            this.definitionImplementationHashByCubeId.delete(cubeId);
        }
        if (status) {
            this.definitionStatusByCubeId.set(cubeId, status);
        }
        else {
            this.definitionStatusByCubeId.delete(cubeId);
        }
    }
    getDefinitionHash(cubeId) {
        if (!cubeId) {
            return null;
        }
        return this.definitionImplementationHashByCubeId.get(cubeId) || null;
    }
    getDefinitionStatus(cubeId) {
        if (!cubeId) {
            return null;
        }
        return this.definitionStatusByCubeId.get(cubeId) || null;
    }
    setLocalBaselineHash(instanceId, hash) {
        this.setLocalImplementationHash(instanceId, hash);
    }
    getLocalBaselineHash(instanceId) {
        return this.getLocalImplementationHash(instanceId);
    }
    clearLocalBaseline(instanceId) {
        this.clearLocalImplementationHash(instanceId);
    }
    setLocalImplementationHash(instanceId, hash) {
        if (!instanceId) {
            return;
        }
        if (typeof hash === 'string' && hash) {
            this.localImplementationHashByInstanceId.set(instanceId, hash);
        }
        else {
            this.localImplementationHashByInstanceId.delete(instanceId);
        }
    }
    getLocalImplementationHash(instanceId) {
        if (!instanceId) {
            return null;
        }
        return this.localImplementationHashByInstanceId.get(instanceId) || null;
    }
    clearLocalImplementationHash(instanceId) {
        if (!instanceId) {
            return;
        }
        this.localImplementationHashByInstanceId.delete(instanceId);
    }
    setLocalCosmeticHash(instanceId, hash) {
        if (!instanceId) {
            return;
        }
        if (typeof hash === 'string' && hash) {
            this.localCosmeticHashByInstanceId.set(instanceId, hash);
        }
        else {
            this.localCosmeticHashByInstanceId.delete(instanceId);
        }
    }
    getLocalCosmeticHash(instanceId) {
        if (!instanceId) {
            return null;
        }
        return this.localCosmeticHashByInstanceId.get(instanceId) || null;
    }
    clearLocalCosmeticHash(instanceId) {
        if (!instanceId) {
            return;
        }
        this.localCosmeticHashByInstanceId.delete(instanceId);
    }
    pruneLocalBaselines(activeInstanceIds) {
        if (!(activeInstanceIds instanceof Set)) {
            return;
        }
        for (const instanceId of this.localImplementationHashByInstanceId.keys()) {
            if (!activeInstanceIds.has(instanceId)) {
                this.localImplementationHashByInstanceId.delete(instanceId);
            }
        }
        for (const instanceId of this.localCosmeticHashByInstanceId.keys()) {
            if (!activeInstanceIds.has(instanceId)) {
                this.localCosmeticHashByInstanceId.delete(instanceId);
            }
        }
    }
}
