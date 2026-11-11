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
/** Supply a small native graph boundary while retaining real association and source-binding owners. */
import { jest } from '@jest/globals';
import type { ComfyGraph, ComfyNode, ComfyLink } from '../../../frontend/comfyui/ui/types/graph.js';
import { ComfyInputAssociations } from '../../../frontend/comfyui/ui/input_canvas/infrastructure/ComfyInputAssociations.js';
import { ComfyInputBindings } from '../../../frontend/comfyui/ui/input_canvas/infrastructure/ComfyInputBindings.js';
import type { InputLoader } from '../../../frontend/comfyui/ui/input_canvas/domain/InputLoader.js';
import { sourceMaskFixture } from './source_mask_fixture.js';

/** Preserve actual link and callback semantics at the external native graph boundary. */
export function associationGraph() {
  const links = new Map<number, ComfyLink>();
  const configure = jest.fn();
  const graph: ComfyGraph = {
    _nodes: [],
    links,
    _links: links,
    beforeChange: jest.fn(),
    afterChange: jest.fn(() => {
      if (typeof graph.onAfterChange === 'function') graph.onAfterChange.call(graph, graph);
    }),
    add: (item) => {
      graph._nodes!.push(item);
      item.graph = graph;
    },
    remove: (item) => {
      graph._nodes = graph._nodes!.filter((node) => node !== item);
    },
    configure,
  };
  const connection = (
    node: ComfyNode,
    type: number,
    slot: number,
    connected: boolean,
    link: ComfyLink,
  ): void => {
    if (typeof node.onConnectionsChange === 'function')
      node.onConnectionsChange.call(node, type, slot, connected, link);
  };
  return {
    graph,
    links,
    configure,
    add: (...nodes: ComfyNode[]) => {
      for (const node of nodes) graph.add?.(node);
    },
    connect: (id: number, source: ComfyNode, output: number, target: ComfyNode, input: number) => {
      const link: ComfyLink = {
        id,
        origin_id: source.id!,
        origin_slot: output,
        target_id: target.id!,
        target_slot: input,
        type: source.outputs?.[output]?.type,
      };
      links.set(id, link);
      const outputSlot = source.outputs![output]!,
        inputSlot = target.inputs![input]!;
      outputSlot.links ??= [];
      outputSlot.links.push(id);
      inputSlot.link = id;
      connection(source, 2, output, true, link);
      connection(target, 1, input, true, link);
    },
    disconnect: (id: number) => {
      const link = links.get(id);
      if (!link) throw new Error('Missing fixture link.');
      links.delete(id);
      const source = graph._nodes!.find((node) => node.id === link.origin_id),
        target = graph._nodes!.find((node) => node.id === link.target_id);
      if (source && typeof link.origin_slot === 'number') {
        source.outputs![link.origin_slot]!.links = source.outputs![link.origin_slot]!.links!.filter(
          (value) => value !== id,
        );
        connection(source, 2, link.origin_slot, false, link);
      }
      if (target && typeof link.target_slot === 'number') {
        target.inputs![link.target_slot]!.link = null;
        connection(target, 1, link.target_slot, false, link);
      }
    },
  };
}

/** Describe independent editable fields and one shared image/mask consumer. */
export function associationNodes() {
  const image: ComfyNode = {
    id: 1000001,
    type: 'LoadImage',
    properties: {},
    widgets: [{ name: 'image', value: '' }],
    inputs: [],
    outputs: [
      { name: 'IMAGE', type: 'IMAGE', links: [] },
      { name: 'MASK', type: 'MASK', links: [] },
    ],
  };
  const mask: ComfyNode = {
    id: 1000002,
    type: 'LoadImageMask',
    properties: {},
    widgets: [
      { name: 'image', value: '' },
      { name: 'channel', value: 'red' },
    ],
    inputs: [],
    outputs: [{ name: 'MASK', type: 'MASK', links: [] }],
  };
  const consumer: ComfyNode = {
    id: 1000003,
    type: 'Inpaint',
    inputs: [
      { name: 'pixels', type: 'IMAGE', link: null },
      { name: 'mask', type: 'MASK', link: null },
    ],
    outputs: [],
  };
  return { image, mask, consumer };
}

/** Associate a tested mask with a real offscreen image loader through production graph-derived relationships. */
export async function maskPreviewAssociation(
  fixture: ReturnType<typeof sourceMaskFixture>,
  mask: ComfyNode,
  loader: InputLoader,
) {
  fixture.loaders.define({
    LoadImage: {},
    [mask.type ?? loader.classType]: {
      input: { required: { [loader.field]: ['COMBO', { image_upload: true }] } },
      output: ['MASK'],
    },
  });
  const nodes = associationNodes(),
    native = associationGraph();
  mask.outputs = [{ name: 'MASK', type: 'MASK', links: [] }];
  native.add(nodes.image, mask, nodes.consumer);
  native.connect(1, nodes.image, 0, nodes.consumer, 0);
  native.connect(2, mask, 0, nodes.consumer, 1);
  const bindings = new ComfyInputBindings(fixture.loaders, fixture.ports);
  const image = bindings.get(nodes.image, fixture.loaders.read('LoadImage')!);
  await image.initialize();
  const associations = new ComfyInputAssociations(
    fixture.loaders,
    bindings,
    () => native.graph,
    fixture.report,
    { warn: jest.fn() },
  );
  associations.start();
  return {
    ...native,
    image,
    imageNode: nodes.image,
    consumer: nodes.consumer,
    associations,
    source: associations.image(mask, loader.field),
    dispose: () => {
      associations.dispose();
      bindings.clear();
    },
  };
}
