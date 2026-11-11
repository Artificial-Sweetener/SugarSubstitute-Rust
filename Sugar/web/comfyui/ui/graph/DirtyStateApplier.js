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
 * Own the SugarCubes graph integration layer in `frontend/comfyui/ui/graph/DirtyStateApplier.js`.
 */
import { setGroupSugarcubes } from './GroupMetadata.js';
/**
 * Coordinate dirty state applier behavior for the SugarCubes UI.
 */
export class DirtyStateApplier {
    tracker;
    constructor({ tracker } = {}) {
        this.tracker = tracker || null;
    }
    applyInstanceState({ group, metadata, instanceId, cubeId, definitionKey, implementationBaselineHash, implementationCurrentHash, cosmeticBaselineHash, cosmeticCurrentHash, surfaceBaselineHash, surfaceCurrentHash, dirty, dirtyAt, implementationDirty, implementationReasons, cosmeticDirty, surfaceValuesChanged, hasSaveableChanges, initializedAt, } = {}) {
        if (!this.tracker || !instanceId) {
            return;
        }
        this.tracker.instances.set(instanceId, {
            cubeId,
            definitionKey,
            baselineHash: implementationBaselineHash,
            currentHash: implementationCurrentHash,
            implementationBaselineHash,
            implementationCurrentHash,
            cosmeticBaselineHash,
            cosmeticCurrentHash,
            surfaceBaselineHash,
            surfaceCurrentHash,
            dirty,
            dirtyAt,
            implementationDirty,
            cosmeticDirty,
            surfaceValuesChanged,
            hasSaveableChanges,
            initializedAt,
            reasons: Array.isArray(implementationReasons) ? implementationReasons : [],
        });
        if (!group || !metadata) {
            return;
        }
        if (metadata.dirty !== dirty ||
            metadata.dirty_at !== dirtyAt ||
            metadata.implementation_dirty !== implementationDirty ||
            metadata.surface_values_changed !== surfaceValuesChanged ||
            metadata.cosmetic_dirty !== cosmeticDirty ||
            metadata.has_saveable_changes !== hasSaveableChanges) {
            const next = {
                ...metadata,
                dirty,
                dirty_at: dirtyAt,
                implementation_dirty: implementationDirty,
                surface_values_changed: surfaceValuesChanged,
                cosmetic_dirty: cosmeticDirty,
                has_saveable_changes: hasSaveableChanges,
            };
            setGroupSugarcubes(group, next);
        }
    }
    finalize() {
        this.tracker?.updateDirtyCubeIds?.();
    }
}
