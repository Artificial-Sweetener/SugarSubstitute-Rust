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
/** Mark Comfy workspace chrome for the active Cube editor session. */
export const CUBE_EDITOR_WORKSPACE_CLASS = 'sugarcubes-cube-editor-workspace';
/** Own the document-level mode used to keep SugarCubes chrome off the graph editor. */
export class CubeEditorWorkspaceChromeAdapter {
    #document;
    /** Bind the Comfy document whose workspace hosts the Cube editor. */
    constructor(documentRef) {
        this.#document = documentRef;
    }
    /** Enter Cube editor mode. */
    enter() {
        this.#document.body.classList.add(CUBE_EDITOR_WORKSPACE_CLASS);
    }
    /** Restore the root workflow workspace. */
    leave() {
        this.#document.body.classList.remove(CUBE_EDITOR_WORKSPACE_CLASS);
    }
}
