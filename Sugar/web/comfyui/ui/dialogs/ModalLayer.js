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
/** Place SugarCubes modals above the host's current dialog layers without changing host elements. */
/** Allocate a valid overlay layer from identified visible dialogs rather than a fixed host-version assumption. */
export function mountModalLayer(document, overlay) {
    const window = document.defaultView;
    let highest = 1999;
    if (window)
        for (const dialog of document.querySelectorAll('[role="dialog"],dialog[open]')) {
            if (overlay.contains(dialog))
                continue;
            const bounds = dialog.getBoundingClientRect();
            const style = window.getComputedStyle(dialog);
            if (!bounds.width ||
                !bounds.height ||
                style.display === 'none' ||
                style.visibility === 'hidden')
                continue;
            for (let element = dialog; element && element !== document.body; element = element.parentElement) {
                const value = Number(window.getComputedStyle(element).zIndex);
                if (Number.isSafeInteger(value) && value >= 0)
                    highest = Math.max(highest, value);
            }
        }
    overlay.style.zIndex = String(Math.min(2147483647, highest + 1));
    document.body.append(overlay);
}
