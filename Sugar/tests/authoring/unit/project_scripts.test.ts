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
/** Prove projects are opened and saved with the files, names and backups SugarSubstitute used. */
import { expect, jest, test } from '@jest/globals';
import {
  isProjectScript,
  namedScriptPath,
  projectDocument,
  projectNameError,
  projectNameOf,
  projectScriptPath,
  uniqueTabName,
  versionPath,
} from '../../../frontend/comfyui/ui/projects/domain/ProjectScript.js';
import {
  ProjectScripts,
  type ProjectFile,
  type ProjectFileStore,
  type ProjectWorkflowHost,
} from '../../../frontend/comfyui/ui/projects/application/ProjectScripts.js';
import {
  ComfyProjectFiles,
  ComfyProjectWorkflows,
} from '../../../frontend/comfyui/ui/projects/infrastructure/ComfyProjects.js';
import { projectCommands } from '../../../frontend/comfyui/ui/projects/presentation/ProjectCommands.js';

const created = new Date(2026, 10, 5, 9, 7, 3);

/** Keep project files in memory and record every move and write in order. */
function folder(initial: Readonly<Record<string, string>> = {}) {
  const texts = new Map(Object.entries(initial));
  const log: string[] = [];
  const store: ProjectFileStore = {
    list: async (): Promise<ProjectFile[]> =>
      [...texts.keys()].map((path) => ({ path, modified: created, created })),
    read: async (path) => texts.get(path) ?? null,
    write: async (path, text) => {
      log.push(`write ${path}`);
      texts.set(path, text);
    },
    move: async (from, to) => {
      const text = texts.get(from);
      if (text === undefined) return 'missing';
      if (texts.has(to)) return 'exists';
      log.push(`move ${from} -> ${to}`);
      texts.delete(from);
      texts.set(to, text);
      return 'moved';
    },
  };
  return { store, texts, log };
}

/** Stand in for Comfy's tabs: one shown workflow, and a record of what was opened and imported. */
function tabs(active: string | null, open: readonly string[] = active ? [active] : []) {
  const opened: string[] = [];
  const imported: string[] = [];
  const host: ProjectWorkflowHost = {
    activeName: () => active,
    openNames: () => [...open, ...opened],
    serialize: () => ({ nodes: [] }),
    checkScript: async (source) => {
      if (source.includes('missing')) throw new Error('Cube not found');
    },
    openEmpty: async (name) => {
      opened.push(name);
    },
    importScript: async (source) => {
      imported.push(source);
    },
    showCubes: () => {
      imported.push('<shown as Cubes>');
    },
  };
  return { host, opened, imported };
}

test('names, paths and file text follow the persisted project layout', () => {
  expect(projectScriptPath('Portrait Study')).toBe('Portrait Study/Portrait Study.sugar');
  expect(namedScriptPath('Portrait Study', 'second')).toBe('Portrait Study/second.sugar');
  expect(namedScriptPath('Portrait Study', 'second.SUGAR')).toBe('Portrait Study/second.SUGAR');
  expect(versionPath('P/P.sugar', created)).toBe('P/versions/P_20261105_090703.sugar');
  expect(versionPath('P/P.sugar', created, 2)).toBe('P/versions/P_20261105_090703_2.sugar');
  expect(projectDocument(' Demo ', 'use "a" as A\n\nset A.n.v = """x\ny"""\n')).toBe(
    '# Project: Demo\r\n\r\nuse "a" as A\r\n\r\nset A.n.v = """x\r\ny"""\r\n',
  );
  // Text that already has Windows line endings is not doubled.
  expect(projectDocument('Demo', 'a\r\nb\n')).toBe('# Project: Demo\r\n\r\na\r\nb\r\n');
});

test('a project is named by its last header, or else by its file', () => {
  expect(
    projectNameOf('# Project: First\r\n\r\n# Project: Last\r\nuse "a" as A\r\n', 'x/y.sugar'),
  ).toBe('Last');
  expect(projectNameOf('use "a" as A\n', 'Folder/Loose Script.sugar')).toBe('Loose Script');
  expect(projectNameOf('# Project:   \n', 'Root.sugar')).toBe('Root');
});

test('names that cannot be a folder are refused and taken tab names are numbered', () => {
  for (const name of ['', '  ', '.', '..', 'a/b', 'a\\b', 'C:x', 'tab\there'])
    expect(projectNameError(name)).not.toBeNull();
  for (const name of ['Portrait Study', 'tizibade (2) (3)', '20_02 x', 'café'])
    expect(projectNameError(name)).toBeNull();
  expect(uniqueTabName('Bune', ['Other'])).toBe('Bune');
  expect(uniqueTabName('Bune', ['bune', 'Bune (2)'])).toBe('Bune (3)');
  expect(isProjectScript('P/P.sugar')).toBe(true);
  expect(isProjectScript('Loose.sugar')).toBe(true);
  expect(isProjectScript('P/versions/P_20261105_090703.sugar')).toBe(false);
  expect(isProjectScript('P/masks/m.png')).toBe(false);
});

test('saving writes the tab’s own script and first keeps the copy it replaces', async () => {
  const f = folder({
    'Demo/Demo.sugar': 'old',
    'Demo/versions/Demo_20261105_090703.sugar': 'older',
  });
  const scripts = new ProjectScripts(f.store, tabs('Demo').host, async () => 'use "a" as A\n');

  const saved = await scripts.save();

  expect(saved).toEqual({
    path: 'Demo/Demo.sugar',
    kept: 'Demo/versions/Demo_20261105_090703_1.sugar',
  });
  expect(f.log).toEqual([
    'move Demo/Demo.sugar -> Demo/versions/Demo_20261105_090703_1.sugar',
    'write Demo/Demo.sugar',
  ]);
  expect(f.texts.get('Demo/versions/Demo_20261105_090703_1.sugar')).toBe('old');
  expect(f.texts.get('Demo/Demo.sugar')).toBe('# Project: Demo\r\n\r\nuse "a" as A\r\n');
});

test('a first save keeps nothing, and save-as writes beside the project’s own script', async () => {
  const f = folder();
  const scripts = new ProjectScripts(f.store, tabs('Demo').host, async () => 'use "a" as A\n');

  expect(await scripts.save()).toEqual({ path: 'Demo/Demo.sugar', kept: null });
  expect(await scripts.saveAs('Demo_new')).toEqual({ path: 'Demo/Demo_new.sugar', kept: null });
  // The header names the tab, not the file.
  expect(f.texts.get('Demo/Demo_new.sugar')).toBe('# Project: Demo\r\n\r\nuse "a" as A\r\n');
  await expect(scripts.saveAs('a/b')).rejects.toThrow('without slashes');
});

test('a workflow that cannot be written leaves the existing script untouched', async () => {
  const f = folder({ 'Demo/Demo.sugar': 'old' });
  const failing = new ProjectScripts(f.store, tabs('Demo').host, async () => {
    throw new Error('Two Cubes are named the same.');
  });
  await expect(failing.save()).rejects.toThrow('named the same');
  expect(f.log).toEqual([]);
  const unnamed = new ProjectScripts(f.store, tabs('a:b').host, async () => 'x');
  await expect(unnamed.save()).rejects.toThrow('cannot name a project');
  await expect(
    new ProjectScripts(f.store, tabs(null).host, async () => 'x').save(),
  ).rejects.toThrow('cannot name a project');
});

test('opening builds the script in a new tab named for its project, numbered when taken', async () => {
  const text = '# Project: Bune\r\n\r\nuse "a" as A\r\nset A.n.v = """x\r\ny"""\r\n';
  const f = folder({
    'Bune/Bune.sugar': text,
    'Bune/versions/Bune_1.sugar': 'x',
    'Loose.sugar': 'y',
    'Broken/Broken.sugar': 'use "missing" as A\n',
  });
  const t = tabs('Other', ['Other', 'Bune']);
  const scripts = new ProjectScripts(f.store, t.host, async () => '');

  expect((await scripts.scripts()).map((file) => file.path)).toEqual([
    'Broken/Broken.sugar',
    'Bune/Bune.sugar',
    'Loose.sugar',
  ]);
  expect(await scripts.open('Bune/Bune.sugar')).toBe('Bune (2)');
  expect(t.opened).toEqual(['Bune (2)']);
  // A stored script has Windows line endings; the values it is built from hold plain newlines.
  expect(t.imported).toEqual([
    '# Project: Bune\n\nuse "a" as A\nset A.n.v = """x\ny"""\n',
    '<shown as Cubes>',
  ]);
  // A script that cannot be built opens no tab.
  await expect(scripts.open('Broken/Broken.sugar')).rejects.toThrow('Cube not found');
  expect(t.opened).toEqual(['Bune (2)']);
  await expect(scripts.open('Gone/Gone.sugar')).rejects.toThrow('no longer exists');
});

test('project files travel through Comfy’s per-user file routes', async () => {
  const requests: { path: string; method: string; body: unknown }[] = [];
  const reply = (status: number, body: unknown = null) => ({
    ok: status >= 200 && status < 300,
    status,
    json: async () => body,
    text: async () => String(body),
  });
  const files = new ComfyProjectFiles(async (path, options) => {
    requests.push({ path, method: options?.method ?? 'GET', body: options?.body ?? null });
    if (path.startsWith('/userdata?'))
      return reply(200, [
        { path: 'Demo/Demo.sugar', size: 3, modified: 1000, created: 500 },
        { path: 5 },
      ]);
    if (path.includes('/move/')) return reply(409);
    if (options?.method === 'POST') return reply(200);
    return reply(404);
  });

  expect(await files.list()).toEqual([
    { path: 'Demo/Demo.sugar', modified: new Date(1000), created: new Date(500) },
  ]);
  expect(await files.read('Demo/Demo.sugar')).toBeNull();
  await files.write('Demo/Demo.sugar', 'a\r\nb');
  expect(await files.move('Demo/Demo.sugar', 'Demo/versions/Demo_1.sugar')).toBe('exists');
  expect(requests.map((request) => `${request.method} ${request.path}`)).toEqual([
    'GET /userdata?dir=projects&recurse=true&full_info=true',
    'GET /userdata/projects%2FDemo%2FDemo.sugar',
    'POST /userdata/projects%2FDemo%2FDemo.sugar?overwrite=true',
    'POST /userdata/projects%2FDemo%2FDemo.sugar/move/projects%2FDemo%2Fversions%2FDemo_1.sugar?overwrite=false',
  ]);
  expect(requests[2]?.body).toBe('a\r\nb');
});

test('a project opens into Comfy’s own new tab, renamed to the project', async () => {
  const active = {
    isTemporary: true,
    path: 'workflows/Unsaved Workflow.json',
    filename: 'Unsaved Workflow',
    fullFilename: 'Unsaved Workflow.json',
  };
  const renamed: unknown[] = [];
  const store = {
    activeWorkflow: active,
    openWorkflows: [active, { filename: 'Other' }],
    renameWorkflow: async (workflow: unknown, path: string) => {
      renamed.push([workflow, path]);
    },
  };
  const commands: string[] = [];
  const host = new ComfyProjectWorkflows({
    app: {
      extensionManager: { workflow: store },
      rootGraph: { serialize: () => ({ nodes: [1] }) },
    },
    command: async (id) => {
      commands.push(id);
    },
    checkScript: async () => undefined,
    importScript: async () => undefined,
    showCubes: () => undefined,
  });

  expect(host.activeName()).toBe('Unsaved Workflow');
  expect(host.openNames()).toEqual(['Unsaved Workflow', 'Other']);
  expect(host.serialize()).toEqual({ nodes: [1] });
  await host.openEmpty('Bune');
  expect(commands).toEqual(['Comfy.NewBlankWorkflow']);
  expect(renamed).toEqual([[active, 'workflows/Bune.json']]);
});

test('the four commands keep their names and order and report their own failures', async () => {
  const f = folder({ 'Bune/Bune.sugar': '# Project: Bune\n' });
  const t = tabs('Demo');
  const notices: string[] = [];
  const exported = jest.fn(async () => undefined);
  const commands = projectCommands({
    scripts: new ProjectScripts(f.store, t.host, async () => 'use "a" as A\n'),
    dialogs: {
      selectItem: async (options) => options.items?.[0]?.value ?? null,
      promptText: async () => 'a/b',
    },
    notify: (severity, title, message) => notices.push(`${severity}: ${title}: ${message}`),
    describe: (error) => (error instanceof Error ? error.message : String(error)),
    exportWorkflow: exported,
  });

  expect(commands.map((command) => command.label)).toEqual([
    'Open Sugar Script...',
    'Save Sugar Script',
    'Save Sugar Script As...',
    'Export to Comfy Workflow...',
  ]);
  for (const command of commands) await command.function();
  expect(t.opened).toEqual(['Bune']);
  expect(exported).toHaveBeenCalledTimes(1);
  expect(notices).toEqual([
    'success: Sugar script saved: Demo/Demo.sugar',
    'error: Sugar script not saved: Use a name without slashes, colons or control characters.',
  ]);
});
