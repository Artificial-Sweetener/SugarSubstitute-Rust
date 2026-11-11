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
/** Resolve the exact graph instances affected by persisted Cube saves. */
import { STALE_SAVE_MODE_LATEST } from './CubeSaveContracts.js';
/** Own marker and native-node reconciliation target projection. */
export class CubeSaveReconciliationTargets {
    /** Resolve legacy marker ids for every saved Cube. */
    buildMarkerTargets(savePlan) {
        const targets = {};
        for (const entry of savePlan) {
            if (!entry.cubeId)
                continue;
            if (entry.forked) {
                targets[entry.cubeId] = (entry.reconciliationMarkerIds || []).map(String);
            }
            else {
                const sources = entry.staleSaveMode === STALE_SAVE_MODE_LATEST
                    ? entry.sourceEntries.filter((source) => source.staleRevision)
                    : entry.selectedSourceEntry && !entry.selectedSourceEntry.staleRevision
                        ? [entry.selectedSourceEntry]
                        : [];
                targets[entry.cubeId] = Array.from(new Set(sources.flatMap((source) => source.markerIds || []).map(String)));
            }
        }
        return targets;
    }
    /** Resolve native Cube-node instance ids for every saved Cube. */
    buildCubeNodeTargets(savePlan) {
        const targets = {};
        for (const entry of savePlan) {
            if (!entry.cubeId)
                continue;
            if (entry.forked && entry.reconciliationCubeNodeInstanceIds) {
                targets[entry.cubeId] = entry.reconciliationCubeNodeInstanceIds.map(String);
                continue;
            }
            const sources = entry.staleSaveMode === STALE_SAVE_MODE_LATEST
                ? entry.sourceEntries.filter((source) => source.staleRevision)
                : entry.selectedSourceEntry
                    ? [entry.selectedSourceEntry]
                    : [];
            targets[entry.cubeId] = Array.from(new Set(sources
                .map((source) => source.cubeNodeInstanceId)
                .filter((instanceId) => Boolean(instanceId))
                .map(String)));
        }
        return targets;
    }
}
