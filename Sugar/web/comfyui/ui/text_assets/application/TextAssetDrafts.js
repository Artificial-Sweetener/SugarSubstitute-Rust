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
 * The editor's unsaved text, one draft per file. A file is read once, when it is first opened;
 * its edits are kept while other files are opened, until the file is saved or its edits are
 * discarded.
 */
/** Every opened file's text as read and as edited, keyed by file id. */
export class TextAssetDrafts {
    #drafts = new Map();
    /** Whether the file was opened already. */
    has(id) {
        return this.#drafts.has(id);
    }
    /** Begin a draft from the file's text as read. */
    open(id, text) {
        if (!this.#drafts.has(id))
            this.#drafts.set(id, { original: text, current: text });
    }
    /** The draft's text. */
    text(id) {
        return this.#drafts.get(id)?.current ?? '';
    }
    edit(id, text) {
        const draft = this.#drafts.get(id);
        if (draft)
            draft.current = text;
    }
    /** Whether the draft differs from the file. */
    dirty(id) {
        const draft = this.#drafts.get(id);
        return draft !== undefined && draft.current !== draft.original;
    }
    /** The ids of every draft that differs from its file. */
    dirtyIds() {
        return [...this.#drafts.keys()].filter((id) => this.dirty(id));
    }
    /** The draft was written: it is the file's text now. */
    saved(id) {
        const draft = this.#drafts.get(id);
        if (draft)
            draft.original = draft.current;
    }
    /** Put the draft back to the file's text. */
    revert(id) {
        const draft = this.#drafts.get(id);
        if (draft)
            draft.current = draft.original;
    }
    /** The file was renamed: its draft goes with it. */
    rename(from, to) {
        const draft = this.#drafts.get(from);
        if (!draft)
            return;
        this.#drafts.delete(from);
        this.#drafts.set(to, draft);
    }
    /** The file is gone: forget its draft. */
    drop(id) {
        this.#drafts.delete(id);
    }
}
