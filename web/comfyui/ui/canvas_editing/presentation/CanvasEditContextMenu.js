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
/** Offer canvas commands on the secondary button, which the native canvas leaves to the page. */
import { ContextMenu } from '../../menus/ContextMenu.js';
import { CANVAS_EDIT_TEXT } from './CanvasEditText.js';
/**
 * Open the canvas menu over the canvas region while an editable document is shown.
 *
 * Its handler runs on the canvas region before the attachment's own output menu and stops the
 * event there. "Edit layer coverage" is enabled only with an active mask. The menu is not offered
 * while the coverage editor exclusively owns the canvas.
 */
export class CanvasEditContextMenu {
    region;
    target;
    #menu;
    constructor(region, target) {
        this.region = region;
        this.target = target;
        this.#menu = new ContextMenu(region.ownerDocument);
        region.addEventListener('contextmenu', this.#open);
    }
    /** Close any open menu whose state may no longer apply. */
    close() {
        this.#menu.close();
    }
    dispose() {
        this.region.removeEventListener('contextmenu', this.#open);
        this.#menu.close();
    }
    #open = (event) => {
        const target = this.target();
        const snapshot = target?.read();
        if (!target || !snapshot)
            return;
        event.preventDefault();
        event.stopPropagation();
        if (snapshot.surface.kind === 'coverage')
            return;
        this.#menu.open({
            label: CANVAS_EDIT_TEXT.canvasActions,
            anchor: { kind: 'pointer', x: event.clientX, y: event.clientY },
            trigger: this.region,
            entries: [
                {
                    label: CANVAS_EDIT_TEXT.editLayerCoverage,
                    disabled: snapshot.state.activeMask === null,
                    attributes: { 'data-canvas-action': 'edit-layer-coverage' },
                    select: () => target.openCoverage(),
                },
            ],
        });
    };
}
