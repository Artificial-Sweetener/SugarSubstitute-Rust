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
 * Own graph-wide SugarCubes instance alias allocation.
 */
import { getGroupSugarcubes } from './GroupMetadata.js';
function readName(value) {
    return typeof value === 'string' ? value.trim() : '';
}
function normalizeInstanceAlias(value) {
    return readName(value).toLowerCase();
}
function readMatchInstanceId(match) {
    return (readName(match?.instance?.instanceId) || readName(getGroupSugarcubes(match?.group)?.instance_id));
}
function isImportedGroup(group) {
    return Boolean(group?.__sugarcubes_imported);
}
function isExistingMatch(match) {
    const metadata = getGroupSugarcubes(match?.group);
    const instanceId = readMatchInstanceId(match);
    const metadataInstanceId = readName(metadata?.instance_id);
    return Boolean(match?.group &&
        metadata?.managed &&
        instanceId &&
        metadataInstanceId &&
        instanceId === metadataInstanceId &&
        !isImportedGroup(match.group));
}
function readInstanceAliasSeed(match) {
    const metadata = getGroupSugarcubes(match?.group);
    const metadataAlias = readName(metadata?.instance_alias);
    const instanceAlias = readName(match?.instance?.instanceAlias);
    const defaultAlias = readName(metadata?.default_alias) || readName(match?.instance?.defaultAlias);
    if (isExistingMatch(match)) {
        return (metadataAlias ||
            instanceAlias ||
            defaultAlias ||
            readName(match?.instance?.cubeId) ||
            'SugarCube');
    }
    return (instanceAlias ||
        metadataAlias ||
        defaultAlias ||
        readName(match?.instance?.cubeId) ||
        'SugarCube');
}
function allocateInstanceAlias(seed, taken) {
    const base = readName(seed) || 'SugarCube';
    const baseKey = normalizeInstanceAlias(base);
    if (!baseKey || !taken.has(baseKey)) {
        return base;
    }
    let index = 2;
    while (index < 1000) {
        const next = `${base} ${index}`;
        if (!taken.has(normalizeInstanceAlias(next))) {
            return next;
        }
        index += 1;
    }
    return `${base} ${Date.now()}`;
}
/**
 * Allocate instance aliases for all matched live instances in a deterministic graph-wide pass.
 */
export function allocateGraphInstanceAliases(instanceMatches = [], options = {}) {
    const ignoreGroup = typeof options.ignoreGroup === 'function' ? options.ignoreGroup : null;
    const matches = Array.isArray(instanceMatches)
        ? instanceMatches
            .filter((match) => Boolean(match.instance))
            .map((match, index) => ({
            ...match,
            order: typeof match.order === 'number' && Number.isInteger(match.order) ? match.order : index,
        }))
        : [];
    const instanceAliases = new Map();
    const taken = new Set();
    const ordered = matches.sort((left, right) => {
        const leftExisting = isExistingMatch(left);
        const rightExisting = isExistingMatch(right);
        if (leftExisting !== rightExisting) {
            return leftExisting ? -1 : 1;
        }
        return left.order - right.order;
    });
    for (const match of ordered) {
        const metadata = getGroupSugarcubes(match.group);
        if (ignoreGroup?.(match.group, metadata)) {
            continue;
        }
        const instanceId = readMatchInstanceId(match);
        if (!instanceId) {
            continue;
        }
        const instanceAlias = allocateInstanceAlias(readInstanceAliasSeed(match), taken);
        instanceAliases.set(instanceId, instanceAlias);
        taken.add(normalizeInstanceAlias(instanceAlias));
    }
    return instanceAliases;
}
