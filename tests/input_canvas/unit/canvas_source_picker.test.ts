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
/** Prove reusable source controls reveal only their applicable secondary picker and preserve exact identities. */
import { jest } from '@jest/globals';
import {
  nativeInputFields,
  nativeSourceField,
  chooseNativeSource,
} from '../support/native_input_fields.js';
import type { ComfyNode } from '../../../frontend/comfyui/ui/types/graph.js';
import { CanvasSourcePicker } from '../../../frontend/comfyui/ui/input_canvas/presentation/CanvasSourcePicker.js';
import type {
  CanvasInputSource,
  CanvasSourceContext,
} from '../../../integrations/contracts/CanvasProtocol.generated.js';

const context: CanvasSourceContext = {
  observesChanges: true,
  kind: 'context',
  supported: true,
  document: { id: 'doc', geometryRevision: 'geometry', width: 10, height: 20 },
  revision: '1',
  targets: ['selection', 'current-layer', 'layer', 'composition', 'composition-from-layer'],
  layers: [
    { id: '1', name: '<img src=x onerror=alert(1)>', parentId: null, depth: 0, visible: true },
    { id: '2', name: 'Second', parentId: null, depth: 0, visible: true },
  ],
  currentLayerId: '2',
};

/** Mount the real native controls against one typed persistence and file boundary. */
function fixture(sourceContext: CanvasSourceContext = context) {
  let source: CanvasInputSource = {
    kind: 'canvas',
    canvasId: 'editor',
    documentId: null,
    target: { kind: 'selection' },
  };
  const report = jest.fn();
  const change = jest.fn((value: CanvasInputSource) => {
    source = value;
  });
  const node: ComfyNode = { widgets: [] };
  const fields = nativeInputFields(node);
  const fieldRoot = document.createElement('article');
  document.body.append(fieldRoot);
  const picker = new CanvasSourcePicker(
    document,
    {
      canvasId: 'editor',
      disabled: false,
      hidden: false,
      source: () => source,
      context: () => sourceContext,
      refresh: async () => {},
      change,
      upload: async () => 'uploads/chosen.png',
      report,
    },
    fields,
    fieldRoot,
    null,
  );
  document.body.append(picker.element);
  return { picker, change, report, node, source: () => source };
}

/** Wait for the consumer's asynchronous state transition without observing a different operation. */
async function settled(): Promise<void> {
  for (let n = 0; n < 12; n++) await Promise.resolve();
}

afterEach(() => document.body.replaceChildren());

test('selects canvas sources through the installed native field and exposes exact document layers', async () => {
  const f = fixture();
  await settled();
  expect(nativeSourceField(f.node).value).toBe('selection');
  chooseNativeSource(f.node, 'layer');
  await settled();
  expect(f.source()).toEqual({
    kind: 'canvas',
    canvasId: 'editor',
    documentId: 'doc',
    target: { kind: 'layer', layerId: '2' },
  });
  const layer = nativeSourceField(f.node, 'layer');
  expect(layer.options?.hidden).toBe(false);
  expect(layer.value).toBe('2');
  expect(layer.type).toBe('combo');
  chooseNativeSource(f.node, '1', 'layer');
  await settled();
  expect(f.source()).toMatchObject({ target: { kind: 'layer', layerId: '1' } });
  const label = nativeSourceField(f.node, 'layer').options?.getOptionLabel;
  expect(typeof label === 'function' ? label('1') : null).toBe('<img src=x onerror=alert(1)>');
  expect(f.picker.element.querySelector('img')).toBeNull();
  chooseNativeSource(f.node, 'composition-from-layer');
  await settled();
  expect(f.source()).toMatchObject({ target: { kind: 'composition-from-layer', layerId: '2' } });
  f.picker.dispose();
});

test('file source reveals a real upload picker and hides the canvas layer combo', async () => {
  const f = fixture();
  await settled();
  chooseNativeSource(f.node, 'file');
  await settled();
  const file = f.picker.element.querySelector<HTMLInputElement>('[aria-label="Source file"]');
  expect(file?.type).toBe('file');
  expect(file?.parentElement?.hidden).toBe(false);
  expect(nativeSourceField(f.node, 'layer').options?.hidden).toBe(true);
  if (!file) throw new Error('Missing file input.');
  Object.defineProperty(file, 'files', {
    value: [new File(['image'], 'chosen.png', { type: 'image/png' })],
  });
  file.dispatchEvent(new Event('change'));
  await settled();
  expect(f.source()).toEqual({ kind: 'file', file: 'uploads/chosen.png' });
  expect(f.report).not.toHaveBeenCalled();
  f.picker.dispose();
});

test('missing saved layers stay unavailable instead of silently selecting another layer', async () => {
  const f = fixture();
  await settled();
  f.change({
    kind: 'canvas',
    canvasId: 'editor',
    documentId: 'doc',
    target: { kind: 'layer', layerId: 'deleted' },
  });
  f.picker.refresh();
  expect(nativeSourceField(f.node, 'layer').value).toBe('');
  expect(f.source()).toMatchObject({ target: { layerId: 'deleted' } });
  f.picker.dispose();
});

test('keeps native field identity stable across unchanged committed metadata and releases its lifecycle', async () => {
  const f = fixture();
  await settled();
  const field = nativeSourceField(f.node);
  f.picker.refresh();
  await settled();
  expect(nativeSourceField(f.node)).toBe(field);
  expect(field.serialize).toBe(false);
  expect(field.options?.serialize).toBe(false);
  f.picker.dispose();
  expect(f.node.widgets).toEqual([]);
});

test('restores the accepted native field value when the canvas rejects a layer-based target', async () => {
  const f = fixture({ ...context, currentLayerId: null });
  await settled();
  chooseNativeSource(f.node, 'layer');
  await settled();
  expect(f.source()).toMatchObject({ target: { kind: 'selection' } });
  expect(nativeSourceField(f.node).value).toBe('selection');
  expect(f.report).toHaveBeenCalledWith(expect.objectContaining({ message: 'Select a layer' }));
  f.picker.dispose();
});
