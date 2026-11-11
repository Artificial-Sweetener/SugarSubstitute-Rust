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
/** Prove all six dimension actions share one captured-context and explicit-discovery workflow. */
import { jest } from '@jest/globals';
import { CanvasDimensionCommands } from '../../../frontend/comfyui/ui/editor_actions/application/CanvasDimensionCommands.js';
import type {
  CanvasDimensionAction,
  DimensionDiscovery,
} from '../../../frontend/comfyui/ui/editor_actions/application/CanvasDimensionCommands.js';
import type { DimensionBinding } from '../../../frontend/comfyui/ui/editor_actions/application/CanvasDimensionFields.js';
import type {
  CanvasDimensionChoice,
  CanvasDimensionConfiguration,
  CanvasGeometryHost,
} from '../../../integrations/contracts/CanvasGeometry.js';
import type {
  CanvasGeometryCommand,
  CanvasGeometryResult,
} from '../../../integrations/contracts/CanvasProtocol.generated.js';

/** Substitute capability boundaries while retaining the production command sequencer and arithmetic. */
function fixture() {
  const values = { width: 768, height: 512 };
  const document = { id: 'doc', geometryRevision: 'geometry', width: 1920, height: 1080 };
  const context = {
    kind: 'context',
    supported: true,
    resize: true,
    selectionSupported: true,
    document,
    selection: { left: 50, top: 20, width: 400, height: 240 },
  } as const;
  let current: CanvasGeometryResult = context;
  const choices: CanvasDimensionChoice[] = [];
  const configurations: CanvasDimensionConfiguration[] = [];
  const commands: CanvasGeometryCommand[] = [];
  const execute = jest.fn<CanvasGeometryHost['execute']>().mockImplementation(async (command) => {
    commands.push(command);
    if (command.type === 'context') return current;
    if (command.type === 'resize')
      return { kind: 'resized', document: { ...document, ...command.size } };
    throw new Error('Unexpected native geometry command');
  });
  const binding: DimensionBinding = {
    width: {
      read: () => values.width,
      write: (value) => {
        values.width = value;
        return true;
      },
      disabled: () => false,
      limits: () => ({ minimum: 16, maximum: 16384, multiple: 8 }),
    },
    height: {
      read: () => values.height,
      write: (value) => {
        values.height = value;
        return true;
      },
      disabled: () => false,
      limits: () => ({ minimum: 16, maximum: 16384, multiple: 8 }),
    },
  };
  const discover = jest.fn<() => Promise<number>>().mockResolvedValue(2);
  const dispose = jest.fn<() => void>();
  const prepare = jest.fn<DimensionDiscovery['prepare']>().mockResolvedValue({
    outputs: [
      { id: 'first', label: 'First output' },
      { id: 'second', label: 'Second output' },
    ],
    discover,
    dispose,
  });
  const configure = async (
    request: CanvasDimensionConfiguration,
  ): Promise<CanvasDimensionChoice> => {
    configurations.push(request);
    return choices.shift() ?? { choice: 'apply', scale: '2', output: null };
  };
  const application = new CanvasDimensionCommands(() => ({ canvasId: 'editor', execute }), {
    prepare,
  });
  return {
    application,
    configure,
    binding,
    values,
    document,
    context,
    choices,
    commands,
    configurations,
    prepare,
    discover,
    dispose,
    setContext: (value: CanvasGeometryResult) => {
      current = value;
    },
  };
}

test.each<[CanvasDimensionAction, number, number, boolean]>([
  ['generation-from-canvas', 1920, 1080, false],
  ['canvas-from-generation', 768, 512, true],
  ['generation-from-canvas-scaled', 960, 544, false],
  ['canvas-from-generation-scaled', 1536, 1024, true],
  ['generation-from-selection', 400, 240, false],
  ['generation-from-selection-scaled', 200, 120, false],
])(
  'executes %s through the appropriate capability and field owners',
  async (action, width, height, resize) => {
    const f = fixture();
    await f.application.execute(action, f.binding, undefined, f.configure);
    if (resize) {
      expect(f.commands.at(-1)).toEqual({
        type: 'resize',
        document: f.document,
        size: { width, height },
      });
      expect(f.values).toEqual({ width: 768, height: 512 });
    } else {
      expect(f.values).toEqual({ width, height });
      expect(f.commands.some((command) => command.type === 'resize')).toBe(false);
    }
    expect(f.configurations.length > 0).toBe(action.endsWith('-scaled'));
    expect(f.prepare).not.toHaveBeenCalled();
  },
);

test('discovers only after explicit choice, resolves output ambiguity and confirms the measured factor', async () => {
  const f = fixture();
  f.choices.push(
    { choice: 'discover', scale: '1', output: null },
    { choice: 'discover', scale: '1', output: 'second' },
    { choice: 'apply', scale: '2', output: 'second' },
  );
  await f.application.execute('generation-from-canvas-scaled', f.binding, undefined, f.configure);
  expect(f.prepare).toHaveBeenCalledTimes(1);
  expect(f.discover).toHaveBeenCalledWith('second', undefined);
  expect(f.configurations).toEqual([
    expect.objectContaining({ stage: 'factor', outputs: [], scale: '1' }),
    expect.objectContaining({
      stage: 'factor',
      outputs: [
        { id: 'first', label: 'First output' },
        { id: 'second', label: 'Second output' },
      ],
    }),
    expect.objectContaining({ stage: 'confirm', scale: '2', output: 'second' }),
  ]);
  expect(f.values).toEqual({ width: 960, height: 544 });
  expect(f.dispose).toHaveBeenCalledTimes(1);
});

test('cancel leaves both the document and the generation fields untouched', async () => {
  const f = fixture();
  f.choices.push({ choice: 'cancel', scale: '1', output: null });
  await f.application.execute('canvas-from-generation-scaled', f.binding, undefined, f.configure);
  expect(f.values).toEqual({ width: 768, height: 512 });
  expect(f.commands.some((command) => command.type === 'resize')).toBe(false);
  expect(f.prepare).not.toHaveBeenCalled();
});

test('rejects changed canvas identity after a probe and disposes its captured workflow', async () => {
  const f = fixture();
  f.choices.push(
    { choice: 'discover', scale: '1', output: null },
    { choice: 'discover', scale: '1', output: 'first' },
  );
  f.discover.mockImplementation(async () => {
    f.setContext({ ...f.context, document: { ...f.document, geometryRevision: 'changed' } });
    return 2;
  });
  await expect(
    f.application.execute('generation-from-canvas-scaled', f.binding, undefined, f.configure),
  ).rejects.toThrow('canvas document changed');
  expect(f.values).toEqual({ width: 768, height: 512 });
  expect(f.dispose).toHaveBeenCalledTimes(1);
});

test('reports an absent selection without changing fields or requesting discovery', async () => {
  const f = fixture();
  f.setContext({ ...f.context, selection: null });
  await expect(
    f.application.execute('generation-from-selection', f.binding, undefined, f.configure),
  ).rejects.toThrow('Select an area');
  expect(f.values).toEqual({ width: 768, height: 512 });
  expect(f.prepare).not.toHaveBeenCalled();
});

test.each([
  ['generation-from-canvas', 1920, 1080],
  ['generation-from-canvas-scaled', 960, 544],
  ['generation-from-selection', 400, 240],
  ['generation-from-selection-scaled', 200, 120],
] as const)('replaces automatic dimension values through %s', async (action, width, height) => {
  const f = fixture();
  f.values.width = 0;
  f.values.height = 0;
  await f.application.execute(action, f.binding, undefined, f.configure);
  expect(f.values).toEqual({ width, height });
  if (action.endsWith('-scaled'))
    expect(f.configurations[0]).toMatchObject({
      generation: { width: 0, height: 0 },
    });
});

test('requires concrete generation dimensions before requesting a canvas resize', async () => {
  const f = fixture();
  f.values.width = 0;
  await expect(
    f.application.execute('canvas-from-generation', f.binding, undefined, f.configure),
  ).rejects.toThrow('Set a positive size');
  expect(f.commands).toEqual([]);
});
