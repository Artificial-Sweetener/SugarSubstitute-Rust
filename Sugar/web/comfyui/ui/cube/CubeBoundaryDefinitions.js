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
/** Parse the read-only canonical Cube boundary contract supplied by the importer. */
import { isRecord } from '../types/common.js';
/** Read complete validated boundary definitions, or reject the dynamic payload as legacy. */
export function readCubeBoundaryDefinitions(value) {
    if (!isRecord(value))
        return null;
    const inputs = readArray(value.inputs, readInputBoundary);
    const outputs = readArray(value.outputs, readOutputBoundary);
    if (!inputs || !outputs)
        return null;
    return { inputs, outputs };
}
/** Read one persisted input boundary with at least one valid target. */
function readInputBoundary(value) {
    if (!isRecord(value))
        return null;
    const id = readString(value.id);
    const name = readString(value.name);
    const label = readString(value.label);
    const type = readPortType(value.type);
    const targets = readArray(value.targets, readTarget);
    if (!id || !name || !label || !type || !targets || targets.length === 0)
        return null;
    return { id, name, label, type, targets };
}
/** Read one persisted output boundary with its explicit source endpoint. */
function readOutputBoundary(value) {
    if (!isRecord(value))
        return null;
    const id = readString(value.id);
    const name = readString(value.name);
    const label = readString(value.label);
    const type = readPortType(value.type);
    const source = readSource(value.source);
    if (!id || !name || !label || !type || !source)
        return null;
    return { id, name, label, type, source };
}
/** Read one typed internal input endpoint. */
function readTarget(value) {
    if (!isRecord(value))
        return null;
    const symbol = readString(value.symbol);
    const input = readString(value.input);
    return symbol && input ? { symbol, input } : null;
}
/** Read one typed internal output endpoint. */
function readSource(value) {
    if (!isRecord(value))
        return null;
    const symbol = readString(value.symbol);
    const slot = Number(value.slot);
    return symbol && Number.isInteger(slot) && slot >= 0 ? { symbol, slot } : null;
}
/** Read every valid entry while rejecting malformed arrays as an atomic boundary contract. */
function readArray(value, readEntry) {
    if (!Array.isArray(value))
        return null;
    const entries = value.map(readEntry);
    return entries.every((entry) => entry !== null) ? entries : null;
}
/** Read one non-empty dynamic string. */
function readString(value) {
    return typeof value === 'string' ? value.trim() : '';
}
/** Preserve the wildcard as an intentional serialized boundary type. */
function readPortType(value) {
    return readString(value);
}
