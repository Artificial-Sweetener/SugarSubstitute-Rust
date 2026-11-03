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
/** Prove mask-only canvas resizing and write-back into its size authorities. */
import { jest } from '@jest/globals';
import {
  WorkflowCanvasResolution,
  type DimensionAuthorityWriter,
} from '../../../frontend/comfyui/ui/input_canvas/application/WorkflowCanvasResolution.js';
import type {
  ProvisionedSurface,
  WorkflowSurfaceState,
} from '../../../frontend/comfyui/ui/input_canvas/application/WorkflowSurfaceProvisioning.js';
import type { CanvasWorkflowHost } from '../../../integrations/contracts/CanvasWorkflowSurfaces.js';
import type {
  CanvasDocument,
  CanvasWorkflowCommand,
  CanvasWorkflowResult,
} from '../../../integrations/contracts/CanvasProtocol.generated.js';

const authority = [
  { node: '4', output: 0, width: 'width', height: 'height' },
  { node: '9:2', output: 0, width: 'target_width', height: 'target_height' },
];
const surface: ProvisionedSurface = {
  key: '@synthetic/0123456789abcdef',
  label: 'Mask',
  image: null,
  authority,
  base: { kind: 'blank', width: 512, height: 512 },
  document: { id: 'doc', geometryRevision: 'g1', width: 512, height: 512 },
  masks: [],
};

/** Compose the resolution owner over a fake canvas and writer. */
function fixture(reply?: CanvasWorkflowResult) {
  let state: WorkflowSurfaceState | null = {
    canvasId: 'native',
    workflow: 'workflow-a',
    surfaces: [surface, { ...surface, key: 'image', authority: null }],
    rejected: [],
  };
  const replaced: [string, CanvasDocument][] = [];
  const commands: CanvasWorkflowCommand[] = [];
  const host: CanvasWorkflowHost = {
    canvasId: 'native',
    execute: jest.fn(async (command: CanvasWorkflowCommand): Promise<CanvasWorkflowResult> => {
      commands.push(command);
      if (reply) return reply;
      if (command.type !== 'resize') return { kind: 'applied' };
      return {
        kind: 'synced',
        surfaces: [
          {
            key: command.surface,
            document: {
              id: 'doc',
              geometryRevision: 'g2',
              width: command.width,
              height: command.height,
            },
            masks: [],
          },
        ],
      };
    }),
  };
  const writer = { write: jest.fn<DimensionAuthorityWriter['write']>() };
  const resolution = new WorkflowCanvasResolution(
    {
      get state() {
        return state;
      },
      replaceSurface: (key, document) => replaced.push([key, document]),
    },
    () => host,
    writer,
  );
  return { resolution, commands, writer, replaced, clear: () => (state = null) };
}

test('authority fields find their mask-only surface; other fields find none', () => {
  const f = fixture();
  expect(f.resolution.surface('4', 'height')?.key).toBe(surface.key);
  expect(f.resolution.surface('9:2', 'target_width')?.key).toBe(surface.key);
  expect(f.resolution.surface('4', 'batch_size')).toBeNull();
  expect(f.resolution.surface('5', 'width')).toBeNull();
});

test('the canvas resizes first and every authority receives the new size', async () => {
  const f = fixture();
  await f.resolution.resize(surface.key, 1024, 768, { kind: 'scale', resampling: 'nearest' });
  expect(f.commands).toEqual([
    {
      type: 'resize',
      workflow: 'workflow-a',
      surface: surface.key,
      width: 1024,
      height: 768,
      mode: { kind: 'scale', resampling: 'nearest' },
    },
  ]);
  expect(f.replaced).toEqual([
    [surface.key, { id: 'doc', geometryRevision: 'g2', width: 1024, height: 768 }],
  ]);
  expect(f.writer.write).toHaveBeenCalledWith(authority, 1024, 768);
  await f.resolution.resize(surface.key, 512, 512, { kind: 'canvas', anchor: 'center' });
  expect(f.commands).toHaveLength(1);
});

test('refused, invalid and stale resizes leave the workflow unchanged', async () => {
  const refused = fixture({ kind: 'failure', status: 'error', message: 'No memory.' });
  await expect(
    refused.resolution.resize(surface.key, 640, 640, { kind: 'canvas', anchor: 'top-left' }),
  ).rejects.toThrow('The canvas could not be resized. Nothing was changed.');
  expect(refused.writer.write).not.toHaveBeenCalled();
  const f = fixture();
  for (const size of [63, 32769, 100.5])
    await expect(
      f.resolution.resize(surface.key, size, 512, { kind: 'canvas', anchor: 'center' }),
    ).rejects.toThrow(RangeError);
  await expect(
    f.resolution.resize('image', 640, 640, { kind: 'canvas', anchor: 'center' }),
  ).rejects.toThrow('The canvas changed while this dialog was open.');
  f.clear();
  await expect(
    f.resolution.resize(surface.key, 640, 640, { kind: 'canvas', anchor: 'center' }),
  ).rejects.toThrow('The canvas changed while this dialog was open.');
  expect(f.commands).toEqual([]);
  const failing = fixture();
  failing.writer.write.mockImplementation(() => {
    throw new Error('Node removed.');
  });
  await expect(
    failing.resolution.resize(surface.key, 640, 640, { kind: 'canvas', anchor: 'center' }),
  ).rejects.toThrow('the workflow dimensions could not be updated');
});
