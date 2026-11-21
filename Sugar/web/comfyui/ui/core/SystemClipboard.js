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
/** The page's one way to the system clipboard, for every SugarCubes surface but the prompt editor's own. */
/** Read and write the system clipboard through the browser of one document. */
export class SystemClipboard {
    document;
    /** Bind the document whose browser holds the clipboard. */
    constructor(document) {
        this.document = document;
    }
    /** Copy plain text. */
    async writeText(text) {
        await this.#clipboard().writeText(text);
    }
    /** The clipboard's plain text; the browser may ask the person first. */
    async readText() {
        return this.#clipboard().readText();
    }
    /** Copy one item in several representations, each by its media type. */
    async writeItem(representations) {
        const Item = this.document.defaultView?.ClipboardItem;
        if (typeof Item !== 'function')
            throw new Error('Copying images is unavailable in this browser.');
        await this.#clipboard().write([new Item(representations)]);
    }
    /** Whether the browser can hold `type` in a clipboard item. */
    supports(type) {
        const Item = this.document.defaultView?.ClipboardItem;
        if (typeof Item !== 'function')
            return false;
        return typeof Item.supports === 'function' ? Item.supports(type) : type === 'image/png';
    }
    #clipboard() {
        const clipboard = this.document.defaultView?.navigator.clipboard;
        if (!clipboard)
            throw new Error('The clipboard is unavailable in this browser.');
        return clipboard;
    }
}
