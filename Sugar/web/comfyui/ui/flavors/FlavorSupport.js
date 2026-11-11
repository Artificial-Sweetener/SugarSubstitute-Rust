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
/** Define shared Flavor contracts and normalization policy. */
import { filterTrackedSurfaceValues } from '../core/SurfaceValuePolicy.js';
import { readWidgetValue, writeWidgetValue } from '../graph/Markers.js';
import { normalizeAuthoredFlavors, normalizeFlavorId } from './FlavorSelection.js';
import { isRecord } from '../types/common.js';
/** Normalize unknown group metadata to the Flavor metadata contract. */
export function asFlavorMetadata(value) {
    return isRecord(value) ? value : {};
}
/** Read a useful error message with a stable fallback. */
export function errorMessage(error, fallback) {
    return error instanceof Error && error.message ? error.message : fallback;
}
/** Read a structured Flavor API response error. */
export function responseErrorMessage(data) {
    const error = isRecord(data.error) ? data.error : {};
    return typeof error.message === 'string' ? error.message : '';
}
/** Read the saved Flavor identity from an API response. */
export function savedFlavorId(data) {
    const saved = isRecord(data.saved) ? data.saved : {};
    return typeof saved.flavor_id === 'string' ? saved.flavor_id : '';
}
/** Clone JSON-compatible Flavor values before crossing ownership boundaries. */
export function cloneValue(value) {
    return JSON.parse(JSON.stringify(value));
}
/** Read the property fallback for one surface input. */
export function readNodePropertyValue(node, inputName) {
    return node.properties && Object.prototype.hasOwnProperty.call(node.properties, inputName)
        ? node.properties[inputName]
        : null;
}
/** Apply one Flavor value through the host's preferred node boundary. */
export function applyNodeValue(node, inputName, value) {
    if (writeWidgetValue(node, inputName, value))
        return true;
    if (typeof node.setProperty === 'function') {
        try {
            node.setProperty(inputName, value);
            return true;
        }
        catch (_error) {
            // Fall through to direct property ownership.
        }
    }
    if (!node.properties || typeof node.properties !== 'object')
        node.properties = {};
    node.properties[inputName] = value;
    if (typeof node.onPropertyChanged === 'function') {
        try {
            node.onPropertyChanged(inputName, value);
        }
        catch (_error) {
            // Host callbacks cannot invalidate the applied property value.
        }
    }
    return true;
}
/** Normalize a Cube surface for Flavor projection. */
export function normalizeSurface(surface) {
    if (!isRecord(surface))
        return { default_flavor_id: 'default', controls: [] };
    return {
        default_flavor_id: typeof surface.default_flavor_id === 'string' && surface.default_flavor_id.trim()
            ? surface.default_flavor_id.trim()
            : 'default',
        controls: Array.isArray(surface.controls) ? surface.controls : [],
    };
}
/** Normalize authored Flavors and retain only tracked surface values. */
export function normalizeAuthoredFlavorEntries(flavors, surface = normalizeSurface(null)) {
    return normalizeAuthoredFlavors(flavors, surface.default_flavor_id).map((entry) => ({
        ...entry,
        values: filterTrackedSurfaceValues(surface, entry.values),
    }));
}
/** Normalize local Flavors and retain only tracked surface values. */
export function normalizeLocalFlavorEntries(flavors, surface) {
    return (Array.isArray(flavors) ? flavors : []).flatMap((entry) => {
        if (!isRecord(entry) || typeof entry.id !== 'string' || !entry.id)
            return [];
        return [
            {
                id: entry.id,
                name: typeof entry.name === 'string' ? entry.name : entry.id,
                scope: 'local',
                stale: Boolean(entry.stale),
                values: filterTrackedSurfaceValues(normalizeSurface(surface), entry.values),
            },
        ];
    });
}
/** Resolve the established dialog name seed from instance metadata. */
export function resolveFlavorNameSeed(metadata) {
    return (metadata.instance_alias?.trim() ||
        metadata.default_alias?.trim() ||
        metadata.cube_id?.trim() ||
        '');
}
/** Normalize a Flavor name for collision comparison. */
export function normalizeFlavorNameKey(value) {
    return String(value || '')
        .trim()
        .toLowerCase();
}
/** Build normalized ID and name sets for Flavor collision policy. */
export function flavorKeySets(flavors) {
    const ids = new Set();
    const names = new Set();
    for (const flavor of Array.isArray(flavors) ? flavors : []) {
        const id = normalizeFlavorId(flavor?.id);
        const name = normalizeFlavorNameKey(flavor?.name);
        if (id)
            ids.add(id);
        if (name)
            names.add(name);
    }
    return { ids, names };
}
/** Return local Flavors that collide with authored identities or names. */
export function findLocalFlavorCollisions(local, authored) {
    const keys = flavorKeySets(authored);
    return local.filter((flavor) => {
        const id = normalizeFlavorId(flavor.id);
        const name = normalizeFlavorNameKey(flavor.name);
        return Boolean((id && keys.ids.has(id)) || (name && keys.names.has(name)));
    });
}
export { readWidgetValue };
