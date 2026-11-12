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
/** Prove model-field discovery reads each node's own component subtree instead of the application tree. */
import { afterEach, expect, jest, test } from '@jest/globals';
import { readComfyModelFields } from '../../../frontend/comfyui/ui/model_library/ComfyModelFieldAdapter.js';
import type { ModelCatalogSnapshot } from '../../../frontend/comfyui/ui/model_library/ModelCatalog.js';
import { ComfyNodeVNodeIndex } from '../../../frontend/comfyui/ui/surface/ComfyVueTree.js';

afterEach(() => document.body.replaceChildren());

const snapshot: ModelCatalogSnapshot = {
  revision: 1,
  items: [],
  scanning: false,
  fieldContracts: [
    { classType: 'CheckpointLoaderSimple', inputName: 'ckpt_name', kind: 'checkpoints' },
  ],
};

interface MountedGraph {
  readonly roots: readonly HTMLElement[];
  readonly subtreeReads: number[];
  readonly applicationReads: () => number;
}

/** Mount Comfy's application tree with many native nodes whose subtree reads are counted. */
function mountGraph(count: number): MountedGraph {
  const app = document.createElement('div');
  app.id = 'vue-app';
  document.body.append(app);
  const subtreeReads: number[] = [];
  const roots: HTMLElement[] = [];
  const nodes = Array.from({ length: count }, (_unused, index) => {
    const root = document.createElement('div');
    root.className = 'lg-node';
    const frame = document.createElement('div');
    frame.setAttribute('data-capture-wheel', 'true');
    const trigger = document.createElement('button');
    trigger.dataset.testid = 'widget-select-default-trigger';
    frame.append(trigger);
    root.append(frame);
    app.append(root);
    roots.push(root);
    subtreeReads.push(0);
    const widget = { name: 'ckpt_name', options: { values: ['a.safetensors', 'None'] } };
    const field = {
      type: { __name: 'WidgetSelectDefault' },
      component: {
        props: { widget, modelValue: 'a.safetensors' },
        emit: jest.fn(),
        proxy: { $watch: () => () => undefined },
        subTree: { el: frame },
      },
    };
    const subTree = { el: root };
    Object.defineProperty(subTree, 'children', {
      get: () => {
        subtreeReads[index] = (subtreeReads[index] ?? 0) + 1;
        return [field];
      },
    });
    return {
      type: { __name: 'LGraphNode' },
      component: {
        props: { nodeData: { type: 'CheckpointLoaderSimple', widgets: [widget] } },
        subTree,
      },
    };
  });
  let applicationReads = 0;
  const appTree = {};
  Object.defineProperty(appTree, 'children', {
    get: () => {
      applicationReads += 1;
      return nodes;
    },
  });
  Object.assign(app, {
    _vnode: { type: { __name: 'GraphCanvas' }, component: { subTree: appTree } },
  });
  return { roots, subtreeReads, applicationReads: () => applicationReads };
}

test('reads only the requested node subtree for one native card', () => {
  const graph = mountGraph(25);
  const target = graph.roots[7];
  if (!target) throw new Error('Missing native node root.');
  const fields = readComfyModelFields(target, snapshot);
  expect(fields.map((field) => field.inputName)).toEqual(['ckpt_name']);
  expect(graph.subtreeReads[7]).toBeGreaterThan(0);
  expect(graph.subtreeReads.filter((reads) => reads > 0)).toHaveLength(1);
});

test('indexes the application tree once while each node reads its own subtree once per batch', () => {
  const graph = mountGraph(173);
  const nodes = new ComfyNodeVNodeIndex();
  const [first, ...rest] = graph.roots;
  if (!first) throw new Error('Missing native node root.');
  expect(readComfyModelFields(first, snapshot, nodes)).toHaveLength(1);
  const applicationReads = graph.applicationReads();
  const ownReads = graph.subtreeReads[0];
  for (const root of rest) expect(readComfyModelFields(root, snapshot, nodes)).toHaveLength(1);
  expect(graph.applicationReads()).toBe(applicationReads);
  expect(graph.subtreeReads.every((reads) => reads === ownReads)).toBe(true);
});
