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
/** Compose the project feature over Comfy's file storage, workflow tabs and SugarCubes' dialogs. */
import { isRecord } from '../types/common.js';
import type { CubeLibraryApi } from '../core/CubeLibraryApi.js';
import { SugarScriptAuthoringApi } from '../sugarscript/SugarScriptAuthoringApi.js';
import type { CubeStackView } from '../cube/chain/CubeStackView.js';
import { SugarScriptCompileError } from '../sugarscript/SugarScriptImportService.js';
import { ProjectScripts } from './application/ProjectScripts.js';
import { ProjectMasks } from './application/ProjectMasks.js';
import {
  ComfyProjectFiles,
  ComfyProjectMaskInputs,
  ComfyProjectWorkflows,
  type ProjectApiFetch,
} from './infrastructure/ComfyProjects.js';
import {
  projectCommands,
  type ProjectCommand,
  type ProjectDialogs,
  type ProjectHostDialog,
} from './presentation/ProjectCommands.js';
import { pickScriptFile } from './presentation/ScriptFilePicker.js';

export interface ProjectCompositionOptions {
  /** Comfy's application object, read as untrusted host state. */
  readonly app: unknown;
  /** Comfy's API object, whose fetch carries the host's addressing and user headers. */
  readonly api: unknown;
  readonly cubeApi: CubeLibraryApi;
  readonly dialogs: ProjectDialogs;
  notify(severity: 'success' | 'error', title: string, message: string): void;
  describe(error: unknown): string;
  importScript(source: string): Promise<unknown>;
  /** Present the shown workflow as its Cubes. */
  showCubes(): void;
  /** The stack the page shows, which a saved script lists its Cubes in. */
  stack(): CubeStackView | null;
  /** Prepare a queue payload's inputs the way a run does: canvas sources captured into files. */
  prepareInputs(payload: unknown): Promise<{ readonly payload: unknown }>;
  /** The host's own file dialog, used while the page runs in a host that shows one. */
  readonly hostDialog?: ProjectHostDialog;
}

/** The backend's own words for a failed read or write, or `fallback` when it gave none. */
function externalFailure(body: unknown, fallback: string): string {
  const error = isRecord(body) ? body.error : null;
  const message = isRecord(error) ? error.message : null;
  return typeof message === 'string' && message ? message : fallback;
}

/** The project feature as the page uses it. */
export interface Projects {
  /** The four project commands, ready to register with Comfy and to list in a menu. */
  readonly commands: readonly ProjectCommand[];
  /** Open a script the person brought to the page, by its file name and text, in its own tab. */
  open(name: string, text: string): Promise<string>;
}

/** Build the project feature over Comfy's services. */
export function createProjects(options: ProjectCompositionOptions): Projects {
  /** Run one Comfy command through the host's own command service. */
  const command = async (id: string): Promise<void> => {
    const app = options.app;
    const manager = isRecord(app) ? app.extensionManager : null;
    const commands = isRecord(manager) ? manager.command : null;
    if (!isRecord(commands) || typeof commands.execute !== 'function')
      throw new Error("Comfy's commands are unavailable.");
    await Reflect.apply(commands.execute, commands, [id]);
  };
  const fetchApi: ProjectApiFetch = (path, init) => {
    const api = options.api;
    if (!isRecord(api) || typeof api.fetchApi !== 'function')
      throw new Error('The Comfy API is unavailable for project files.');
    return (api.fetchApi as ProjectApiFetch).call(api, path, init);
  };
  const files = new ComfyProjectFiles(fetchApi);
  const language = new SugarScriptAuthoringApi(options.cubeApi);
  const scripts = new ProjectScripts(
    files,
    new ComfyProjectWorkflows({
      app: options.app,
      command,
      checkScript: async (source) => {
        const compiled = await language.compile(source);
        if (!compiled.plan) throw new SugarScriptCompileError(compiled.diagnostics);
      },
      importScript: options.importScript,
      showCubes: options.showCubes,
      captureInputs: async (payload) => (await options.prepareInputs(payload)).payload,
    }),
    (workflow) => language.render(workflow, options.stack()),
    new ProjectMasks(files, new ComfyProjectMaskInputs(fetchApi)),
  );
  const commands = projectCommands({
    scripts,
    dialogs: options.dialogs,
    pickScript: () => pickScriptFile(document),
    ...(options.hostDialog ? { hostDialog: options.hostDialog } : {}),
    external: {
      read: async (path) => {
        const response = await fetchApi('/sugarcubes/projects/script/read', {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({ path }),
        });
        const body: unknown = await response.json().catch(() => null);
        const text = isRecord(body) ? body.text : null;
        if (!response.ok || typeof text !== 'string')
          throw new Error(externalFailure(body, 'The Sugar script could not be read.'));
        return text;
      },
      write: async (path, text) => {
        const response = await fetchApi('/sugarcubes/projects/script/write', {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({ path, text }),
        });
        if (!response.ok)
          throw new Error(
            externalFailure(
              await response.json().catch(() => null),
              'The Sugar script could not be written.',
            ),
          );
      },
    },
    projectsFolder: async () => {
      const response = await fetchApi('/sugarcubes/projects/folder');
      const body: unknown = response.ok ? await response.json() : null;
      const folder = isRecord(body) ? body.folder : null;
      if (typeof folder !== 'string' || !folder)
        throw new Error('The projects folder is unavailable.');
      return folder;
    },
    notify: options.notify,
    describe: options.describe,
    exportWorkflow: () => command('Comfy.ExportWorkflow'),
  });
  return { commands, open: (name, text) => scripts.openPicked(name, text) };
}
