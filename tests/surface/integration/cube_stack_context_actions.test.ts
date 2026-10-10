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
/** Prove complete desktop context actions dispatch through current workflow owners and SugarCubes dialogs. */
import { afterEach, expect, jest, test } from '@jest/globals';
import {
  createCubeStackCommands,
  type CubeStackCommandRuntime,
} from '../../../frontend/comfyui/ui/surface/stack/CubeStackCommands.js';
import { FluentAppearanceState } from '../../../frontend/comfyui/ui/surface/appearance/FluentAppearanceState.js';
import type { CubeNode } from '../../../frontend/comfyui/ui/cube/node/ComfyCubeNodeFactory.js';
import type { NativeCubeSubgraph } from '../../../frontend/comfyui/ui/cube/ComfyCubeGraphBuilder.js';

const close: (() => void)[] = [];
afterEach(() => {
  for (const dispose of close.splice(0)) dispose();
  document.body.replaceChildren();
  jest.restoreAllMocks();
});

test('matches Substitute action order, includes eligible capture, and adds ordering commands', () => {
  const f = fixture();
  f.open();
  expect(labels()).toEqual([
    "Don't save outputs",
    'Rename',
    'Duplicate',
    'Capture Cube',
    'Bypass',
    'Remove',
    'Move up',
    'Move down',
    'Edit stack',
  ]);
  expect(document.querySelector('[data-sc-cube-stack-menu]')?.getAttribute('data-appearance')).toBe(
    'comfy',
  );
  f.node.mode = 4;
  f.node.properties.sugarcubes_save_outputs = false;
  f.setCapture(false);
  f.appearance.update(true, true);
  f.open();
  expect(labels()).toEqual([
    'Save outputs',
    'Rename',
    'Duplicate',
    'Remove bypass',
    'Remove',
    'Move up',
    'Move down',
    'Edit stack',
  ]);
  expect(document.querySelector('[data-sc-cube-stack-menu]')?.getAttribute('data-appearance')).toBe(
    'fluent',
  );
});

test.each(['Duplicate', 'Bypass', "Don't save outputs", 'Remove', 'Capture Cube'])(
  'dispatches %s to its authoritative owner without opening the cart',
  async (label) => {
    const f = fixture();
    f.open();
    select(label);
    await settle();
    const call =
      label === 'Duplicate'
        ? f.actions.duplicate
        : label === 'Bypass'
          ? f.actions.toggleBypass
          : label === "Don't save outputs"
            ? f.actions.toggleOutputPersistence
            : label === 'Remove'
              ? f.actions.remove
              : f.capture;
    expect(call).toHaveBeenCalledWith('instance');
    expect(f.cart).not.toHaveBeenCalled();
    expect(document.querySelector('[data-sc-cube-stack-menu]')).toBeNull();
  },
);

test('renames through SugarCubes input and rejects a workflow replacement while awaiting input', async () => {
  const f = fixture();
  let resolve: (value: string | null) => void = () => undefined;
  f.prompt.mockImplementation(
    () =>
      new Promise((result) => {
        resolve = result;
      }),
  );
  f.open();
  select('Rename');
  await settle();
  expect(f.prompt).toHaveBeenCalledWith(
    expect.objectContaining({ title: 'Rename Cube', initialValue: 'Demo' }),
  );
  f.retire();
  resolve('new name');
  await settle();
  expect(f.actions.rename).not.toHaveBeenCalled();
  expect(f.error).toHaveBeenCalledWith(expect.any(Error));
});

test('accepts a rename and cancellation without invoking browser-native dialogs', async () => {
  const f = fixture();
  const native = jest.spyOn(window, 'prompt').mockImplementation(() => {
    throw new Error('native prompt');
  });
  f.prompt.mockResolvedValue('New label');
  f.open();
  select('Rename');
  await settle();
  expect(f.actions.rename).toHaveBeenCalledWith('instance', 'New label');
  f.prompt.mockResolvedValue(null);
  f.open();
  select('Rename');
  await settle();
  expect(f.actions.rename).toHaveBeenCalledTimes(1);
  expect(native).not.toHaveBeenCalled();
});

test('disables mutation intent during pending canonical operations and reports failures', async () => {
  const f = fixture();
  f.runtime.reorder.pending = true;
  f.open();
  select('Remove');
  await settle();
  expect(f.actions.remove).not.toHaveBeenCalled();
  f.runtime.reorder.pending = false;
  f.actions.remove.mockImplementation(() => {
    throw new Error('host removal failed');
  });
  f.open();
  select('Remove');
  await settle();
  expect(f.error).toHaveBeenCalledWith(expect.objectContaining({ message: 'host removal failed' }));
});

/** Read public accessible rows rather than private menu owner state. */
function labels(): string[] {
  return [...document.querySelectorAll('[role="menuitem"]')].map((row) => row.textContent ?? '');
}
/** Activate one accessible menu entry using its public click contract. */
function select(label: string): void {
  const row = [...document.querySelectorAll<HTMLElement>('[role="menuitem"]')].find(
    (value) => value.textContent === label,
  );
  if (!row) throw new Error(`Missing menu entry ${label}`);
  row.click();
}
/** Settle dispatch and a resolved modal boundary without relying on wall-clock delays. */
async function settle(): Promise<void> {
  for (let index = 0; index < 5; index++) await Promise.resolve();
}
/** Isolate external graph and dialog ports while rendering the real official Fluent menu. */
function fixture() {
  const subgraph: NativeCubeSubgraph = {
    id: 'definition',
    name: 'Definition',
    _nodes: [],
    inputs: [],
    outputs: [],
    inputNode: {},
    outputNode: {},
    add() {},
    remove() {},
    configure() {},
    addInput() {
      return { connect() {} };
    },
    addOutput() {
      return { connect() {} };
    },
  };
  const node: CubeNode = {
    id: 'native',
    title: 'Demo',
    mode: 0,
    pos: [0, 0],
    size: [200, 300],
    inputs: [],
    outputs: [],
    properties: {},
    subgraph,
    isSubgraphNode: () => true,
    serialize: () => ({}),
    connect() {},
  };
  const actions = {
    require: jest.fn(() => node),
    rename: jest.fn<(id: string, name: string) => void>(),
    duplicate: jest.fn(() => node),
    remove: jest.fn<(id: string) => void>(),
    toggleBypass: jest.fn<(id: string) => void>(),
    toggleOutputPersistence: jest.fn<(id: string) => void>(),
  };
  const reorder = {
    analysis: null,
    refresh: async () => null,
    pending: false,
    subscribe: () => () => undefined,
    move: async () => undefined,
    available: true,
    reorder: async () => undefined,
    canMove: () => false,
  };
  const runtime = { nodes: { get: () => node }, instanceActions: actions, reorder };
  let current: CubeStackCommandRuntime | null = runtime;
  let eligible = true;
  const appearance = new FluentAppearanceState();
  const prompt = jest.fn<() => Promise<string | null>>().mockResolvedValue(null);
  const capture = jest.fn<(id: string) => Promise<void>>().mockResolvedValue(undefined);
  const cart = jest.fn();
  const error = jest.fn();
  const commands = createCubeStackCommands({
    document,
    appearance,
    picker: { openCart: cart },
    getRuntime: () => current,
    dialogs: { promptText: prompt },
    library: {
      capture,
      classification: () => ({
        definitionId: 'definition',
        cubeId: 'demo.cube',
        cubeVersion: '1',
        semanticHash: 'hash',
        instanceIds: ['instance'],
        primaryClass: 'none',
        access: 'read_only',
        sourceAvailable: false,
        permittedOperations: new Set(eligible ? ['capture'] : []),
      }),
    },
    resolveSource: () => null,
    reportError: error,
  });
  close.push(commands.closeTransient);
  return {
    node,
    actions,
    appearance,
    prompt,
    capture,
    cart,
    error,
    runtime,
    open: () => commands.contextMenu('instance', new MouseEvent('contextmenu')),
    retire: () => {
      current = null;
    },
    setCapture: (value: boolean) => {
      eligible = value;
    },
  };
}
