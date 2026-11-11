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
/** Own versioned relation persistence in root workflow metadata. */
import { isRecord } from '../../types/common.js';
import { emptyRelationState } from '../domain/RelationContracts.js';
import { isFieldValue } from './NativeFieldContract.js';
/** Identify root workflow relation metadata independently from reusable Cube data. */
export const RELATION_METADATA_KEY = 'sugarcubes_relations';
/** Validate every untrusted persisted relation without leaking dynamic values into policy. */
export function parseRelationState(value) {
    const state = emptyRelationState();
    if (!isRecord(value) || value.schema !== 1)
        return state;
    if (isRecord(value.overrides))
        for (const [key, item] of Object.entries(value.overrides)) {
            if (isRecord(item) && typeof item.enabled === 'boolean' && isFieldValue(item.value))
                state.overrides[key] = {
                    enabled: item.enabled,
                    value: item.value,
                    seedMode: seedMode(item.seedMode),
                };
        }
    for (const key of ['fieldLinks', 'nodeLinks'])
        if (isRecord(value[key]))
            for (const [target, source] of Object.entries(value[key]))
                if (source === null || typeof source === 'string')
                    state[key][target] = source;
    if (isRecord(value.locals))
        for (const [key, item] of Object.entries(value.locals))
            if (isFieldValue(item))
                state.locals[key] = item;
    if (isRecord(value.localSeedModes))
        for (const [key, item] of Object.entries(value.localSeedModes))
            state.localSeedModes[key] = seedMode(item);
    if (isRecord(value.localNodeModes))
        for (const [key, item] of Object.entries(value.localNodeModes))
            if (typeof item === 'number' && [0, 2, 4].includes(item))
                state.localNodeModes[key] = item;
    return state;
}
/** Normalize persisted generation mode without accepting arbitrary combo strings. */
function seedMode(value) {
    return value === 'randomize' || value === 'increment' || value === 'decrement' ? value : 'fixed';
}
