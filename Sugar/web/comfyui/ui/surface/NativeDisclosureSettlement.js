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
/** Synchronize disclosure measurement with native refresh, Fluent annotation, and widget sizing. */
/** Let native mounting and appearance run, then allow resize-driven widgets to finish sizing. */
export function settleNativeDisclosure(target) {
    return new Promise((resolve) => {
        queueMicrotask(() => {
            const windowRef = target.ownerDocument.defaultView;
            if (!windowRef) {
                resolve();
                return;
            }
            windowRef.setTimeout(() => {
                if (!windowRef.requestAnimationFrame) {
                    resolve();
                    return;
                }
                windowRef.requestAnimationFrame(() => {
                    // ResizeObserver delivers textarea sizing after the appearance frame's callbacks.
                    windowRef.requestAnimationFrame(() => resolve());
                });
            }, 0);
        });
    });
}
