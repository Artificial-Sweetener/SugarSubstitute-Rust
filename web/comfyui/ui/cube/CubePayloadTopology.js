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
/** Translate prepared Cube payload connections into graph-owned topology. */
import { isRecord } from '../types/common.js';
/** Parse marker-era payloads without preserving marker nodes as runtime objects. */
export function buildCubePayloadTopology(payload) {
    const nodeSymbols = new Set((payload.nodes ?? [])
        .map((entry) => readString(entry.symbol))
        .filter((symbol) => symbol !== null));
    const markerKinds = new Map();
    for (const marker of payload.markers ?? []) {
        const alias = readString(marker.alias);
        const kind = marker.kind === 'input' || marker.kind === 'output' ? marker.kind : null;
        if (alias && kind)
            markerKinds.set(alias, kind);
    }
    const nodeConnections = [];
    const inputTargets = new Map();
    const outputs = [];
    for (const connection of payload.connections ?? []) {
        const parsed = parseConnection(connection);
        if (!parsed)
            continue;
        const sourceKind = markerKinds.get(parsed.sourceSymbol);
        const targetKind = markerKinds.get(parsed.targetSymbol);
        if (sourceKind === 'input' && nodeSymbols.has(parsed.targetSymbol)) {
            const targets = inputTargets.get(parsed.sourceSymbol) ?? [];
            targets.push({ symbol: parsed.targetSymbol, input: parsed.targetInput });
            inputTargets.set(parsed.sourceSymbol, targets);
            continue;
        }
        if (targetKind === 'output' && nodeSymbols.has(parsed.sourceSymbol)) {
            outputs.push({
                name: parsed.targetSymbol,
                sourceSymbol: parsed.sourceSymbol,
                sourceSlot: parsed.sourceSlot,
            });
            continue;
        }
        if (nodeSymbols.has(parsed.sourceSymbol) && nodeSymbols.has(parsed.targetSymbol)) {
            nodeConnections.push(parsed);
        }
    }
    return {
        nodes: [...(payload.nodes ?? [])],
        nodeConnections,
        inputs: [...inputTargets].map(([name, targets]) => ({ name, targets })),
        outputs,
    };
}
/** Parse one untrusted prepared connection into stable symbolic endpoints. */
function parseConnection(connection) {
    const source = isRecord(connection.from) ? connection.from : {};
    const target = isRecord(connection.to) ? connection.to : {};
    const sourceSymbol = readString(source.symbol);
    const targetSymbol = readString(target.symbol);
    const targetInput = readString(target.input);
    if (!sourceSymbol || !targetSymbol || !targetInput)
        return null;
    const sourceSlotValue = Number(source.slot);
    return {
        sourceSymbol,
        sourceSlot: Number.isInteger(sourceSlotValue) && sourceSlotValue >= 0 ? sourceSlotValue : 0,
        targetSymbol,
        targetInput,
    };
}
/** Return one trimmed non-empty string from an untrusted host value. */
function readString(value) {
    return typeof value === 'string' && value.trim() ? value.trim() : null;
}
