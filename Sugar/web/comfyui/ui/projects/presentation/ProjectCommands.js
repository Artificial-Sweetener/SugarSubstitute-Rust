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
/** Offer a project's open and save actions as commands, with the same names SugarSubstitute used. */
import { projectNameError } from '../domain/ProjectScript.js';
/** Open a project script from the projects folder. */
export const OPEN_SUGAR_SCRIPT = 'SugarCubes.OpenSugarScript';
/** Save the shown workflow as its project script. */
export const SAVE_SUGAR_SCRIPT = 'SugarCubes.SaveSugarScript';
/** Save the shown workflow under another file name in its project. */
export const SAVE_SUGAR_SCRIPT_AS = 'SugarCubes.SaveSugarScriptAs';
/** Export the shown workflow as a Comfy workflow file. */
export const EXPORT_COMFY_WORKFLOW = 'SugarCubes.ExportComfyWorkflow';
/**
 * Build the four project commands in menu order.
 *
 * Every command reports its own failure, so a menu or a keybinding that runs one never sees it
 * reject.
 */
export function projectCommands(options) {
    const { scripts, dialogs, notify, describe } = options;
    /** Run one action; a failure is told to the person under the action's own title. */
    const guarded = (title, action) => async () => {
        try {
            await action();
        }
        catch (error) {
            notify('error', title, describe(error));
        }
    };
    return [
        {
            id: OPEN_SUGAR_SCRIPT,
            label: 'Open Sugar Script...',
            function: guarded('Sugar script not opened', async () => {
                const available = await scripts.scripts();
                const path = await dialogs.selectItem({
                    title: 'Open Sugar Script',
                    items: available.map((file) => ({
                        value: file.path,
                        label: file.path,
                        ...(file.modified ? { description: file.modified.toLocaleString() } : {}),
                    })),
                    confirmLabel: 'Open',
                    emptyMessage: 'The projects folder holds no Sugar scripts yet.',
                });
                if (path === null)
                    return;
                await scripts.open(path);
            }),
        },
        {
            id: SAVE_SUGAR_SCRIPT,
            label: 'Save Sugar Script',
            function: guarded('Sugar script not saved', async () => {
                const saved = await scripts.save();
                notify('success', 'Sugar script saved', saved.path);
            }),
        },
        {
            id: SAVE_SUGAR_SCRIPT_AS,
            label: 'Save Sugar Script As...',
            function: guarded('Sugar script not saved', async () => {
                const file = await dialogs.promptText({
                    title: 'Save Sugar Script As',
                    label: 'File name',
                    confirmLabel: 'Save',
                    validate: projectNameError,
                });
                if (file === null)
                    return;
                const saved = await scripts.saveAs(file);
                notify('success', 'Sugar script saved', saved.path);
            }),
        },
        {
            id: EXPORT_COMFY_WORKFLOW,
            label: 'Export to Comfy Workflow...',
            function: guarded('Workflow not exported', () => options.exportWorkflow()),
        },
    ];
}
