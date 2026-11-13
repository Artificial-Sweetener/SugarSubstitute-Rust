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
/** Preserve native control focus and popup state while presentation owners relocate existing DOM. */
export function relocateControl(element, target, before = null) {
    const focused = element.ownerDocument.activeElement;
    const moveBefore = Reflect.get(target, 'moveBefore');
    if (typeof moveBefore === 'function' && element.isConnected && target.isConnected)
        moveBefore.call(target, element, before);
    else
        target.insertBefore(element, before);
    if (!(focused instanceof HTMLElement) || !element.contains(focused))
        return;
    if (element.ownerDocument.activeElement === focused) {
        if (target.matches(':focus-within'))
            return;
        // Some platform moves retain activeElement without updating focus state on its new ancestors.
        focused.blur();
    }
    focused.focus({ preventScroll: true });
}
