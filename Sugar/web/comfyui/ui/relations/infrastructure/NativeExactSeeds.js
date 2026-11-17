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
/** Keep the exact digits of the seeds a workflow shows, where its runs read them. */
import { isRecord } from '../../types/common.js';
import { readExactDigits, recordExactIntegers, } from '../../workflow/ExactIntegerRelations.js';
import { resolveNativeSeedOwner } from '../../surface/appearance/ComfyNativeSeedControl.js';
/**
 * Give each seed box the digits recorded for the seed it edits.
 *
 * A seed above a page number's exact range is held by its box as a rounding. The workflow records
 * its digits per Cube input, and a run gives those digits to the sampler. A box that edits one
 * Cube's seed reads that input's record; the shared seed reads and writes every Cube it drives.
 */
export class NativeExactSeeds {
    getGraph;
    relations;
    inventory;
    /** Read the root workflow and the relation inventory instead of browser state. */
    constructor(getGraph, relations, inventory) {
        this.getGraph = getGraph;
        this.relations = relations;
        this.inventory = inventory;
    }
    /** Resolve the digits of the seed one native field edits. */
    field(node, name) {
        return this.#text(() => {
            const native = resolveNativeSeedOwner(node, name);
            for (const [id, owner] of this.inventory.owners) {
                if (owner.node === node && owner.widget.name === name)
                    return [id];
                const candidate = native ? resolveNativeSeedOwner(owner.node, owner.widget.name) : null;
                if (candidate && candidate.node === native?.node && candidate.widget === native.widget)
                    return [id];
            }
            return [];
        });
    }
    /** Resolve the digits of one shared seed across every Cube it drives. */
    group(key) {
        return this.#text(() => this.relations
            .snapshot()
            .groups.find((group) => group.key === key)
            ?.members.map((member) => member.id) ?? []);
    }
    /** Read one record common to every input, and write the same record to each. */
    #text(fieldIds) {
        const targets = () => fieldIds().flatMap((id) => {
            const owner = this.inventory.owners.get(id);
            const symbol = owner ? nodeSymbol(owner.nodeId) : null;
            return owner?.cubeId && symbol !== null
                ? [{ instanceId: owner.cubeId, nodeSymbol: symbol, inputName: owner.inputName }]
                : [];
        });
        return {
            read: (value) => {
                const graph = this.getGraph();
                const inputs = targets();
                if (!isRecord(graph) || !inputs.length)
                    return null;
                const digits = inputs.map((input) => readExactDigits(graph, input, value));
                const first = digits[0] ?? null;
                return digits.every((value) => value === first) ? first : null;
            },
            write: (digits) => {
                const graph = this.getGraph();
                const inputs = targets();
                if (!isRecord(graph) || !inputs.length)
                    return;
                recordExactIntegers(graph, inputs.map((input) => ({ ...input, digits })));
            },
        };
    }
}
/** Read the Cube node symbol from a relation node identity. */
function nodeSymbol(nodeId) {
    try {
        const parts = JSON.parse(nodeId);
        return Array.isArray(parts) && parts[0] === 'cube' && typeof parts[2] === 'string'
            ? parts[2]
            : null;
    }
    catch {
        return null;
    }
}
