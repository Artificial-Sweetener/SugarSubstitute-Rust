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
/** Reach the user's projects folder and workflow tabs through Comfy's own services. */
import { isRecord, type UnknownRecord } from '../../types/common.js';
import { maskLeaf, PROJECTS_FOLDER } from '../domain/ProjectScript.js';
import type { ProjectMaskInputs } from '../application/ProjectMasks.js';
import type {
  ProjectFile,
  ProjectFileStore,
  ProjectWorkflowHost,
} from '../application/ProjectScripts.js';

/** The part of a fetch response this adapter reads. */
export interface ProjectApiResponse {
  readonly ok?: boolean;
  readonly status?: number;
  json(): Promise<unknown>;
  text(): Promise<string>;
  blob(): Promise<Blob>;
}
export type ProjectApiFetch = (path: string, options?: RequestInit) => Promise<ProjectApiResponse>;

/** List, read, write and move project files through Comfy's per-user file storage. */
export class ComfyProjectFiles implements ProjectFileStore {
  /** Bind the host's configured backend fetch. */
  constructor(private readonly fetchApi: ProjectApiFetch) {}

  async list(): Promise<readonly ProjectFile[]> {
    const response = await this.#request(
      '/userdata?' +
        new URLSearchParams({
          dir: PROJECTS_FOLDER,
          recurse: 'true',
          full_info: 'true',
        }).toString(),
    );
    if (response.status === 404) return [];
    if (!response.ok)
      throw new Error(`The projects folder could not be listed (${String(response.status)}).`);
    const entries: unknown = await response.json();
    if (!Array.isArray(entries)) throw new Error('The projects folder listing is invalid.');
    return entries.flatMap((entry: unknown): ProjectFile[] =>
      isRecord(entry) && typeof entry.path === 'string' && entry.path
        ? [{ path: entry.path, modified: time(entry.modified), created: time(entry.created) }]
        : [],
    );
  }

  async read(path: string): Promise<string | null> {
    const response = await this.#request(location(path));
    if (response.status === 404) return null;
    if (!response.ok)
      throw new Error(`The project file could not be read (${String(response.status)}).`);
    return await response.text();
  }

  async write(path: string, text: string): Promise<void> {
    const response = await this.#request(location(path) + '?overwrite=true', {
      method: 'POST',
      headers: { 'Content-Type': 'text/plain; charset=utf-8' },
      body: text,
    });
    if (!response.ok)
      throw new Error(`The project file could not be written (${String(response.status)}).`);
  }

  async readBytes(path: string): Promise<Blob | null> {
    const response = await this.#request(location(path));
    if (response.status === 404) return null;
    if (!response.ok)
      throw new Error(`The project file could not be read (${String(response.status)}).`);
    return await response.blob();
  }

  async writeBytes(path: string, bytes: Blob): Promise<void> {
    const response = await this.#request(location(path) + '?overwrite=true', {
      method: 'POST',
      headers: { 'Content-Type': 'application/octet-stream' },
      body: bytes,
    });
    if (!response.ok)
      throw new Error(`The project file could not be written (${String(response.status)}).`);
  }

  async move(from: string, to: string): Promise<'moved' | 'exists' | 'missing'> {
    const destination = encodeURIComponent(`${PROJECTS_FOLDER}/${to}`);
    const response = await this.#request(`${location(from)}/move/${destination}?overwrite=false`, {
      method: 'POST',
    });
    if (response.status === 409) return 'exists';
    if (response.status === 404) return 'missing';
    if (!response.ok)
      throw new Error(`The project file could not be moved (${String(response.status)}).`);
    return 'moved';
  }

  /** Bound network IO and release the timeout after the host request settles. */
  async #request(path: string, options: RequestInit = {}): Promise<ProjectApiResponse> {
    const controller = new AbortController();
    const timeout = setTimeout(() => controller.abort(), 30_000);
    try {
      return await this.fetchApi(path, { ...options, signal: controller.signal });
    } finally {
      clearTimeout(timeout);
    }
  }
}

/** Put masks into, and read them from, the input folder Comfy's loaders read. */
export class ComfyProjectMaskInputs implements ProjectMaskInputs {
  /** Bind the host's configured backend fetch. */
  constructor(private readonly fetchApi: ProjectApiFetch) {}

  async put(inputName: string, bytes: Blob): Promise<void> {
    const form = new FormData();
    form.append('image', bytes, maskLeaf(inputName));
    form.append('subfolder', inputName.slice(0, Math.max(0, inputName.lastIndexOf('/'))));
    form.append('type', 'input');
    form.append('overwrite', 'true');
    const response = await this.fetchApi('/upload/image', { method: 'POST', body: form });
    if (!response.ok)
      throw new Error(`A project mask could not be made loadable (${String(response.status)}).`);
  }

  async get(inputName: string): Promise<Blob | null> {
    const response = await this.fetchApi(
      '/view?' +
        new URLSearchParams({
          filename: maskLeaf(inputName),
          subfolder: inputName.slice(0, Math.max(0, inputName.lastIndexOf('/'))),
          type: 'input',
        }).toString(),
    );
    if (response.status === 404 || response.status === 400) return null;
    if (!response.ok)
      throw new Error(`A mask could not be read for the project (${String(response.status)}).`);
    return await response.blob();
  }
}

/** Address one file of the folder; Comfy takes the whole relative path as one encoded segment. */
function location(path: string): string {
  return '/userdata/' + encodeURIComponent(`${PROJECTS_FOLDER}/${path}`);
}

/** Read a file time Comfy reports in milliseconds. */
function time(value: unknown): Date | null {
  return typeof value === 'number' && Number.isFinite(value) && value > 0 ? new Date(value) : null;
}

export interface ComfyProjectWorkflowsOptions {
  /** Comfy's application object, read as untrusted host state. */
  readonly app: unknown;
  /** Run one Comfy command by its identifier. */
  command(id: string): Promise<void>;
  /** Refuse a script whose Cubes cannot be built. */
  checkScript(source: string): Promise<void>;
  /** Build the Cubes a script describes in the shown workflow. */
  importScript(source: string): Promise<unknown>;
  /** Present the shown workflow as its Cubes. */
  showCubes(): void;
  /** Return a queue payload with what its input canvas holds captured into files, as a run does. */
  captureInputs(payload: unknown): Promise<unknown>;
}

/** Read and drive the workflow tabs Comfy owns, for opening a project into one. */
export class ComfyProjectWorkflows implements ProjectWorkflowHost {
  constructor(private readonly options: ComfyProjectWorkflowsOptions) {}

  activeName(): string | null {
    const active = this.#store().activeWorkflow;
    return isRecord(active) && typeof active.filename === 'string' ? active.filename : null;
  }

  openNames(): readonly string[] {
    const open = this.#store().openWorkflows;
    return Array.isArray(open)
      ? open.flatMap((workflow: unknown) =>
          isRecord(workflow) && typeof workflow.filename === 'string' ? [workflow.filename] : [],
        )
      : [];
  }

  /**
   * Read the workflow as a run would send it, so the input canvas can capture what it holds
   * exactly as it does for a run: it reads a loader's settings from the prompt beside the
   * workflow, which the workflow alone does not carry.
   */
  async serialize(): Promise<unknown> {
    const app = this.options.app;
    const read = isRecord(app) ? app.graphToPrompt : null;
    if (typeof read !== 'function') throw new Error('The workflow cannot be read.');
    const payload: unknown = await Reflect.apply(read, app, []);
    const prepared = await this.options.captureInputs(payload);
    const workflow: unknown = isRecord(prepared) ? prepared.workflow : null;
    if (!isRecord(workflow)) throw new Error('The workflow cannot be read.');
    return workflow;
  }

  /** Let Comfy open its own empty tab, then give that tab the project's name. */
  async openEmpty(name: string): Promise<void> {
    await this.options.command('Comfy.NewBlankWorkflow');
    const store = this.#store();
    const workflow = store.activeWorkflow;
    if (!isRecord(workflow) || workflow.isTemporary !== true)
      throw new Error('Comfy did not open a new workflow.');
    const { path, filename, fullFilename } = workflow;
    if (
      typeof path !== 'string' ||
      typeof filename !== 'string' ||
      typeof fullFilename !== 'string' ||
      !fullFilename.startsWith(filename) ||
      typeof store.renameWorkflow !== 'function'
    )
      throw new Error('Comfy workflow renaming is unavailable.');
    if (filename === name) return;
    const next =
      path.slice(0, path.lastIndexOf('/') + 1) + name + fullFilename.slice(filename.length);
    await Reflect.apply(store.renameWorkflow, store, [workflow, next]);
  }

  /** Close the shown tab without asking, while it is still one Comfy never saved. */
  async discardOpened(): Promise<void> {
    const store = this.#store();
    const workflow = store.activeWorkflow;
    if (!isRecord(workflow) || workflow.isTemporary !== true) return;
    if (typeof store.closeWorkflow !== 'function')
      throw new Error('Comfy workflow closing is unavailable.');
    await Reflect.apply(store.closeWorkflow, store, [workflow]);
  }

  /**
   * Take the workflow as it is now for Comfy's saved state, so closing its tab does not ask to
   * save a project that was just written. A later edit makes it unsaved again as usual.
   */
  markSaved(): void {
    const workflow = this.#store().activeWorkflow;
    const tracker = isRecord(workflow) ? workflow.changeTracker : null;
    if (!isRecord(tracker)) return;
    for (const step of ['checkState', 'reset', 'updateModified']) {
      const run = tracker[step];
      if (typeof run === 'function') Reflect.apply(run, tracker, []);
    }
  }

  checkScript(source: string): Promise<void> {
    return this.options.checkScript(source);
  }

  async importScript(source: string): Promise<void> {
    await this.options.importScript(source);
  }

  showCubes(): void {
    this.options.showCubes();
  }

  /** Narrow Comfy's workflow store on every use; it is replaced as Comfy starts. */
  #store(): UnknownRecord {
    const app = this.options.app;
    const manager = isRecord(app) ? app.extensionManager : null;
    const store = isRecord(manager) ? manager.workflow : null;
    if (!isRecord(store)) throw new Error("Comfy's workflows are unavailable.");
    return store;
  }
}
