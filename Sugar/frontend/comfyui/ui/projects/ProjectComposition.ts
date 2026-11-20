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
  /** Prepare a queue payload's inputs the way a run does: canvas sources captured into files. */
  prepareInputs(payload: unknown): Promise<{ readonly payload: unknown }>;
}

/** Build the four project commands, ready to register with Comfy and to list in a menu. */
export function createProjectCommands(
  options: ProjectCompositionOptions,
): readonly ProjectCommand[] {
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
    (workflow) => language.render(workflow),
    new ProjectMasks(files, new ComfyProjectMaskInputs(fetchApi)),
  );
  return projectCommands({
    scripts,
    dialogs: options.dialogs,
    pickScript: () => pickScriptFile(document),
    notify: options.notify,
    describe: options.describe,
    exportWorkflow: () => command('Comfy.ExportWorkflow'),
  });
}
