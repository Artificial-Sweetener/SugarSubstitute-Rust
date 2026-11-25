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
 * Painters on the graph's front canvas after LiteGraph has drawn everything that lies beneath the
 * node cards: the grid, groups, links, reroutes and the proximity noodles, composited from its
 * background canvas. LiteGraph has one foreground callback per canvas and the overlay draw hook
 * holds it, so every other owner paints through this registry, which that hook runs before the
 * Cube chrome, and none chains the callback itself.
 */
const painters = new WeakMap();
/** Add a painter for one canvas; the returned function removes it. */
export function addGraphForegroundPainter(canvas, painter) {
    const registered = painters.get(canvas) ?? new Set();
    painters.set(canvas, registered);
    registered.add(painter);
    return () => {
        registered.delete(painter);
        if (registered.size === 0)
            painters.delete(canvas);
    };
}
/** Run a canvas's painters; one that throws is reported and the rest still paint. */
export function paintGraphForeground(canvas, context, report) {
    for (const painter of painters.get(canvas) ?? []) {
        try {
            painter(context, canvas);
        }
        catch (error) {
            report(error);
        }
    }
}
