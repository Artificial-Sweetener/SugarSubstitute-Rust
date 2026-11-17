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
/** Prove which links a saved workflow hands over, and how a linked field's card is kept. */
import { expect, test } from '@jest/globals';
import { readImportedNodeLinks } from '../../../frontend/comfyui/ui/workflow/ImportedNodeLinks.js';
import {
  isNativeWidgetLocallyRepresented,
  isNativeWidgetPresented,
  NativeWidgetPresentation,
} from '../../../frontend/comfyui/ui/surface/NativeWidgetPresentation.js';
import type { ComfyWidget } from '../../../frontend/comfyui/ui/types/graph.js';

const endpoint = (instance: string, symbol: string, input = 'value') => ({
  instance_id: instance,
  node_symbol: symbol,
  input_name: input,
});
const identity = (instance: string, symbol: string) => JSON.stringify(['cube', instance, symbol]);

test('only the links a workflow records are read, and overrides are not links', () => {
  const workflow = {
    extra: {
      sugarcubes_composition: {
        schema_version: 1,
        value_relations: [
          {
            kind: 'field_link',
            source: endpoint('first', 'negative_prompt'),
            targets: [endpoint('second', 'negative_prompt')],
          },
          {
            kind: 'global_override',
            override_key: 'seed',
            value: 7,
            targets: [
              endpoint('first', 'ksampler', 'seed'),
              endpoint('second', 'ksampler', 'seed'),
            ],
          },
        ],
      },
    },
  };
  expect(readImportedNodeLinks(workflow)).toEqual([
    { target: identity('second', 'negative_prompt'), source: identity('first', 'negative_prompt') },
  ]);
});

test('a workflow with a composition and no link records that nothing is linked', () => {
  expect(readImportedNodeLinks({ extra: { sugarcubes_composition: {} } })).toEqual([]);
});

test('a workflow with no composition records no links of its own', () => {
  expect(readImportedNodeLinks({ extra: {} })).toBeNull();
  expect(readImportedNodeLinks({})).toBeNull();
});

test('a field replaced on its card keeps the card, and passes between owners without a clash', () => {
  const widget: ComfyWidget = { name: 'variation_strength', type: 'number', value: 0 };
  const moved = new NativeWidgetPresentation();
  const replaced = new NativeWidgetPresentation('replacement');
  replaced.reconcile(new Set([widget]));
  expect(isNativeWidgetPresented(widget)).toBe(false);
  expect(isNativeWidgetLocallyRepresented(widget)).toBe(true);

  // The toolbar takes the field: the link's owner lets go first.
  replaced.release(new Set());
  moved.reconcile(new Set([widget]));
  expect(isNativeWidgetLocallyRepresented(widget)).toBe(false);
  moved.dispose();
  expect(isNativeWidgetPresented(widget)).toBe(true);
});
