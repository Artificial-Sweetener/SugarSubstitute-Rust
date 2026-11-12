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
/** Solve measured presentation rectangles from renderer-neutral authored relations. */
const RELATION_EPSILON = 0.001;
/** Preserve authored ordering, alignment, gaps, and intentional overlaps. */
export function solveAuthoredLayout(measurements) {
    const solved = measurements.map((measurement) => ({
        ...measurement,
        solved: {
            x: measurement.authored.x,
            y: measurement.authored.y,
            w: measurement.measured.w,
            h: measurement.measured.h,
        },
    }));
    solveAxis(solved, 'x');
    solveAxis(solved, 'y');
    return solved;
}
/** Return the minimal rectangle containing every input rectangle. */
export function unionLayoutRects(rects) {
    if (!rects.length)
        return null;
    const minX = Math.min(...rects.map(({ x }) => x));
    const minY = Math.min(...rects.map(({ y }) => y));
    const maxX = Math.max(...rects.map(({ x, w }) => x + w));
    const maxY = Math.max(...rects.map(({ y, h }) => y + h));
    return { x: minX, y: minY, w: maxX - minX, h: maxY - minY };
}
function solveAxis(items, axis) {
    const perpendicular = axis === 'x' ? 'y' : 'x';
    const extent = axis === 'x' ? 'w' : 'h';
    const perpendicularExtent = axis === 'x' ? 'h' : 'w';
    const constraints = [];
    for (let left = 0; left < items.length; left += 1) {
        for (let right = left + 1; right < items.length; right += 1) {
            const first = items[left];
            const second = items[right];
            if (!first || !second)
                continue;
            if (!intervalsOverlap(first.authored[perpendicular], first.authored[perpendicularExtent], second.authored[perpendicular], second.authored[perpendicularExtent])) {
                continue;
            }
            const firstEnd = first.authored[axis] + first.authored[extent];
            const secondEnd = second.authored[axis] + second.authored[extent];
            if (firstEnd <= second.authored[axis] + RELATION_EPSILON) {
                constraints.push({
                    before: left,
                    after: right,
                    gap: Math.max(0, second.authored[axis] - firstEnd),
                });
            }
            else if (secondEnd <= first.authored[axis] + RELATION_EPSILON) {
                constraints.push({
                    before: right,
                    after: left,
                    gap: Math.max(0, first.authored[axis] - secondEnd),
                });
            }
        }
    }
    constraints.sort((first, second) => {
        const firstBefore = items[first.before]?.authored[axis] ?? 0;
        const secondBefore = items[second.before]?.authored[axis] ?? 0;
        const firstAfter = items[first.after]?.authored[axis] ?? 0;
        const secondAfter = items[second.after]?.authored[axis] ?? 0;
        return firstBefore - secondBefore || firstAfter - secondAfter;
    });
    let remainingPasses = items.length;
    while (remainingPasses > 0) {
        remainingPasses -= 1;
        let changed = false;
        for (const constraint of constraints) {
            const before = items[constraint.before];
            const after = items[constraint.after];
            if (!before || !after)
                continue;
            const required = before.solved[axis] + before.solved[extent] + constraint.gap;
            if (after.solved[axis] + RELATION_EPSILON < required) {
                after.solved[axis] = required;
                changed = true;
            }
        }
        if (!changed)
            return;
    }
}
function intervalsOverlap(firstStart, firstExtent, secondStart, secondExtent) {
    return (firstStart < secondStart + secondExtent - RELATION_EPSILON &&
        secondStart < firstStart + firstExtent - RELATION_EPSILON);
}
