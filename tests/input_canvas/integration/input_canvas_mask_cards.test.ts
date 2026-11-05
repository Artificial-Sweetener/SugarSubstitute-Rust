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
/** Prove mask cards present the input canvas's mask workflow there and source bindings elsewhere. */
import { jest } from '@jest/globals';
import { randomUUID } from 'node:crypto';
import { sourceMaskFixture } from '../support/source_mask_fixture.js';
import { nativeInputFields, nativeSourceField } from '../support/native_input_fields.js';
import { createComfyInputBinding } from '../../../frontend/comfyui/ui/input_canvas/infrastructure/ComfyInputBinding.js';
import { MasonryInputEditor } from '../../../frontend/comfyui/ui/input_canvas/presentation/MasonryInputEditor.js';
import type { InputSurfaceCardPort } from '../../../frontend/comfyui/ui/input_canvas/application/InputEditorPorts.js';
import type { ComfyNode } from '../../../frontend/comfyui/ui/types/graph.js';

beforeAll(() =>
  Object.defineProperty(crypto, 'randomUUID', { configurable: true, value: randomUUID }),
);
beforeEach(() => {
  // jsdom supplies no native raster context; previews are outside this proof.
  jest.spyOn(HTMLCanvasElement.prototype, 'getContext').mockReturnValue({
    createImageData: (width: number, height: number) => ({
      data: new Uint8ClampedArray(width * height * 4),
    }),
    putImageData: jest.fn(),
  } as unknown as CanvasRenderingContext2D);
});
afterEach(() => {
  document.body.replaceChildren();
  jest.restoreAllMocks();
});

const scalar = {
  kind: 'mask' as const,
  classType: 'LoadImageMask',
  field: 'image',
  channel: 'channel',
  ordered: false,
};
const regional = { ...scalar, classType: 'SimpleSyrup.LoadMaskBatch', ordered: true };

/** Wait for committed metadata and preview microtasks. */
async function settled(): Promise<void> {
  for (let n = 0; n < 50; n++) await Promise.resolve();
}

/** A card port whose regional loader accepts imports only where surfaces are provisioned. */
function card(provisioned: boolean): InputSurfaceCardPort {
  return {
    activate: jest.fn(),
    openChosen: jest.fn(),
    hover: jest.fn(),
    hovered: () => null,
    subscribeHover: () => () => {},
    region: () => null,
    opacity: () => 0.5,
    changeOpacity: jest.fn(),
    canImport: () => provisioned,
    importMasks: jest.fn(async () => {}),
    subscribe: () => () => {},
  };
}

/** Mount one mask card on a canvas that does or does not provision input surfaces. */
async function mount(loader: typeof scalar, provisions: boolean) {
  const f = sourceMaskFixture();
  const ports = { ...f.ports, canvases: { editable: () => true, provisions: () => provisions } };
  const node: ComfyNode = {
    id: 7,
    type: loader.classType,
    widgets: [
      { name: 'image', value: loader.ordered ? [] : 'mask.png' },
      { name: 'channel', value: 'red' },
      { name: 'upload', type: 'button' },
    ],
    properties: {},
    graph: { beforeChange: jest.fn(), afterChange: jest.fn() },
  };
  const binding = createComfyInputBinding(node, loader, ports);
  await binding.initialize();
  const upload = jest.fn(async (file: File) => 'sugarcubes/sources/' + file.name);
  const view = new MasonryInputEditor(
    document,
    binding,
    f.previews,
    upload,
    f.report,
    nativeInputFields(node),
    document.createElement('article'),
    null,
    false,
    card(provisions),
  );
  document.body.append(view.element, view.controlsElement);
  await settled();
  const shown = () =>
    [...view.controlsElement.querySelectorAll<HTMLElement>('[data-sc-button]')]
      .filter((button) => !button.closest('[hidden]'))
      .map((button) => button.textContent);
  return { ...f, node, binding, view, upload, shown };
}

test('an input-canvas scalar mask card loads a picture with Browse Files and has no source fields', async () => {
  const f = await mount(scalar, true);
  expect(nativeSourceField(f.node).hidden).toBe(true);
  expect(f.shown()).toEqual(['Browse Files']);
  const input = f.view.controlsElement.querySelector<HTMLInputElement>(
    '.sc-input-editor__browse input[type="file"]',
  );
  const picture = new File(['m'], 'picked.png');
  Object.defineProperty(input, 'files', { configurable: true, value: [picture] });
  input?.dispatchEvent(new Event('change'));
  await settled();
  expect(f.upload).toHaveBeenCalledWith(picture);
  expect(f.binding.state.entries[0]?.source).toEqual({
    kind: 'file',
    file: 'sugarcubes/sources/picked.png',
  });
  expect(f.binding.state.explicit).toBeUndefined();
  expect(f.report).not.toHaveBeenCalled();
  f.view.dispose();
  f.binding.dispose();
  f.previews.dispose();
});

test('an input-canvas regional mask card adds, chooses and removes regions without reordering', async () => {
  const f = await mount(regional, true);
  expect(nativeSourceField(f.node).hidden).toBe(true);
  expect(f.shown()).toEqual(['Add mask', 'Choose mask', 'Remove mask']);
  f.view.dispose();
  f.binding.dispose();
  f.previews.dispose();
});

test('peer canvases keep explicit source bindings and ordered collection controls', async () => {
  const single = await mount(scalar, false);
  expect(nativeSourceField(single.node).hidden).toBe(false);
  expect(single.shown()).toEqual([]);
  single.view.dispose();
  single.binding.dispose();
  single.previews.dispose();
  const ordered = await mount(regional, false);
  expect(nativeSourceField(ordered.node).hidden).toBe(false);
  expect(ordered.shown()).toEqual(['Add mask', 'Remove mask', 'Move mask up', 'Move mask down']);
  ordered.view.dispose();
  ordered.binding.dispose();
  ordered.previews.dispose();
});
