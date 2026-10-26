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
/** Validate durable workflow source bindings before using native document references. */
import { isRecord } from '../../types/common.js';
import { readMaskEditSettings } from '../domain/MaskEditSettings.js';
import { isCanvasInputSource } from '../../core/embedded/CanvasRuntime.js';
import { IMAGE_BINDING_KEY, MASK_BINDING_KEY, } from '../domain/InputBinding.js';
/** Read persisted bindings strictly so malformed entries cannot silently become another source. */
export function readInputBinding(properties, kind, field) {
    if (kind === 'image') {
        if (!isRecord(properties) || !Object.hasOwn(properties, IMAGE_BINDING_KEY))
            return null;
        const saved = properties[IMAGE_BINDING_KEY];
        if (!isRecord(saved) ||
            saved.version !== 1 ||
            !isRecord(saved.fields) ||
            Object.keys(saved.fields).length > 64)
            throw new Error('Invalid image source bindings.');
        if (!field || !Object.hasOwn(saved.fields, field))
            return null;
        const state = parseBinding(saved.fields[field]);
        if (!state || state.field !== field)
            throw new Error('The saved image source field changed.');
        return state;
    }
    if (!isRecord(properties) || !Object.hasOwn(properties, MASK_BINDING_KEY))
        return null;
    return parseBinding(properties[MASK_BINDING_KEY]);
}
/** Validate the shared binding value independently of its public per-kind persistence envelope. */
function parseBinding(value) {
    if (isRecord(value) && value.version === 1)
        return null;
    if (!isRecord(value) ||
        value.version !== 2 ||
        typeof value.field !== 'string' ||
        value.field.length > 128 ||
        !Array.isArray(value.entries) ||
        !Array.isArray(value.files) ||
        value.files.length > 64 ||
        value.files.some((file) => typeof file !== 'string' || file.length > 4096) ||
        value.entries.length > 64 ||
        (value.selected !== null && typeof value.selected !== 'string'))
        throw new Error('Invalid canvas source binding.');
    const entries = [];
    const ids = new Set();
    for (const entry of value.entries) {
        if (!isRecord(entry) ||
            typeof entry.id !== 'string' ||
            !entry.id ||
            entry.id.length > 128 ||
            ids.has(entry.id) ||
            typeof entry.label !== 'string' ||
            entry.label.length > 128 ||
            !isCanvasInputSource(entry.source))
            throw new Error('Invalid canvas source entry.');
        ids.add(entry.id);
        entries.push({ id: entry.id, label: entry.label, source: entry.source });
    }
    if (value.selected !== null && !ids.has(value.selected))
        throw new Error('Invalid selected canvas source.');
    return {
        version: 2,
        field: value.field,
        selected: value.selected,
        entries,
        files: value.files.filter((file) => typeof file === 'string'),
        ...(value.edits === undefined ? {} : { edits: readMaskEditSettings(value.edits) }),
    };
}
/** Replace only the owning field's metadata and retain every sibling field and unrelated node property. */
export function withInputBinding(properties, kind, state) {
    const current = isRecord(properties) ? properties : {};
    if (kind === 'mask')
        return { ...current, [MASK_BINDING_KEY]: state };
    const saved = current[IMAGE_BINDING_KEY];
    if (saved !== undefined && (!isRecord(saved) || saved.version !== 1 || !isRecord(saved.fields)))
        throw new Error('Invalid image source bindings.');
    const fields = isRecord(saved) && isRecord(saved.fields) ? saved.fields : {};
    return {
        ...current,
        [IMAGE_BINDING_KEY]: { version: 1, fields: { ...fields, [state.field]: state } },
    };
}
