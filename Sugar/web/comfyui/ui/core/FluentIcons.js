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
/** Serve the vendored Microsoft Fluent UI System Icons that product chrome draws. */
/** Every Fluent icon product chrome draws, by name, mapped to its vendored file. */
const FLUENT_ICON_FILES = {
    add: 'seed/add.svg',
    'app-generic': 'node/app-generic.svg',
    'brain-circuit': 'node/brain-circuit.svg',
    color: 'node/color.svg',
    edit: 'node/edit.svg',
    folder: 'node/folder.svg',
    'arrow-clockwise': 'action/arrow-clockwise.svg',
    'arrow-export': 'action/arrow-export.svg',
    broom: 'action/broom.svg',
    code: 'action/code.svg',
    copy: 'action/copy.svg',
    delete: 'action/delete.svg',
    dismiss: 'search/dismiss.svg',
    'folder-open': 'action/folder-open.svg',
    'more-horizontal': 'action/more-horizontal.svg',
    'panel-right': 'action/panel-right.svg',
    play: 'action/play.svg',
    'puzzle-piece': 'action/puzzle-piece.svg',
    save: 'action/save.svg',
    'save-edit': 'action/save-edit.svg',
    settings: 'action/settings.svg',
};
/** The deployed URL of a vendored Fluent icon. */
export function fluentIconUrl(name) {
    return new URL(`../../../icons/${FLUENT_ICON_FILES[name]}`, import.meta.url).href;
}
/** A CSS mask value drawing `name` at `size` CSS pixels in the element's own colour. */
export function fluentIconMask(name, size) {
    return `url("${fluentIconUrl(name)}") center/${String(size)}px ${String(size)}px no-repeat`;
}
/** An inline icon element drawn in the current text colour, hidden from assistive technology. */
export function createFluentIcon(document, name, size = 16) {
    const icon = document.createElement('span');
    icon.dataset.scFluentIcon = name;
    icon.setAttribute('aria-hidden', 'true');
    const mask = fluentIconMask(name, size);
    icon.style.cssText = `display:inline-block;flex:none;width:${String(size)}px;height:${String(size)}px;background:currentColor;`;
    icon.style.setProperty('-webkit-mask', mask);
    icon.style.setProperty('mask', mask);
    return icon;
}
