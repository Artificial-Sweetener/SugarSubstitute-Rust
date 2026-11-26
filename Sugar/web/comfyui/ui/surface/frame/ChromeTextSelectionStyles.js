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
/**
 * In the SugarSubstitute frame the application reads as a desktop app: the text of its chrome
 * (tabs, buttons, menus, field labels, toolbars, cards) cannot be selected by dragging over it, as
 * in a native window. Text the user edits, or reads as content to copy, stays selectable: fields and
 * editors, the console's output, dialog and modal content, notifications, markdown and code.
 */
/** Surfaces whose text is content rather than chrome. */
export const SELECTABLE_CONTENT = [
    'input',
    'textarea',
    '[contenteditable]:not([contenteditable="false"])',
    '.cm-editor',
    '.sc-console__output',
    '.p-dialog-content',
    '.sugarcubes-modal__panel',
    '.p-toast-message-content',
    '.comfy-markdown',
    'pre',
    'code',
];
/** Make chrome text unselectable in the frame while content keeps ordinary selection. */
export const chromeTextSelectionCss = `
  :root[data-sc-substitute-frame] body { user-select:none; -webkit-user-select:none; }
  :root[data-sc-substitute-frame] :is(${SELECTABLE_CONTENT.join(', ')}) { user-select:text; -webkit-user-select:text; }
`;
