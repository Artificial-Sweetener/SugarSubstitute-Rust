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
import type { ProjectScripts } from '../application/ProjectScripts.js';
import type { PickedScript } from './ScriptFilePicker.js';

/** The dialogs the commands ask the person through. */
export interface ProjectDialogs {
  promptText(options: {
    title?: string;
    label?: string;
    initialValue?: string;
    confirmLabel?: string;
    validate?: (value: string) => string | null;
  }): Promise<string | null>;
}

export interface ProjectCommandOptions {
  readonly scripts: ProjectScripts;
  readonly dialogs: ProjectDialogs;
  /** Let the person choose a script in the operating system's file dialog; `null` when dismissed. */
  pickScript(): Promise<PickedScript | null>;
  /** Tell the person what a command did. */
  notify(severity: 'success' | 'error', title: string, message: string): void;
  /** Describe an unknown failure. */
  describe(error: unknown): string;
  /** Comfy's own export of the shown workflow. */
  exportWorkflow(): Promise<void>;
}

/** One command, as Comfy registers it and as a menu lists it. */
export interface ProjectCommand {
  readonly id: string;
  readonly label: string;
  function(): Promise<void>;
}

/** Open a project script chosen in the operating system's file dialog. */
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
export function projectCommands(options: ProjectCommandOptions): readonly ProjectCommand[] {
  const { scripts, dialogs, notify, describe } = options;
  /** Run one action; a failure is told to the person under the action's own title. */
  const guarded = (title: string, action: () => Promise<void>) => async (): Promise<void> => {
    try {
      await action();
    } catch (error: unknown) {
      notify('error', title, describe(error));
    }
  };
  return [
    {
      id: OPEN_SUGAR_SCRIPT,
      label: 'Open Sugar Script...',
      function: guarded('Sugar script not opened', async () => {
        const picked = await options.pickScript();
        if (picked === null) return;
        await scripts.openPicked(picked.name, picked.text);
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
        if (file === null) return;
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
