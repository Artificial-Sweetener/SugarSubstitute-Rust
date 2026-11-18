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
/** Name and lay out a project's files exactly as SugarSubstitute kept them. */
/** The projects folder inside the Comfy user's own folder: `user/<user>/projects`. */
export const PROJECTS_FOLDER = 'projects';
/** The extension of a project script. */
export const SCRIPT_EXTENSION = '.sugar';
const PROJECT_HEADER = '# Project:';
/**
 * Say why a name cannot name a project or a script, or `null` when it can.
 *
 * A name becomes a folder and a file, so it may not be empty, a relative reference, or contain
 * a path separator, a drive colon or a control character.
 */
export function projectNameError(name) {
    const trimmed = name.trim();
    if (!trimmed || trimmed === '.' || trimmed === '..')
        return 'Enter a name.';
    if (/[/\\:]/.test(trimmed) ||
        Array.from(trimmed).some((character) => character.charCodeAt(0) < 32))
        return 'Use a name without slashes, colons or control characters.';
    return null;
}
/** The script a project saves to: `<name>/<name>.sugar`. */
export function projectScriptPath(name) {
    const folder = name.trim();
    return `${folder}/${folder}${SCRIPT_EXTENSION}`;
}
/** A second script kept in a project's folder under its own file name. */
export function namedScriptPath(project, file) {
    const name = file.trim();
    const leaf = name.toLowerCase().endsWith(SCRIPT_EXTENSION) ? name : name + SCRIPT_EXTENSION;
    return `${project.trim()}/${leaf}`;
}
/**
 * Where the copy a save replaces is kept: `versions/<stem>_<YYYYMMDD_HHMMSS>.sugar` beside the
 * script, with `_<attempt>` appended when that name is taken.
 */
export function versionPath(path, written, attempt = 0) {
    const slash = path.lastIndexOf('/');
    const folder = slash < 0 ? '' : path.slice(0, slash + 1);
    const leaf = path.slice(slash + 1);
    const stem = leaf.toLowerCase().endsWith(SCRIPT_EXTENSION)
        ? leaf.slice(0, -SCRIPT_EXTENSION.length)
        : leaf;
    const two = (value) => String(value).padStart(2, '0');
    const stamp = String(written.getFullYear()) +
        two(written.getMonth() + 1) +
        two(written.getDate()) +
        '_' +
        two(written.getHours()) +
        two(written.getMinutes()) +
        two(written.getSeconds());
    const suffix = attempt > 0 ? `_${String(attempt)}` : '';
    return `${folder}versions/${stem}_${stamp}${suffix}${SCRIPT_EXTENSION}`;
}
/** The file text of a project: its name, a blank line, the script, with Windows line endings. */
export function projectDocument(name, script) {
    const text = `${PROJECT_HEADER} ${name.trim()}\n\n${script.replace(/\r\n/g, '\n')}`;
    return text.replace(/\n/g, '\r\n');
}
/** The name a script gives its project: its last header, or else the file's own name. */
export function projectNameOf(text, path) {
    let named = null;
    for (const line of text.split(/\r?\n/)) {
        const trimmed = line.trim();
        if (trimmed.startsWith(PROJECT_HEADER)) {
            const value = trimmed.slice(PROJECT_HEADER.length).trim();
            if (value)
                named = value;
        }
    }
    if (named)
        return named;
    const leaf = path.slice(path.lastIndexOf('/') + 1);
    return leaf.toLowerCase().endsWith(SCRIPT_EXTENSION)
        ? leaf.slice(0, -SCRIPT_EXTENSION.length)
        : leaf;
}
/** A tab name no open workflow has: the name itself, or the name with ` (2)`, ` (3)`… */
export function uniqueTabName(name, taken) {
    const used = new Set(taken.map((value) => value.toLowerCase()));
    if (!used.has(name.toLowerCase()))
        return name;
    for (let index = 2;; index += 1) {
        const candidate = `${name} (${String(index)})`;
        if (!used.has(candidate.toLowerCase()))
            return candidate;
    }
}
/** Whether a path inside the projects folder is a script a person saved, not a kept version. */
export function isProjectScript(path) {
    return (path.toLowerCase().endsWith(SCRIPT_EXTENSION) &&
        !path.split('/').slice(0, -1).includes('versions'));
}
