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
/** Publish accepted visual style to browser-only surfaces outside the native card lifecycle. */
import { isVisualStyle } from './VisualStyle.js';
const ATTRIBUTE = 'data-sc-visual-style';
const CHANGE_EVENT = 'sugarcubes:visual-style-change';
/** Read validated appearance facts at the browser presentation boundary. */
export function readDocumentVisualStyle(document) {
    const value = document.documentElement.getAttribute(ATTRIBUTE);
    return isVisualStyle(value) ? value : 'comfy';
}
/** Keep the established owned-control appearance when the host's native style is selected. */
export function readOwnedVisualStyle(document) {
    const style = readDocumentVisualStyle(document);
    return style === 'comfy' ? 'fluent' : style;
}
/** Publish only from the appearance owner after validating the host preference. */
export function publishVisualStyle(document, style) {
    if (document.documentElement.getAttribute(ATTRIBUTE) === style)
        return;
    document.documentElement.setAttribute(ATTRIBUTE, style);
    const EventConstructor = document.defaultView?.Event;
    if (EventConstructor)
        document.dispatchEvent(new EventConstructor(CHANGE_EVENT));
}
/** Follow appearance transitions without discovering graph nodes or observing unrelated DOM mutations. */
export function subscribeVisualStyle(document, refresh) {
    document.addEventListener(CHANGE_EVENT, refresh);
    return () => document.removeEventListener(CHANGE_EVENT, refresh);
}
/** Release only the browser marker still owned by the retiring appearance lifetime. */
export function releaseVisualStyle(document, style) {
    if (document.documentElement.getAttribute(ATTRIBUTE) === style)
        document.documentElement.removeAttribute(ATTRIBUTE);
}
