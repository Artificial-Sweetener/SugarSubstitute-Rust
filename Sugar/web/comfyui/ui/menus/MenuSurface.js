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
/** Identify action and host-owned choice menus for one shared appearance owner. */
/** Mark a menu without taking its existing keyboard, selection, or dismissal owner. */
export function identifyMenuSurface(element, kind) {
    element.dataset.scMenuSurface = kind;
}
/** Release the borrowed host menu when its appearance scope ends. */
export function releaseMenuSurface(element) {
    delete element.dataset.scMenuSurface;
}
