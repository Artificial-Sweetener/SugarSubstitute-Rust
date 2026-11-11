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
/** Prove the "Change canvas resolution" entry, dialog and authority write-back. */
import { jest } from '@jest/globals';
import { CanvasResolutionDialog } from '../../../frontend/comfyui/ui/input_canvas/presentation/CanvasResolutionDialog.js';
import {
  canvasResolutionEntries,
  canvasResolutionNodeMenuItems,
} from '../../../frontend/comfyui/ui/input_canvas/presentation/CanvasResolutionMenu.js';
import { ComfyDimensionAuthorityWriter } from '../../../frontend/comfyui/ui/input_canvas/infrastructure/ComfyDimensionAuthorityWriter.js';
import type { ProvisionedSurface } from '../../../frontend/comfyui/ui/input_canvas/application/WorkflowSurfaceProvisioning.js';
import type { ComfyGraph, ComfyNode } from '../../../frontend/comfyui/ui/types/graph.js';

afterEach(() => document.body.replaceChildren());

const surface: ProvisionedSurface = {
  key: 'mask-only',
  label: 'Mask',
  image: null,
  authority: [{ node: '4', output: 0, width: 'width', height: 'height' }],
  base: { kind: 'blank', width: 512, height: 512 },
  document: { id: 'doc', geometryRevision: 'g', width: 512, height: 512 },
  masks: [],
};

/** Read the open dialog's controls. */
function controls() {
  const dialog = document.querySelector<HTMLElement>('.sc-canvas-resolution');
  const inputs = [...(dialog?.querySelectorAll<HTMLInputElement>('input[type="number"]') ?? [])];
  const selects = [...(dialog?.querySelectorAll<HTMLSelectElement>('select') ?? [])];
  const buttons = [...(dialog?.querySelectorAll<HTMLButtonElement>('button') ?? [])];
  const confirm = buttons.find((button) =>
    ['Resize canvas', 'Scale canvas'].includes(button.textContent ?? ''),
  );
  return { dialog, inputs, selects, confirm };
}

/** Change a control and notify its listeners. */
function set(control: HTMLInputElement | HTMLSelectElement, value: string): void {
  control.value = value;
  control.dispatchEvent(new Event('input', { bubbles: true }));
  control.dispatchEvent(new Event('change', { bubbles: true }));
}

test('the entry appears only for authority fields and the dialog applies the chosen mode', async () => {
  const resolution = {
    surface: jest.fn((node: string, field: string) =>
      node === '4' && field === 'width' ? surface : null,
    ),
    resize: jest.fn(async () => {}),
  };
  const dialog = new CanvasResolutionDialog(document);
  expect(
    canvasResolutionEntries([{ nodeId: '7', inputName: 'width' }], resolution, dialog, jest.fn()),
  ).toEqual([]);
  const entries = canvasResolutionEntries(
    [{ nodeId: '4', inputName: 'width' }],
    resolution,
    dialog,
    jest.fn(),
  );
  expect(entries.map((entry) => entry.label)).toEqual(['', 'Change canvas resolution…']);
  entries[1]?.select?.();
  let view = controls();
  expect(view.dialog?.textContent).toContain('Change canvas resolution');
  expect(view.inputs.map((input) => input.value)).toEqual(['512', '512']);
  expect(view.confirm?.disabled).toBe(true);
  const [width, height] = view.inputs;
  const [scope, anchor, resampling] = view.selects;
  set(width!, '1024');
  expect(view.confirm?.disabled).toBe(false);
  expect(anchor?.value).toBe('center');
  expect(resampling?.closest('label')?.hidden).toBe(true);
  set(scope!, 'scale');
  view = controls();
  expect(view.confirm?.textContent).toBe('Scale canvas');
  expect(anchor?.closest('label')?.hidden).toBe(true);
  set(resampling!, 'nearest');
  set(height!, '40');
  expect(view.confirm?.disabled).toBe(true);
  set(height!, '768');
  view.confirm?.click();
  await Promise.resolve();
  expect(resolution.resize).toHaveBeenCalledWith('mask-only', 1024, 768, {
    kind: 'scale',
    resampling: 'nearest',
  });
});

test('a refused resize keeps the dialog open with its message', async () => {
  const dialog = new CanvasResolutionDialog(document);
  const apply = jest.fn(async () => {
    throw new Error('The canvas could not be resized. Nothing was changed.');
  });
  const result = dialog.open({ width: 512, height: 512 }, apply);
  const view = controls();
  set(view.inputs[0]!, '640');
  view.confirm?.click();
  await Promise.resolve();
  await Promise.resolve();
  expect(apply).toHaveBeenCalledWith(640, 512, { kind: 'canvas', anchor: 'center' });
  expect(controls().dialog?.textContent).toContain('Nothing was changed.');
  dialog.close();
  await expect(result).resolves.toBe(false);
});

test('authority fields are written as one history step through their native callbacks', () => {
  const callback = jest.fn();
  const changed = jest.fn();
  const node: ComfyNode = {
    id: 4,
    type: 'EmptyLatentImage',
    widgets: [
      { name: 'width', value: 512, callback },
      { name: 'height', value: 512, callback },
    ],
    onWidgetChanged: changed,
  };
  const graph: ComfyGraph = { beforeChange: jest.fn(), afterChange: jest.fn() };
  const writer = new ComfyDimensionAuthorityWriter(
    (path) => (path === '4' ? node : null),
    () => graph,
  );
  writer.write(surface.authority ?? [], 1024, 512);
  expect(node.widgets?.map((widget) => widget.value)).toEqual([1024, 512]);
  expect(callback).toHaveBeenCalledTimes(1);
  expect(changed).toHaveBeenCalledWith('width', 1024, 512, node.widgets?.[0]);
  expect(graph.beforeChange).toHaveBeenCalledTimes(1);
  expect(graph.afterChange).toHaveBeenCalledTimes(1);
  expect(() =>
    writer.write([{ node: 'gone', output: 0, width: 'width', height: 'height' }], 64, 64),
  ).toThrow('no longer in the workflow');
});

test('the node context menu of a sizing node offers the same dialog in either renderer', async () => {
  const resolution = {
    surfaceSizedBy: jest.fn((node: string) => (node === '4' ? surface : null)),
    resize: jest.fn(async () => {}),
  };
  const dialog = new CanvasResolutionDialog(document);
  expect(canvasResolutionNodeMenuItems('7', resolution, dialog, jest.fn())).toEqual([]);
  expect(canvasResolutionNodeMenuItems(null, resolution, dialog, jest.fn())).toEqual([]);
  const [item] = canvasResolutionNodeMenuItems('4', resolution, dialog, jest.fn());
  expect(item?.content).toBe('Change canvas resolution…');
  item?.callback();
  const view = controls();
  expect(view.inputs.map((input) => input.value)).toEqual(['512', '512']);
  set(view.inputs[1]!, '256');
  view.confirm?.click();
  await Promise.resolve();
  expect(resolution.resize).toHaveBeenCalledWith('mask-only', 512, 256, {
    kind: 'canvas',
    anchor: 'center',
  });
});
