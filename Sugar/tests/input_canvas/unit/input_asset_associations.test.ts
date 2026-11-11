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
/** Prove reusable image/mask relationships independently of rendering, source choices and editor identity. */
import { InputAssetAssociations } from '../../../frontend/comfyui/ui/input_canvas/application/InputAssetAssociations.js';
import type { InputAssetAssociationGraph } from '../../../frontend/comfyui/ui/input_canvas/domain/InputAssetAssociation.js';
import {
  exclusiveInputImage,
  resolveInputAssetAssociations,
} from '../../../frontend/comfyui/ui/input_canvas/domain/InputAssetAssociation.js';

/** Describe exact typed consumer connections supplied by the native adapter. */
function graph(
  imageInputs: readonly (readonly string[])[] = [['image']],
): InputAssetAssociationGraph {
  return {
    endpoints: [
      { key: 'image', kind: 'image' },
      { key: 'other', kind: 'image' },
      { key: 'mask', kind: 'mask' },
      { key: 'second-mask', kind: 'mask' },
    ],
    inputs: [
      ...imageInputs.map((endpoints, index) => ({
        consumer: 'inpaint',
        input: 'image-' + index,
        kind: 'image' as const,
        endpoints,
      })),
      { consumer: 'inpaint', input: 'mask', kind: 'mask', endpoints: ['mask'] },
      { consumer: 'inpaint', input: 'other-mask', kind: 'mask', endpoints: ['second-mask'] },
    ],
  };
}

test('editing requires exclusive image ownership even when each mask individually has a valid association', () => {
  const shared = graph();
  expect(
    exclusiveInputImage(resolveInputAssetAssociations([shared]), 'mask', new Set()),
  ).toBeNull();
  const single = {
    ...shared,
    inputs: shared.inputs.filter((input) => input.input !== 'other-mask'),
  };
  expect(exclusiveInputImage(resolveInputAssetAssociations([single]), 'mask', new Set())).toBe(
    'image',
  );
  expect(
    exclusiveInputImage(resolveInputAssetAssociations([single]), 'second-mask', new Set()),
  ).toBeNull();
});

test('a competing ambiguous mask also prevents editing a potentially shared image', () => {
  const shared = graph();
  const associations = resolveInputAssetAssociations([
    shared,
    {
      endpoints: [],
      inputs: [
        { consumer: 'other', input: 'image', kind: 'image', endpoints: ['other'] },
        { consumer: 'other', input: 'mask', kind: 'mask', endpoints: ['second-mask'] },
      ],
    },
  ]);
  expect(associations.get('second-mask')?.kind).toBe('ambiguous');
  expect(exclusiveInputImage(associations, 'mask', new Set())).toBeNull();
});

test('exposes shared-consumer associations in both directions for independent application consumers', () => {
  const owner = new InputAssetAssociations();
  owner.replace([graph()]);
  expect(owner.read('mask')).toMatchObject({
    kind: 'associated',
    image: 'image',
    consumers: ['inpaint'],
  });
  expect(owner.masksFor('image')).toEqual(['mask', 'second-mask']);
  expect(owner.read('unknown')).toMatchObject({ kind: 'missing' });
});

test.each([
  { inputs: [] },
  { inputs: [[]] },
  { inputs: [['image'], ['other']] },
  { inputs: [['image', 'other']] },
])('fails closed when the consumer cannot identify one image loader field: %j', ({ inputs }) => {
  const owner = new InputAssetAssociations();
  owner.replace([graph(inputs)]);
  expect(owner.read('mask').kind).not.toBe('associated');
});

test('coalesces shared use of one image and rejects masks that belong to distinct image contexts', () => {
  const owner = new InputAssetAssociations(),
    original = graph();
  owner.replace([
    original,
    {
      endpoints: [],
      inputs: [
        { consumer: 'second', input: 'image', kind: 'image', endpoints: ['image'] },
        { consumer: 'second', input: 'mask', kind: 'mask', endpoints: ['mask'] },
      ],
    },
  ]);
  expect(owner.read('mask')).toMatchObject({
    kind: 'associated',
    image: 'image',
    consumers: ['inpaint', 'second'],
  });
  owner.replace([
    original,
    {
      endpoints: [],
      inputs: [
        { consumer: 'second', input: 'image', kind: 'image', endpoints: ['other'] },
        { consumer: 'second', input: 'mask', kind: 'mask', endpoints: ['mask'] },
      ],
    },
  ]);
  expect(owner.read('mask')).toMatchObject({ kind: 'ambiguous', candidates: ['image', 'other'] });
});

test('notifies changed relationships and their shared-image editing eligibility without unrelated discovery', () => {
  const owner = new InputAssetAssociations(),
    events: string[][] = [];
  const stop = owner.subscribe((changed) => events.push([...changed]));
  owner.replace([graph()]);
  const association = owner.read('mask');
  owner.replace([graph()]);
  expect(events).toHaveLength(1);
  expect(owner.read('mask')).toBe(association);
  owner.replace([{ ...graph(), inputs: graph().inputs.filter((input) => input.input !== 'mask') }]);
  expect(events.at(-1)).toEqual(['mask', 'second-mask']);
  expect(owner.read('second-mask').kind).toBe('associated');
  expect(owner.exclusiveImage('second-mask')).toBe('image');
  owner.replace([]);
  expect(owner.masksFor('image')).toEqual([]);
  stop();
});
