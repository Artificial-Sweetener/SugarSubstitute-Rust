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
/** Open and save project scripts the way SugarSubstitute did, over ports this owner does not implement. */
import {
  isProjectScript,
  namedScriptPath,
  projectDocument,
  projectNameError,
  projectNameOf,
  projectScriptPath,
  uniqueTabName,
  versionPath,
} from '../domain/ProjectScript.js';

/** One file of the projects folder: its path inside the folder and its file times. */
export interface ProjectFile {
  readonly path: string;
  readonly modified: Date | null;
  readonly created: Date | null;
}

/** The projects folder. */
export interface ProjectFileStore {
  /** Every file of the folder; a folder that does not exist yet holds none. */
  list(): Promise<readonly ProjectFile[]>;
  /** One file's text, or `null` when it does not exist. */
  read(path: string): Promise<string | null>;
  /** Write one file, replacing what is there. */
  write(path: string, text: string): Promise<void>;
  /** Move one file, never replacing another. */
  move(from: string, to: string): Promise<'moved' | 'exists' | 'missing'>;
}

/** The workflow tabs a project is opened into and saved from. */
export interface ProjectWorkflowHost {
  /** The shown workflow's tab name, or `null` when none is shown. */
  activeName(): string | null;
  /** The names of every open workflow tab. */
  openNames(): readonly string[];
  /** The shown workflow as Comfy serializes it. */
  serialize(): unknown;
  /** Refuse a script whose Cubes cannot be built, before a tab is opened for it. */
  checkScript(source: string): Promise<void>;
  /** Show a new empty workflow tab under this name. */
  openEmpty(name: string): Promise<void>;
  /** Build the Cubes a script describes in the shown workflow. */
  importScript(source: string): Promise<void>;
  /** Present the shown workflow as its Cubes, the way a project is worked on. */
  showCubes(): void;
}

/** Turn a workflow into its project script. */
export type ProjectScriptRenderer = (workflow: unknown) => Promise<string>;

/** What a save wrote. */
export interface SavedProject {
  readonly path: string;
  /** Where the copy it replaced was kept, or `null` when it replaced none. */
  readonly kept: string | null;
}

const MOVE_ATTEMPTS = 100;

/**
 * Open, save and save-as project scripts.
 *
 * A project is the folder named after its workflow tab. Saving always writes
 * `<tab name>/<tab name>.sugar` and first keeps the copy it replaces under `versions/`; the
 * script itself carries the tab's name in its header.
 */
export class ProjectScripts {
  constructor(
    private readonly files: ProjectFileStore,
    private readonly workflows: ProjectWorkflowHost,
    private readonly render: ProjectScriptRenderer,
    private readonly now: () => Date = () => new Date(),
  ) {}

  /** List the scripts a person can open, by path, most recently written first. */
  async scripts(): Promise<readonly ProjectFile[]> {
    const scripts = (await this.files.list()).filter((file) => isProjectScript(file.path));
    return [...scripts].sort(
      (left, right) =>
        (right.modified?.getTime() ?? 0) - (left.modified?.getTime() ?? 0) ||
        left.path.localeCompare(right.path),
    );
  }

  /** Open one script into a new tab named for its project; returns that tab's name. */
  async open(path: string): Promise<string> {
    const stored = await this.files.read(path);
    if (stored === null) throw new Error('The project script no longer exists.');
    // A script is stored with Windows line endings; a multi-line value holds plain newlines.
    const text = stored.replace(/\r\n?/g, '\n');
    await this.workflows.checkScript(text);
    const name = uniqueTabName(projectNameOf(text, path), this.workflows.openNames());
    await this.workflows.openEmpty(name);
    await this.workflows.importScript(text);
    this.workflows.showCubes();
    return name;
  }

  /** Save the shown workflow as its project's script. */
  async save(): Promise<SavedProject> {
    const name = this.#name();
    return this.#write(name, projectScriptPath(name));
  }

  /** Save the shown workflow under another file name inside its project's folder. */
  async saveAs(file: string): Promise<SavedProject> {
    const error = projectNameError(file);
    if (error) throw new Error(error);
    const name = this.#name();
    return this.#write(name, namedScriptPath(name, file));
  }

  /** The shown workflow's name, refused when it cannot name a project folder. */
  #name(): string {
    const name = this.workflows.activeName()?.trim() ?? '';
    const error = projectNameError(name);
    if (error) throw new Error(`The workflow's name cannot name a project. ${error}`);
    return name;
  }

  /** Render first, so a workflow that cannot be written leaves the existing script untouched. */
  async #write(name: string, path: string): Promise<SavedProject> {
    const script = await this.render(this.workflows.serialize());
    const existing = (await this.files.list()).find((file) => file.path === path);
    let kept: string | null = null;
    if (existing) {
      // The kept copy is named for when its file was created, as SugarSubstitute named it.
      const written = existing.created ?? existing.modified ?? this.now();
      for (let attempt = 0; attempt < MOVE_ATTEMPTS && kept === null; attempt += 1) {
        const destination = versionPath(path, written, attempt);
        const moved = await this.files.move(path, destination);
        if (moved === 'moved') kept = destination;
        else if (moved === 'missing') break;
      }
      if (kept === null && (await this.files.read(path)) !== null)
        throw new Error('The previous copy of the project script could not be kept.');
    }
    await this.files.write(path, projectDocument(name, script));
    return { path, kept };
  }
}
