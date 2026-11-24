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
/** Offer the wildcard folder's file actions in SugarCubes' settings. */
import { settingsCatalogFor } from '../../settings/workspace/SettingsCatalog.js';
/**
 * Contribute Prompt Editing's Manage wildcards row: open the folder, create a wildcard, rename a
 * wildcard. Its actions read the folder as it is when they run.
 *
 * A name is written as a prompt writes it, so `people/hair` creates `people/hair.txt`; ending the
 * name in `.csv` creates a CSV wildcard. Names that are taken or are not wildcard files are
 * refused in the dialog, before anything is written.
 */
export function registerWildcardSettings(options) {
    settingsCatalogFor(options.app).contribute('prompt_editing.wildcards.manage', {
        controls: [
            { kind: 'button', button: { label: 'Open folder', run: () => options.openFolder() } },
            { kind: 'button', button: { label: 'New…', run: () => createWildcard(options) } },
            { kind: 'button', button: { label: 'Rename…', run: () => renameWildcard(options) } },
        ],
    });
}
/** Turn a typed name into a file path: a TXT wildcard unless the name ends in `.csv` or `.txt`. */
function pathOf(name, extension = '.txt') {
    const trimmed = name.trim().replaceAll('\\', '/');
    return /\.(?:txt|csv)$/iu.test(trimmed) ? trimmed : trimmed + extension;
}
/** Find the file a typed name refers to: its path, or its name as a prompt writes it. */
function named(files, name) {
    const typed = name.trim().replaceAll('\\', '/').toLowerCase();
    return (files.find((file) => file.path.toLowerCase() === typed) ??
        files.find((file) => file.identifier === typed) ??
        null);
}
/** Why a new file cannot take this name, or null when it can. */
function refusal(options, path) {
    const file = options.admit(path);
    if (!file)
        return 'Use letters, numbers and folders, such as colors or people/hair.';
    return options
        .files()
        .some((existing) => existing.kind === file.kind && existing.identifier === file.identifier)
        ? 'A wildcard with this name already exists.'
        : null;
}
/** Ask for a name and create an empty wildcard file with it. */
async function createWildcard(options) {
    await options.refresh();
    const name = await options.prompt({
        title: 'New wildcard',
        label: 'Name',
        helperText: 'As a prompt will write it: colors, or people/hair. End it in .csv for a CSV wildcard.',
        confirmLabel: 'Create',
        validate: (value) => refusal(options, pathOf(value)),
    });
    if (name === null)
        return;
    if ((await options.create(pathOf(name), '')) === 'exists')
        throw new Error('A wildcard with this name already exists.');
    await options.refresh();
}
/** Ask which wildcard to rename and what to call it, then rename its file. */
async function renameWildcard(options) {
    await options.refresh();
    const current = await options.prompt({
        title: 'Rename wildcard',
        label: 'Wildcard to rename',
        helperText: 'The name a prompt uses for it now, such as colors or people/hair.',
        confirmLabel: 'Next',
        validate: (value) => named(options.files(), value) ? null : 'There is no wildcard with this name.',
    });
    const file = current === null ? null : named(options.files(), current);
    if (!file)
        return;
    const extension = file.path.slice(file.path.lastIndexOf('.'));
    const name = await options.prompt({
        title: 'Rename wildcard',
        label: `New name for ${file.identifier}`,
        helperText: 'Prompts that use the old name must be changed to the new one.',
        confirmLabel: 'Rename',
        initialValue: file.path.slice(0, file.path.length - extension.length),
        validate: (value) => refusal(options, pathOf(value, extension)),
    });
    if (name === null)
        return;
    const outcome = await options.rename(file.path, pathOf(name, extension));
    if (outcome === 'exists')
        throw new Error('A wildcard with this name already exists.');
    if (outcome === 'missing')
        throw new Error('That wildcard is no longer in the folder.');
    await options.refresh();
}
