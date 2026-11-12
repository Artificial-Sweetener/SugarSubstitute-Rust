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
/** Pack a finite batch into native-aspect frames using the Output canvas reference policy. */
/** Maximize reference image area and retain a settled topology within the reference's 2% hysteresis. */
export function canvasBatchLayout(count, viewport, reference, previousColumns) {
    if (!Number.isInteger(count) ||
        count < 1 ||
        count > 8 ||
        ![viewport.width, viewport.height, reference.width, reference.height].every((value) => Number.isFinite(value) && value > 0))
        throw new RangeError('Canvas packed batch geometry is invalid.');
    const candidates = [];
    for (let columns = 1; columns <= Math.max(1, count); columns++) {
        const rows = Math.ceil(count / columns);
        const horizontal = columns * reference.width;
        const vertical = rows * reference.height;
        const gaps = horizontal >= vertical ? columns - 1 : rows - 1;
        const nativeGap = Math.max(2, Math.max(horizontal, vertical) / (512 - gaps));
        const scale = Math.min(viewport.width / (horizontal + (columns - 1) * nativeGap), viewport.height / (vertical + (rows - 1) * nativeGap));
        const cellWidth = reference.width * scale;
        const cellHeight = reference.height * scale;
        const gap = nativeGap * scale;
        const width = columns * cellWidth + (columns - 1) * gap;
        const height = rows * cellHeight + (rows - 1) * gap;
        candidates.push({
            columns,
            cellWidth,
            cellHeight,
            gap,
            width,
            left: (viewport.width - width) / 2,
            top: (viewport.height - height) / 2,
        });
    }
    let best = candidates[0];
    if (!best)
        throw new RangeError('Canvas packed batch has no layout candidate.');
    for (const candidate of candidates.slice(1)) {
        const area = candidate.cellWidth * candidate.cellHeight;
        const bestArea = best.cellWidth * best.cellHeight;
        const empty = Math.ceil(count / candidate.columns) * candidate.columns - count;
        const bestEmpty = Math.ceil(count / best.columns) * best.columns - count;
        if (area > bestArea + 1e-9 || (Math.abs(area - bestArea) < 1e-9 && empty < bestEmpty))
            best = candidate;
    }
    const previous = candidates.find((candidate) => candidate.columns === previousColumns);
    if (previous &&
        best.cellWidth * best.cellHeight < previous.cellWidth * previous.cellHeight * 1.02)
        best = previous;
    return Object.freeze(best);
}
