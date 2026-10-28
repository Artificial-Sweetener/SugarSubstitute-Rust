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
/** Route supported shared output actions without allocating browser image resources. */
/** Advertise camera and admission operations only when the host can perform them. */
export class NativeCanvasActionSurface {
    actions;
    openItem;
    /** Keep transfers with the same qualified owner as the browser surface. */
    constructor(actions, openItem) {
        this.actions = actions;
        this.openItem = openItem;
    }
    /** Camera commands are unavailable until the native host exposes them. */
    fit() {
        return false;
    }
    /** Native scale is unavailable until the native host exposes it. */
    native() {
        return false;
    }
    /** Zoom is unavailable until the native host exposes it. */
    zoom() {
        return false;
    }
    /** Pan is unavailable until the native host exposes it. */
    pan() {
        return false;
    }
    /** Native image retries require a host-owned retry operation. */
    retry() {
        return false;
    }
    /** Native image admission remains with the native resource owner. */
    admitLargeImage() {
        return false;
    }
    /** Open the exact selected output through the host's existing action. */
    open(item) {
        this.openItem(item);
    }
    /** Execute a qualified user-requested transfer. */
    media(command, scope) {
        return this.actions.run(command, scope);
    }
}
