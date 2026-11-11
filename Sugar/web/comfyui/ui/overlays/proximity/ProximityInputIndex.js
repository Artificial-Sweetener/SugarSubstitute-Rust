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
/** Index proximity inputs by graph X so matching avoids a full endpoint cross-product. */
/** Own deterministic horizontal range lookup for nearby input candidates. */
export class ProximityInputIndex {
    #entries;
    /** Sort an immutable view of discovered inputs once per proximity pass. */
    constructor(inputs) {
        this.#entries = inputs
            .map((endpoint) => ({ endpoint, x: finite(endpoint.slotPos[0]) }))
            .sort((left, right) => left.x - right.x || left.endpoint.key.localeCompare(right.endpoint.key));
    }
    /** Return only inputs inside the maximum possible acquire/release distance. */
    query(output, maximumDistance) {
        const outputX = finite(output.slotPos[0]);
        const radius = Math.max(0, finite(maximumDistance));
        const start = lowerBound(this.#entries, outputX - radius);
        const result = [];
        for (let index = start; index < this.#entries.length; index += 1) {
            const entry = this.#entries[index];
            if (!entry || entry.x > outputX + radius)
                break;
            result.push(entry.endpoint);
        }
        return result;
    }
}
/** Locate the first entry whose X is not below the requested boundary. */
function lowerBound(entries, minimumX) {
    let low = 0;
    let high = entries.length;
    while (low < high) {
        const middle = Math.floor((low + high) / 2);
        const entry = entries[middle];
        if (entry && entry.x < minimumX) {
            low = middle + 1;
        }
        else {
            high = middle;
        }
    }
    return low;
}
/** Normalize dynamic host coordinates before ordering them. */
function finite(value) {
    const number = Number(value);
    return Number.isFinite(number) ? number : 0;
}
