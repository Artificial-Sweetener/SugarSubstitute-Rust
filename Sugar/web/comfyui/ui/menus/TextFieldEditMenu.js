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
/** Give ordinary text fields SugarCubes' edit menu, in the browser's own editing engine. */
import { ContextMenu } from './ContextMenu.js';
import { SystemClipboard } from '../core/SystemClipboard.js';
import { HOST_CAPABILITIES } from '../core/embedded/PanelRuntime.js';
const TEXT_INPUT_TYPES = new Set(['text', 'search', 'url', 'tel', 'email', 'password', 'number']);
/** Surfaces SugarCubes draws itself; their fields take its menu even in a plain browser. */
const SUGARCUBES_SURFACES = '[data-sc-style-theme-root], [data-sc-floating-surface], .sugarcubes-modal-overlay, .sc-substitute-frame, .sugarcubes-workflow-masonry, .sc-cube-stack-host, .sugarcubes-settings-row, [id^="SugarCubes."]';
/** The plain text field an event landed in, by element type alone. */
export function plainTextField(target) {
    if (!(target instanceof Element))
        return null;
    const field = target.closest('textarea, input');
    if (field instanceof HTMLTextAreaElement)
        return field;
    if (field instanceof HTMLInputElement && TEXT_INPUT_TYPES.has(field.type))
        return field;
    return null;
}
/**
 * Undo, Redo, Cut, Copy, Paste and Select all for one plain field. Every command runs through the
 * field's own selection and undo history, so Ctrl+Z undoes a menu Paste and the page sees the
 * same input events as when someone types.
 */
export function textFieldEditEntries(field, clipboard, report) {
    const editable = !field.readOnly && !field.disabled;
    const selected = field.selectionStart !== null &&
        field.selectionEnd !== null &&
        field.selectionStart !== field.selectionEnd;
    // Password fields never give their text away.
    const readable = !(field instanceof HTMLInputElement && field.type === 'password');
    const command = (name) => () => {
        field.focus();
        field.ownerDocument.execCommand(name);
    };
    return [
        { label: 'Undo', disabled: !editable, select: command('undo') },
        { label: 'Redo', disabled: !editable, select: command('redo') },
        { label: '', separator: true },
        { label: 'Cut', disabled: !editable || !selected || !readable, select: command('cut') },
        { label: 'Copy', disabled: !selected || !readable, select: command('copy') },
        {
            label: 'Paste',
            disabled: !editable,
            select: () => {
                void clipboard
                    .readText()
                    .then((text) => {
                    field.focus();
                    field.ownerDocument.execCommand('insertText', false, text);
                })
                    .catch(report);
            },
        },
        { label: 'Select all', disabled: !field.value, select: command('selectAll') },
    ];
}
/**
 * Own the right-click menu of plain text fields. In a host that declares
 * `editing.text-menus`, every text field takes it, the host application's own included; anywhere
 * else only fields on SugarCubes' own surfaces do, and the browser keeps every other field.
 * A field's own menu (one that already handled the right-click) is left alone.
 */
export class TextFieldEditMenu {
    document;
    host;
    report;
    #menu;
    #clipboard;
    #active = false;
    /** Bind one document, the host's declaration and the failure boundary. */
    constructor(document, host, report) {
        this.document = document;
        this.host = host;
        this.report = report;
        this.#menu = new ContextMenu(document);
        this.#clipboard = new SystemClipboard(document);
    }
    /** Start answering right-clicks on text fields. */
    setup() {
        if (this.#active)
            return;
        this.#active = true;
        this.document.addEventListener('contextmenu', this.#open);
    }
    /** Stop answering and close an open menu. */
    dispose() {
        this.#active = false;
        this.document.removeEventListener('contextmenu', this.#open);
        this.#menu.close();
    }
    #open = (event) => {
        if (event.defaultPrevented)
            return;
        const field = plainTextField(event.target);
        if (!field)
            return;
        const everywhere = this.host.read()?.declares(HOST_CAPABILITIES.textMenus) === true;
        if (!everywhere && !field.closest(SUGARCUBES_SURFACES))
            return;
        event.preventDefault();
        this.#menu.open({
            label: 'Text actions',
            entries: textFieldEditEntries(field, this.#clipboard, this.report),
            anchor: { kind: 'pointer', x: event.clientX, y: event.clientY },
            trigger: field,
        });
    };
}
