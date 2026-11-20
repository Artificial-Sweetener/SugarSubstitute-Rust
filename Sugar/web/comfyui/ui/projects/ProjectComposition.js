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
import { SugarScriptAuthoringApi } from '../sugarscript/SugarScriptAuthoringApi.js';
import { SugarScriptCompileError } from '../sugarscript/SugarScriptImportService.js';
import { ProjectScripts } from './application/ProjectScripts.js';
import { ProjectMasks } from './application/ProjectMasks.js';
import { ComfyProjectFiles, ComfyProjectMaskInputs, ComfyProjectWorkflows, } from './infrastructure/ComfyProjects.js';
import { projectCommands, } from './presentation/ProjectCommands.js';
import { pickScriptFile } from './presentation/ScriptFilePicker.js';
/** Build the four project commands, ready to register with Comfy and to list in a menu. */
export function createProjectCommands(options) {
    /** Run one Comfy command through the host's own command service. */
    const command = async (id) => {
        const app = options.app;
        const manager = isRecord(app) ? app.extensionManager : null;
        const commands = isRecord(manager) ? manager.command : null;
        if (!isRecord(commands) || typeof commands.execute !== 'function')
            throw new Error("Comfy's commands are unavailable.");
        await Reflect.apply(commands.execute, commands, [id]);
    };
    const fetchApi = (path, init) => {
        const api = options.api;
        if (!isRecord(api) || typeof api.fetchApi !== 'function')
            throw new Error('The Comfy API is unavailable for project files.');
        return api.fetchApi.call(api, path, init);
    };
    const files = new ComfyProjectFiles(fetchApi);
    const language = new SugarScriptAuthoringApi(options.cubeApi);
    const scripts = new ProjectScripts(files, new ComfyProjectWorkflows({
        app: options.app,
        command,
        checkScript: async (source) => {
            const compiled = await language.compile(source);
            if (!compiled.plan)
                throw new SugarScriptCompileError(compiled.diagnostics);
        },
        importScript: options.importScript,
        showCubes: options.showCubes,
        captureInputs: async (payload) => (await options.prepareInputs(payload)).payload,
    }), (workflow) => language.render(workflow), new ProjectMasks(files, new ComfyProjectMaskInputs(fetchApi)));
    return projectCommands({
        scripts,
        dialogs: options.dialogs,
        pickScript: () => pickScriptFile(document),
        ...(options.hostDialog ? { hostDialog: options.hostDialog } : {}),
        projectsFolder: async () => {
            const response = await fetchApi('/sugarcubes/projects/folder');
            const body = response.ok ? await response.json() : null;
            const folder = isRecord(body) ? body.folder : null;
            if (typeof folder !== 'string' || !folder)
                throw new Error('The projects folder is unavailable.');
            return folder;
        },
        notify: options.notify,
        describe: options.describe,
        exportWorkflow: () => command('Comfy.ExportWorkflow'),
    });
}
