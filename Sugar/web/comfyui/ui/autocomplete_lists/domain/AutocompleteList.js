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
 * Autocomplete lists as SugarSubstitute's Python application kept them: TXT files of one tag per
 * line, either custom (tags added to suggestions) or censored (tags never suggested), each one
 * enabled unless its id is recorded as disabled.
 */
/** The lists folder inside the Comfy user's own folder: `user/<user>/sugar/autocomplete`. */
export const AUTOCOMPLETE_LIST_FOLDER = 'sugar/autocomplete';
/** The id of a list of `kind` named `name`. */
export function autocompleteListId(kind, name) {
    return `${kind}/${name}.txt`;
}
/** Read a path inside the lists folder as a list, or null when it is not one. */
export function autocompleteListAt(path, stamp, disabled) {
    const normalized = path.replaceAll('\\', '/');
    const slash = normalized.indexOf('/');
    const kind = normalized.slice(0, slash);
    const rest = normalized.slice(slash + 1);
    if (slash < 0 || (kind !== 'custom' && kind !== 'censored') || !/\.txt$/iu.test(rest))
        return null;
    const name = rest.slice(0, -4);
    if (listNameRefusal(name) !== null)
        return null;
    const id = autocompleteListId(kind, name);
    return { id, kind, name, enabled: !disabled.has(id), stamp };
}
/** A typed list name as a path: forward slashes, no surrounding space, no `.txt` ending. */
export function autocompleteListName(typed) {
    return typed
        .trim()
        .replaceAll('\\', '/')
        .replace(/\.txt$/iu, '');
}
/** Why a list cannot take this name, or null when it can. */
export function listNameRefusal(name) {
    if (!name || name.startsWith('/'))
        return 'Autocomplete list name must be relative.';
    if (name.length > 512 ||
        name.includes('\0') ||
        name
            .split('/')
            .some((part) => part === '' || part === '.' || part === '..' || part.includes(':')))
        return 'Autocomplete list name contains an unsafe path part.';
    return null;
}
/** A list's tags: every non-blank line, trimmed, in order. */
export function autocompleteListTags(text) {
    return text
        .split(/\r?\n/u)
        .map((line) => line.trim())
        .filter(Boolean);
}
/** The disabled list ids recorded in `lists.json`; a missing or damaged record disables none. */
export function disabledAutocompleteLists(record) {
    if (typeof record !== 'object' || record === null)
        return new Set();
    const disabled = record.disabled;
    return new Set(Array.isArray(disabled) ? disabled.filter((id) => typeof id === 'string') : []);
}
/** `lists.json` for `disabled`, written as Python wrote it. */
export function autocompleteListRecord(disabled) {
    return JSON.stringify({ version: 1, disabled: [...disabled].sort() }, null, 2) + '\n';
}
