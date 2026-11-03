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
/** Prove which prompts name a regional mask loader's regions. */
import { readRegionalPrompts } from '../../../frontend/comfyui/ui/input_canvas/infrastructure/ComfyRegionalPrompts.js';
import type { SpatialEdge } from '../../../frontend/comfyui/ui/input_canvas/domain/CanvasDimensionAuthority.js';
import type { ComfyNode } from '../../../frontend/comfyui/ui/types/graph.js';

/** Build a multiline prompt node. */
function prompt(text: string): ComfyNode {
  return { type: 'PrimitiveStringMultiline', widgets: [{ name: 'value', value: text }] };
}

test('prompts within three upstream hops of the mask consumers name its regions', () => {
  const nodes: Record<string, ComfyNode> = {
    near: prompt('global\n[SEP|sky]\nblue\n[SEP]\nground'),
    far: prompt('a\n[SEP]\nb\n[SEP|person]\nc\n[SEP|extra]\nd'),
    tooFar: prompt('x\n[SEP|hidden]\ny\n[SEP]\nz\n[SEP]\nw\n[SEP]\nv'),
    inline: prompt('not a separator [SEP|inline] here'),
  };
  const edge = (source: string, target: string, type: string | null = null): SpatialEdge => ({
    source,
    output: 0,
    type,
    target,
    inputType: type,
  });
  const result = readRegionalPrompts(
    {
      nodes: [],
      edges: [
        edge('mask', 'regional', 'MASK'),
        edge('near', 'regional'),
        edge('encode', 'regional'),
        edge('concat', 'encode'),
        edge('far', 'concat'),
        edge('inline', 'concat'),
        edge('deeper', 'concat'),
        edge('deepest', 'deeper'),
        edge('tooFar', 'deepest'),
      ],
    },
    'mask',
    (path) => nodes[path] ?? { type: 'Other', widgets: [] },
  );
  expect(result.nodes).toEqual([nodes.near, nodes.far, nodes.inline]);
  expect(result.demand).toEqual({ names: ['sky', 'person', 'extra'], required: 3 });
});
