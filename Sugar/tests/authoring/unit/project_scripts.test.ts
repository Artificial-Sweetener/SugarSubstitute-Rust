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
  maskInputName,
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
import { ProjectMasks } from '../../../frontend/comfyui/ui/projects/application/ProjectMasks.js';
import {
  ComfyProjectFiles,
  ComfyProjectMaskInputs,
  ComfyProjectWorkflows,
} from '../../../frontend/comfyui/ui/projects/infrastructure/ComfyProjects.js';
import { projectCommands } from '../../../frontend/comfyui/ui/projects/presentation/ProjectCommands.js';

const created = new Date(2026, 10, 5, 9, 7, 3);

/** A rendering that names no masks. */
const written = (source: string) => ({ source, maskFiles: [] });

/** Keep project files in memory and record every move and write in order. */
function folder(initial: Readonly<Record<string, string>> = {}) {
  const texts = new Map(Object.entries(initial));
  const bytes = new Map<string, Blob>();
  const log: string[] = [];
  const store: ProjectFileStore = {
    list: async (): Promise<ProjectFile[]> =>
      [...texts.keys(), ...bytes.keys()].map((path) => ({ path, modified: created, created })),
    readBytes: async (path) => bytes.get(path) ?? null,
    writeBytes: async (path, content) => {
      log.push(`write ${path}`);
      bytes.set(path, content);
    },
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
  return { store, texts, bytes, log };
}

/** Stand in for Comfy's tabs: one shown workflow, and a record of what was opened and imported. */
function tabs(active: string | null, open: readonly string[] = active ? [active] : []) {
  const opened: string[] = [];
  const imported: string[] = [];
  const discarded: string[] = [];
  const saved: string[] = [];
  const host: ProjectWorkflowHost = {
    activeName: () => active,
    openNames: () => [...open, ...opened],
    serialize: async () => ({ nodes: [] }),
    checkScript: async (source) => {
      if (source.includes('missing')) throw new Error('Cube not found');
    },
    openEmpty: async (name) => {
      opened.push(name);
    },
    importScript: async (source) => {
      if (source.includes('unbuildable')) throw new Error('Cube boundary is unavailable.');
      imported.push(source);
    },
    discardOpened: async () => {
      discarded.push(opened.pop() ?? '<none>');
    },
    markSaved: () => {
      saved.push(active ?? '<none>');
    },
    showCubes: () => {
      imported.push('<shown as Cubes>');
    },
  };
  return { host, opened, imported, discarded, saved };
}

test('a script whose Cubes cannot be built leaves no tab under the project’s name', async () => {
  const f = folder({ 'Demo/Demo.sugar': '# Project: Demo\r\n\r\nuse "unbuildable" as A\r\n' });
  const t = tabs('Other');
  const scripts = new ProjectScripts(f.store, t.host, async () => written(''));

  await expect(scripts.open('Demo/Demo.sugar')).rejects.toThrow('Cube boundary is unavailable.');

  expect(t.discarded).toEqual(['Demo']);
  expect(t.host.openNames()).toEqual(['Other']);
  expect(f.log).toEqual([]);
});

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
  const scripts = new ProjectScripts(f.store, tabs('Demo').host, async () =>
    written('use "a" as A\n'),
  );

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
  const t = tabs('Demo');
  const scripts = new ProjectScripts(f.store, t.host, async () => written('use "a" as A\n'));

  expect(await scripts.save()).toEqual({ path: 'Demo/Demo.sugar', kept: null });
  expect(await scripts.saveAs('Demo_new')).toEqual({ path: 'Demo/Demo_new.sugar', kept: null });
  // Each write leaves the tab with nothing unsaved, so closing it does not ask.
  expect(t.saved).toEqual(['Demo', 'Demo']);
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
  const unnamed = new ProjectScripts(f.store, tabs('a:b').host, async () => written('x'));
  await expect(unnamed.save()).rejects.toThrow('cannot name a project');
  await expect(
    new ProjectScripts(f.store, tabs(null).host, async () => written('x')).save(),
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
  const scripts = new ProjectScripts(f.store, t.host, async () => written(''));

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
    blob: async () => new Blob([String(body)]),
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
      graphToPrompt: async () => ({ workflow: { nodes: [1] }, output: { '1': {} } }),
    },
    command: async (id) => {
      commands.push(id);
    },
    checkScript: async () => undefined,
    importScript: async () => undefined,
    showCubes: () => undefined,
    captureInputs: async (payload) => ({
      workflow: { captured: (payload as { workflow: unknown }).workflow },
      output: (payload as { output: unknown }).output,
    }),
  });

  expect(host.activeName()).toBe('Unsaved Workflow');
  expect(host.openNames()).toEqual(['Unsaved Workflow', 'Other']);
  // What the input canvas holds is captured into the workflow a save writes.
  expect(await host.serialize()).toEqual({ captured: { nodes: [1] } });
  // Comfy's own tracker takes the workflow as saved: read, reset, then tell the tab.
  const tracked: string[] = [];
  Object.assign(active, {
    changeTracker: {
      checkState: () => tracked.push('checkState'),
      reset: () => tracked.push('reset'),
      updateModified: () => tracked.push('updateModified'),
    },
  });
  host.markSaved();
  expect(tracked).toEqual(['checkState', 'reset', 'updateModified']);
  await host.openEmpty('Bune');
  expect(commands).toEqual(['Comfy.NewBlankWorkflow']);
  expect(renamed).toEqual([[active, 'workflows/Bune.json']]);
});

test('a script chosen from disk opens as its project when the projects folder holds it', async () => {
  const text = '# Project: Bune\r\n\r\nuse "a" as A\r\n';
  const f = folder({ 'Bune/Bune.sugar': text, 'Other/Bune.sugar': '# Project: Other\r\n' });
  const t = tabs('Demo');
  const scripts = new ProjectScripts(f.store, t.host, async () => written(''));

  // The same file name and text as the project's own script: that project.
  expect(await scripts.openPicked('Bune.sugar', text)).toBe('Bune');
  // A file from anywhere else opens under its own header, or else its file name.
  expect(await scripts.openPicked('Loose.sugar', 'use "a" as A\n')).toBe('Loose');
  expect(t.opened).toEqual(['Bune', 'Loose']);
});

test('the four commands keep their names and order and report their own failures', async () => {
  const f = folder({ 'Bune/Bune.sugar': '# Project: Bune\n' });
  const t = tabs('Demo');
  const notices: string[] = [];
  const exported = jest.fn(async () => undefined);
  const commands = projectCommands({
    scripts: new ProjectScripts(f.store, t.host, async () => written('use "a" as A\n')),
    dialogs: { promptText: async () => 'a/b' },
    pickScript: async () => ({ name: 'Bune.sugar', text: '# Project: Bune\n' }),
    projectsFolder: async () => 'C:\\user\\projects',
    external,
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

/** Scripts kept outside the projects folder, by full path, with a record of what was written. */
const outside = new Map<string, string>([
  ['D:\\kept\\Far.sugar', '# Project: Far\nuse "a" as A\n'],
]);
const external = {
  read: async (path: string) => {
    const text = outside.get(path);
    if (text === undefined) throw new Error('The Sugar script no longer exists.');
    return text;
  },
  write: async (path: string, text: string) => void outside.set(path, text),
};

/** A host dialog that records each question and answers from a script. */
function hostDialog(answers: readonly ({ name: string; relative: string; path: string } | null)[]) {
  const asked: string[] = [];
  const queue = [...answers];
  return {
    asked,
    dialog: {
      available: true,
      open: async (folder: string, start: string, extension: string) => {
        asked.push(`open ${folder} [${start}] *.${extension}`);
        return queue.shift() ?? null;
      },
      save: async (folder: string, start: string, name: string, extension: string) => {
        asked.push(`save ${folder} [${start}] ${name} *.${extension}`);
        return queue.shift() ?? null;
      },
    },
  };
}

test('in a host with a file dialog, Open starts in the projects folder and opens what is chosen there', async () => {
  const f = folder({ 'Bune/Bune.sugar': '# Project: Bune\n' });
  const t = tabs('Demo');
  const notices: string[] = [];
  const host = hostDialog([
    {
      name: 'Bune.sugar',
      relative: 'Bune/Bune.sugar',
      path: 'C:\\user\\projects\\Bune\\Bune.sugar',
    },
    null,
    { name: 'Far.sugar', relative: '', path: 'D:\\kept\\Far.sugar' },
  ]);
  const picked = jest.fn(async () => null);
  const [open] = projectCommands({
    scripts: new ProjectScripts(f.store, t.host, async () => written('use "a" as A\n')),
    dialogs: { promptText: async () => null },
    pickScript: picked,
    hostDialog: host.dialog,
    projectsFolder: async () => 'C:\\user\\projects',
    external,
    notify: (severity, title, message) => notices.push(`${severity}: ${title}: ${message}`),
    describe: (error) => (error instanceof Error ? error.message : String(error)),
    exportWorkflow: async () => undefined,
  });
  await open?.function();
  await open?.function();
  await open?.function();
  expect(host.asked).toEqual(Array(3).fill('open C:\\user\\projects [] *.sugar'));
  expect(picked).not.toHaveBeenCalled();
  // A script kept elsewhere is read by its full path and opens as a script of its own.
  expect(t.opened).toEqual(['Bune', 'Far']);
  expect(notices).toEqual([]);
});

test('in a host with a file dialog, Save As asks the operating system where and writes there', async () => {
  const f = folder({ 'Demo/Demo.sugar': '# Project: Demo\n' });
  const t = tabs('Demo');
  const notices: string[] = [];
  const host = hostDialog([
    {
      name: 'take two.sugar',
      relative: 'Demo/take two.sugar',
      path: 'C:\\user\\projects\\Demo\\take two.sugar',
    },
    null,
    { name: 'far.sugar', relative: '', path: 'D:\\kept\\far.sugar' },
    {
      name: 'old.sugar',
      relative: 'Demo/versions/old.sugar',
      path: 'C:\\user\\projects\\Demo\\versions\\old.sugar',
    },
  ]);
  const prompted = jest.fn(async () => null);
  const commands = projectCommands({
    scripts: new ProjectScripts(f.store, t.host, async () => written('use "a" as A\n')),
    dialogs: { promptText: prompted },
    pickScript: async () => null,
    hostDialog: host.dialog,
    projectsFolder: async () => 'C:\\user\\projects',
    external,
    notify: (severity, title, message) => notices.push(`${severity}: ${title}: ${message}`),
    describe: (error) => (error instanceof Error ? error.message : String(error)),
    exportWorkflow: async () => undefined,
  });
  const saveAs = commands[2];
  for (let turn = 0; turn < 4; turn += 1) await saveAs?.function();
  // The dialog starts in the project's own folder, suggesting the project's script, and what
  // is chosen is placed relative to the projects folder.
  expect(host.asked).toEqual(Array(4).fill('save C:\\user\\projects [Demo] Demo.sugar *.sugar'));
  expect(prompted).not.toHaveBeenCalled();
  expect(f.texts.get('Demo/take two.sugar')).toContain('use "a" as A');
  expect(f.log).toEqual(['write Demo/take two.sugar']);
  expect(outside.get('D:\\kept\\far.sugar')).toContain('use "a" as A');
  expect(notices).toEqual([
    'success: Sugar script saved: Demo/take two.sugar',
    'success: Sugar script saved: D:\\kept\\far.sugar',
    'error: Sugar script not saved: Save a Sugar script as a .sugar file outside a versions folder.',
  ]);
});

test('a project never saved has no folder yet, so its save dialog starts in the projects folder', async () => {
  const f = folder();
  const scripts = new ProjectScripts(f.store, tabs('New').host, async () => written('x\n'));
  expect(await scripts.saveAsPlace()).toEqual({ folder: '', file: 'New.sugar' });
  expect(await scripts.saveTo('New/New.sugar')).toEqual({ path: 'New/New.sugar', kept: null });
  expect(await scripts.saveAsPlace()).toEqual({ folder: 'New', file: 'New.sugar' });
  await expect(scripts.saveTo('New/../x.sugar')).rejects.toThrow('Enter a name.');
  await expect(scripts.saveTo('New/x.txt')).rejects.toThrow('.sugar file');
});

test('a project’s masks become loadable when it opens and come home when it is saved', async () => {
  const mask = new Blob(['first']);
  const other = new Blob(['second']);
  const f = folder({
    'Bune/Bune.sugar':
      '# Project: Bune\r\n\r\nset A.L.image = ["m1.png", "gone.png"]\r\n' +
      'set A.P.value = "said \\"m1.png\\" aloud"\r\n',
  });
  f.bytes.set('Bune/masks/m1.png', mask);
  f.bytes.set('Bune/masks/unused.png', other);
  f.bytes.set('Bune/masks/.generation/x/0.png', other);
  const inputs = new Map<string, Blob>();
  // A project's mask goes where the input canvas takes masks from, marked with its project.
  const own = maskInputName('Bune', 'm1.png');
  expect(own).toMatch(/^sugarcubes\/sources\/project-[0-9a-f]{8}-m1\.png$/u);
  expect(maskInputName('Other', 'm1.png')).not.toBe(own);
  const masks = new ProjectMasks(f.store, {
    put: async (name, bytes) => {
      inputs.set(name, bytes);
    },
    get: async (name) => inputs.get(name) ?? null,
  });
  const t = tabs('Bune');
  const scripts = new ProjectScripts(
    f.store,
    t.host,
    async () => ({
      source: `set A.L.image = ["${own}", "sugarcubes/sources/source-1.png", "lost.png"]\n`,
      maskFiles: [
        { file: own, name: 'm1.png' },
        { file: 'sugarcubes/sources/source-1.png', name: 'm1.png' },
        { file: 'lost.png', name: 'A__L-3.png' },
      ],
    }),
    masks,
  );

  await scripts.open('Bune/Bune.sugar');
  // Only a mask the script names is made loadable, and only as a value of its own.
  expect([...inputs.keys()]).toEqual([own]);
  expect(t.imported[0]).toBe(
    `# Project: Bune\n\nset A.L.image = ["${own}", "gone.png"]\n` +
      'set A.P.value = "said \\"m1.png\\" aloud"\n',
  );

  inputs.set('sugarcubes/sources/source-1.png', other);
  await scripts.save();
  // Each mask the host still has is kept beside the script under its bare name; two masks of
  // one name stay two files, and a mask the host no longer has keeps the name it was used by.
  expect(f.bytes.get('Bune/masks/m1.png')).toBe(mask);
  expect(f.bytes.get('Bune/masks/m1__v02.png')).toBe(other);
  expect(f.texts.get('Bune/Bune.sugar')).toBe(
    '# Project: Bune\r\n\r\nset A.L.image = ["m1.png", "m1__v02.png", "lost.png"]\r\n',
  );
});

test('masks travel through Comfy’s input folder and the project folder as bytes', async () => {
  const requests: { path: string; method: string; body: unknown }[] = [];
  const fetchApi = async (path: string, options?: RequestInit) => {
    requests.push({ path, method: options?.method ?? 'GET', body: options?.body ?? null });
    const missing = path.includes('absent');
    return {
      ok: !missing,
      status: missing ? 404 : 200,
      json: async () => null,
      text: async () => '',
      blob: async () => new Blob(['pixels']),
    };
  };
  const inputs = new ComfyProjectMaskInputs(fetchApi);
  const bytes = new Blob(['pixels']);

  await inputs.put('sugarcubes/sources/project-0000abcd-m1.png', bytes);
  const form = requests[0]?.body as FormData;
  expect(requests[0]?.path).toBe('/upload/image');
  expect([form.get('subfolder'), form.get('type'), form.get('overwrite')]).toEqual([
    'sugarcubes/sources',
    'input',
    'true',
  ]);
  expect((form.get('image') as File).name).toBe('project-0000abcd-m1.png');
  expect(await inputs.get('sugarcubes/sources/project-0000abcd-m1.png')).not.toBeNull();
  expect(requests[1]?.path).toBe(
    '/view?filename=project-0000abcd-m1.png&subfolder=sugarcubes%2Fsources&type=input',
  );
  expect(await inputs.get('absent.png')).toBeNull();

  const files = new ComfyProjectFiles(fetchApi);
  await files.writeBytes('Bune/masks/m1.png', bytes);
  expect(requests.at(-1)).toEqual({
    path: '/userdata/projects%2FBune%2Fmasks%2Fm1.png?overwrite=true',
    method: 'POST',
    body: bytes,
  });
  expect(await files.readBytes('Bune/masks/m1.png')).not.toBeNull();
  expect(await files.readBytes('Bune/masks/absent.png')).toBeNull();
});
