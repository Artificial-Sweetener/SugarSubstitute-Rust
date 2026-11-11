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
/** Keep native execution feedback in overflow while reserving only the actual toolbar row. */
const HEIGHT = '--sc-output-toolbar-height';
const MARKER = 'sc-output-workspace-topmenu';
/** Own the qualified native menu allocation without moving its Vue-managed controls or feedback. */
export class ComfyCanvasToolbarLayout {
    panel;
    #saved = null;
    /** Retain the graph panel as the boundary for identifying native toolbar mounts. */
    constructor(panel) {
        this.panel = panel;
    }
    /** Find the button row independently of its progress and error siblings. */
    row() {
        const center = this.panel.parentElement?.parentElement;
        return center?.querySelector('.actionbar-container')?.parentElement ?? null;
    }
    /** Reserve the measured toolbar and its native gap; let feedback overflow over the splitter. */
    measure() {
        const row = this.row();
        const candidate = this.panel.parentElement?.parentElement?.firstElementChild;
        const menu = candidate instanceof HTMLElement && row && candidate.contains(row) ? candidate : null;
        if (menu !== this.#saved?.menu) {
            this.dispose();
            if (menu)
                this.#saved = {
                    menu,
                    value: menu.style.getPropertyValue(HEIGHT),
                    priority: menu.style.getPropertyPriority(HEIGHT),
                    marked: menu.classList.contains(MARKER),
                };
        }
        if (!menu || !row)
            return;
        const toolbar = row.getBoundingClientRect();
        if (toolbar.height <= 0)
            return;
        const gap = Number.parseFloat(getComputedStyle(menu).rowGap) || 0;
        const height = `${Math.max(0, toolbar.bottom - menu.getBoundingClientRect().top + gap)}px`;
        menu.classList.add(MARKER);
        if (menu.style.getPropertyValue(HEIGHT) !== height)
            menu.style.setProperty(HEIGHT, height);
    }
    /** Restore exact borrowed menu styles when the host replaces the toolbar or releases the workspace. */
    dispose() {
        const saved = this.#saved;
        if (!saved)
            return;
        if (saved.value)
            saved.menu.style.setProperty(HEIGHT, saved.value, saved.priority);
        else
            saved.menu.style.removeProperty(HEIGHT);
        if (!saved.marked)
            saved.menu.classList.remove(MARKER);
        this.#saved = null;
    }
}
