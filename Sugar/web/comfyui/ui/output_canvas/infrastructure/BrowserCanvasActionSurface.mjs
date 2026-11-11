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
/** Translate shared surface actions to the browser's camera and image owners. */
/** Retain the existing browser owners without moving their camera or resource state. */
export class BrowserCanvasActionSurface {
    camera;
    resources;
    actions;
    openItem;
    /** Bind the existing per-attachment owners; their lifetimes stay with the attachment. */
    constructor(camera, resources, actions, openItem) {
        this.camera = camera;
        this.resources = resources;
        this.actions = actions;
        this.openItem = openItem;
    }
    /** Preserve the browser camera's own fit transition. */
    fit() {
        return this.camera.fit();
    }
    /** Request one source pixel per physical pixel from the browser camera. */
    native() {
        return this.camera.native();
    }
    /** Apply the browser camera's existing zoom and snapping policy. */
    zoom(factor) {
        return this.camera.zoom(factor);
    }
    /** Translate viewport fractions into the camera's current CSS-pixel geometry. */
    pan(x, y) {
        const target = this.camera.read();
        return target
            ? this.camera.pan({
                x: x * target.geometry.viewport.width,
                y: y * target.geometry.viewport.height,
            })
            : false;
    }
    /** Retry the exact display plan through its resource owner. */
    retry(plan) {
        this.resources.reconcile(plan, true);
        return true;
    }
    /** Ask the resource owner to admit its current large image. */
    admitLargeImage() {
        this.resources.admit();
        return true;
    }
    /** Delegate the exact selected item to the host's existing open action. */
    open(item) {
        this.openItem(item);
        return true;
    }
    /** Keep copy and file actions with the existing browser transfer owner. */
    media(command, scope) {
        return this.actions.run(command, scope);
    }
}
