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
/** Prove the Input/Output route, card activation, remembered selection, opacity and hover. */
import { jest } from '@jest/globals';
import {
  WorkflowInputRoute,
  type WorkflowInputSelection,
} from '../../../frontend/comfyui/ui/input_canvas/application/WorkflowInputRoute.js';
import { WorkflowMaskAppearance } from '../../../frontend/comfyui/ui/input_canvas/application/WorkflowMaskAppearance.js';
import type {
  ProvisionedSurface,
  WorkflowSurfaceState,
} from '../../../frontend/comfyui/ui/input_canvas/application/WorkflowSurfaceProvisioning.js';
import type { CanvasWorkflowHost } from '../../../integrations/contracts/CanvasWorkflowSurfaces.js';
import type {
  CanvasWorkflowCommand,
  CanvasWorkflowResult,
} from '../../../integrations/contracts/CanvasProtocol.generated.js';

/** Describe a provisioned surface with masks of one endpoint. */
function surface(key: string, image: string | null, masks: readonly string[]): ProvisionedSurface {
  return {
    key,
    label: key,
    image,
    authority: null,
    base: { kind: 'blank', width: 8, height: 8 },
    document: { id: 'doc:' + key, geometryRevision: 'g', width: 8, height: 8 },
    masks: masks.map((mask, index) => ({
      key: mask,
      endpoint: mask.startsWith('[') ? 'regions' : mask,
      entry: mask.startsWith('[') ? String(index) : null,
      label: mask,
      color: '#479ef5',
      opacity: 0.5,
      initial: null,
      channel: 'alpha',
      layerId: 'layer:' + mask,
    })),
  };
}

/** Publish synced states and record canvas commands and stored selections. */
function fixture(stored: Record<string, WorkflowInputSelection> = {}) {
  let state: WorkflowSurfaceState | null = null;
  const listeners = new Set<() => void>();
  const commands: CanvasWorkflowCommand[] = [];
  const host: CanvasWorkflowHost = {
    canvasId: 'native',
    execute: jest.fn(async (command: CanvasWorkflowCommand): Promise<CanvasWorkflowResult> => {
      commands.push(command);
      return command.type === 'activate' ? { kind: 'activated' } : { kind: 'applied' };
    }),
  };
  const source = {
    get state() {
      return state;
    },
    subscribe: (listener: () => void) => {
      listeners.add(listener);
      return () => listeners.delete(listener);
    },
  };
  const writes: [string, WorkflowInputSelection][] = [];
  const store = {
    read: (workflow: string) => stored[workflow] ?? null,
    write: (workflow: string, selection: WorkflowInputSelection) => {
      writes.push([workflow, selection]);
      stored[workflow] = selection;
    },
  };
  const report = jest.fn<(error: unknown) => void>();
  const route = new WorkflowInputRoute(source, () => host, store, report);
  return {
    route,
    commands,
    writes,
    report,
    source,
    host,
    publish(surfaces: readonly ProvisionedSurface[] | null, workflow = 'workflow-a') {
      state = surfaces ? { canvasId: 'native', workflow, surfaces, rejected: [] } : null;
      for (const listener of listeners) listener();
    },
  };
}

test('Input is unavailable and the view stays on Output without surfaces', () => {
  const f = fixture({ 'workflow-a': { route: 'input', surface: null, mask: null } });
  expect(f.route.read()).toBeNull();
  f.publish([]);
  expect(f.route.read()).toEqual({
    route: 'output',
    surface: null,
    mask: null,
    available: false,
  });
  f.route.chooseRoute('input');
  expect(f.route.read()?.route).toBe('output');
  expect(f.commands).toEqual([
    { type: 'activate', workflow: 'workflow-a', route: 'output', surface: null, mask: null },
  ]);
});

test('a workflow that remembers no selection opens on Input with its first surface', () => {
  const f = fixture();
  f.publish([surface('image', 'image', ['mask-a']), surface('other', 'other', [])]);
  expect(f.route.read()).toEqual({
    route: 'input',
    surface: 'image',
    mask: 'mask-a',
    available: true,
  });
  expect(f.commands.at(-1)).toEqual({
    type: 'activate',
    workflow: 'workflow-a',
    route: 'input',
    surface: 'image',
    mask: 'mask-a',
  });
  const remembered = fixture({ 'workflow-a': { route: 'output', surface: null, mask: null } });
  remembered.publish([surface('image', 'image', [])]);
  expect(remembered.route.read()?.route).toBe('output');
});

test('thumbnail and mask clicks activate their surface and mask on Input and are remembered', () => {
  const f = fixture();
  f.publish([surface('image', 'image', ['mask-a', 'mask-b']), surface('other', 'other', [])]);
  f.route.chooseRoute('output');
  expect(f.route.read()?.route).toBe('output');
  expect(f.route.activateImage('image')).toBe(true);
  expect(f.route.read()).toMatchObject({ route: 'input', surface: 'image', mask: 'mask-a' });
  expect(f.route.activateMask('mask-b', null)).toBe(true);
  expect(f.commands.at(-1)).toEqual({
    type: 'activate',
    workflow: 'workflow-a',
    route: 'input',
    surface: 'image',
    mask: 'mask-b',
  });
  expect(f.writes.at(-1)).toEqual([
    'workflow-a',
    { route: 'input', surface: 'image', mask: 'mask-b' },
  ]);
  expect(f.route.activateImage('missing')).toBe(false);
  f.route.chooseRoute('output');
  expect(f.commands.at(-1)).toMatchObject({ route: 'output', surface: 'image', mask: 'mask-b' });
  const sent = f.commands.length;
  f.publish([surface('image', 'image', ['mask-a', 'mask-b']), surface('other', 'other', [])]);
  expect(f.commands).toHaveLength(sent);
});

test('each workflow restores its own selection and missing surfaces are cleared, not reassigned', () => {
  const f = fixture({
    'workflow-b': { route: 'input', surface: 'image-b', mask: 'gone' },
  });
  f.publish([surface('image-b', 'image-b', ['mask-b'])], 'workflow-b');
  expect(f.route.read()).toMatchObject({ route: 'input', surface: 'image-b', mask: null });
  f.publish([surface('image-a', 'image-a', ['mask-a'])], 'workflow-a');
  f.route.chooseRoute('output');
  expect(f.route.read()).toMatchObject({ route: 'output', surface: 'image-a' });
  f.route.chooseRoute('input');
  expect(f.route.read()).toMatchObject({ route: 'input', surface: 'image-a', mask: 'mask-a' });
  f.publish([surface('image-b', 'image-b', ['mask-b'])], 'workflow-b');
  expect(f.route.read()).toMatchObject({ route: 'input', surface: 'image-b' });
  f.publish(null);
  expect(f.route.read()).toBeNull();
});

test('activation refusals are reported and resent on the next change', async () => {
  const f = fixture();
  jest
    .mocked(f.host.execute)
    .mockResolvedValueOnce({ kind: 'failure', status: 'stale', message: 'Stale surface.' });
  f.publish([surface('image', 'image', [])]);
  await Promise.resolve();
  await Promise.resolve();
  expect(f.report).toHaveBeenCalledWith(new Error('Stale surface.'));
  f.publish([surface('image', 'image', [])]);
  expect(f.host.execute).toHaveBeenCalledTimes(2);
});

test('opacity applies to every mask of a loader at once and hover names one region', () => {
  const f = fixture();
  const store = { write: jest.fn<(endpoint: string, opacity: number) => void>() };
  const appearance = new WorkflowMaskAppearance(f.source, () => f.host, store, f.report);
  expect(appearance.opacity('regions')).toBeNull();
  f.publish([surface('image', 'image', ['["regions","0"]', '["regions","1"]', 'single'])]);
  expect(appearance.provisioned('regions')).toBe(true);
  expect(appearance.opacity('regions')).toBe(0.5);
  appearance.changeOpacity('regions', 0.25);
  expect(store.write).toHaveBeenCalledWith('regions', 0.25);
  expect(f.commands.at(-1)).toEqual({
    type: 'appearance',
    workflow: 'workflow-a',
    masks: [
      { surface: 'image', mask: '["regions","0"]', color: '#479ef5', opacity: 0.25 },
      { surface: 'image', mask: '["regions","1"]', color: '#479ef5', opacity: 0.25 },
    ],
  });
  expect(() => appearance.changeOpacity('regions', 1.5)).toThrow(RangeError);
  appearance.hover('regions', '1');
  appearance.hover('regions', '1');
  appearance.hover('regions', null);
  expect(f.commands.filter((command) => command.type === 'hover')).toEqual([
    { type: 'hover', workflow: 'workflow-a', surface: 'image', mask: '["regions","1"]' },
    { type: 'hover', workflow: 'workflow-a', surface: 'image', mask: null },
  ]);
});
