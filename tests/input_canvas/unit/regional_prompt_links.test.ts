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
/** Prove how regional mask rows, canvas masks and prompt separators name, colour and hover together. */
import { jest } from '@jest/globals';
import { regionalPromptDemand } from '../../../frontend/comfyui/ui/input_canvas/domain/RegionalPromptDemand.js';
import { regionMaskColor } from '../../../frontend/comfyui/ui/input_canvas/domain/MaskColorScheme.js';
import { planWorkflowSurfaces } from '../../../frontend/comfyui/ui/input_canvas/domain/WorkflowSurfacePlan.js';
import type {
  WorkflowMaskEndpoint,
  WorkflowSurfaceFacts,
} from '../../../frontend/comfyui/ui/input_canvas/domain/WorkflowSurfacePlan.js';
import { WorkflowRegionalPrompts } from '../../../frontend/comfyui/ui/input_canvas/application/WorkflowRegionalPrompts.js';
import { WorkflowRegionalHover } from '../../../frontend/comfyui/ui/input_canvas/application/WorkflowRegionalHover.js';

/** Describe an ordered loader whose regions the given prompts name. */
function regional(
  key: string,
  entries: number,
  prompts: readonly { path: string; separators: (string | null)[] }[],
): WorkflowMaskEndpoint {
  return {
    key,
    label: 'Load Mask Batch',
    opacity: 0.5,
    image: 'image',
    authority: { kind: 'missing', reason: 'mask_output_is_unused' },
    regions: regionalPromptDemand(prompts),
    channel: 'alpha',
    entries: Array.from({ length: entries }, (_, index) => ({
      entry: key + String(index),
      label: 'Mask ' + String(index + 1),
      initial: null,
    })),
  };
}

/** Serve mutable facts through the graph port and record every change notification. */
function graph(initial: WorkflowSurfaceFacts) {
  let facts = initial;
  const listeners = new Set<() => void>();
  const read = jest.fn(() => facts);
  return {
    port: {
      facts: read,
      subscribe: (listener: () => void) => {
        listeners.add(listener);
        return () => listeners.delete(listener);
      },
    },
    read,
    change(next: WorkflowSurfaceFacts) {
      facts = next;
      for (const listener of listeners) listener();
    },
  };
}

const prompts = [
  { path: 'positive', separators: [null, 'sky'] },
  { path: 'negative', separators: ['person', 'cloud', 'extra'] },
];

test('rows take the first-authored separator name and the regional colour of their position', () => {
  const g = graph({ images: [], masks: [regional('regions', 2, prompts)] });
  const regions = new WorkflowRegionalPrompts(g.port, jest.fn());
  expect(regions.region('regions', 'regions0')).toEqual({
    label: 'person',
    color: regionMaskColor(0, 2),
  });
  expect(regions.region('regions', 'regions1')).toEqual({
    label: 'sky',
    color: regionMaskColor(1, 2),
  });
  expect(regions.region('regions', 'missing')).toBeNull();
  expect(regions.region('scalar', 'regions0')).toBeNull();
  expect(regions.forPrompt('negative').map((mask) => mask.endpoint)).toEqual(['regions']);
  expect(g.read).toHaveBeenCalledTimes(1);
  const changed = jest.fn();
  regions.subscribe(changed);
  g.change({ images: [], masks: [regional('regions', 1, [])] });
  expect(changed).toHaveBeenCalledTimes(1);
  expect(regions.region('regions', 'regions0')).toEqual({
    label: null,
    color: regionMaskColor(0, 1),
  });
  expect(g.read).toHaveBeenCalledTimes(2);
  regions.dispose();
});

test('canvas masks of an ordered loader take the same regional colours as its rows', () => {
  const plan = planWorkflowSurfaces(
    {
      images: [
        { key: 'image', label: 'Image', file: { filename: 'a.png', subfolder: '', type: 'input' } },
      ],
      masks: [
        {
          ...regional('scalar', 1, []),
          regions: null,
          entries: [{ entry: null, label: 'Scalar', initial: null }],
        },
        regional('regions', 3, prompts),
      ],
    },
    '#479ef5',
    { synthetic: 'Mask canvas' },
  );
  expect(plan.surfaces[0]?.masks.map((mask) => mask.color)).toEqual([
    '#479ef5',
    regionMaskColor(0, 3),
    regionMaskColor(1, 3),
    regionMaskColor(2, 3),
  ]);
});

/** Link two loaders, a canvas and a prompt editor host through the hover owner. */
function hover() {
  const g = graph({
    images: [],
    masks: [regional('regions', 2, prompts), regional('other', 1, [])],
  });
  const regions = new WorkflowRegionalPrompts(g.port, jest.fn());
  const canvas = { hover: jest.fn() };
  const report = jest.fn();
  const owner = new WorkflowRegionalHover(regions, canvas, report);
  const highlight = jest.fn();
  owner.connect({ highlight });
  const changed = jest.fn();
  owner.subscribe(changed);
  return { g, owner, canvas, highlight, changed, report };
}

test('hovering a row highlights its canvas mask and the matching separator of every related prompt', () => {
  const h = hover();
  h.owner.hoverMask('regions', 'regions1');
  expect(h.canvas.hover).toHaveBeenLastCalledWith('regions', 'regions1');
  expect(h.highlight).toHaveBeenLastCalledWith(['positive', 'negative'], 1);
  expect(h.owner.hovered('regions')).toBe('regions1');
  expect(h.owner.hovered('other')).toBeNull();
  h.owner.hoverMask('regions', null);
  expect(h.canvas.hover).toHaveBeenLastCalledWith('regions', null);
  expect(h.highlight).toHaveBeenLastCalledWith(['positive', 'negative'], null);
  expect(h.owner.hovered('regions')).toBeNull();
  expect(h.changed).toHaveBeenCalledTimes(2);
  h.owner.hoverMask('other', 'other0');
  expect(h.canvas.hover).toHaveBeenLastCalledWith('other', 'other0');
  expect(h.highlight).toHaveBeenCalledTimes(2);
  expect(h.report).not.toHaveBeenCalled();
});

test('hovering a separator highlights the related row, its canvas mask and the peer prompts', () => {
  const h = hover();
  h.owner.hoverPrompt('negative', 0);
  expect(h.owner.hovered('regions')).toBe('regions0');
  expect(h.canvas.hover).toHaveBeenLastCalledWith('regions', 'regions0');
  expect(h.highlight).toHaveBeenLastCalledWith(['positive', 'negative'], 0);
  h.owner.hoverPrompt('negative', 2);
  expect(h.owner.hovered('regions')).toBeNull();
  expect(h.canvas.hover).toHaveBeenLastCalledWith('regions', null);
  expect(h.highlight).toHaveBeenLastCalledWith(['positive', 'negative'], 2);
  h.owner.hoverPrompt('unrelated', 0);
  expect(h.owner.hovered('regions')).toBeNull();
  expect(h.highlight).toHaveBeenLastCalledWith(['positive', 'negative'], null);
});

test('a clear from a view the pointer already left does not clear the newer hover', () => {
  const h = hover();
  h.owner.hoverPrompt('positive', 1);
  h.owner.hoverMask('regions', 'regions0');
  h.owner.hoverPrompt('positive', null);
  expect(h.owner.hovered('regions')).toBe('regions0');
  expect(h.canvas.hover).toHaveBeenLastCalledWith('regions', 'regions0');
  h.owner.hoverMask('regions', null);
  expect(h.owner.hovered('regions')).toBeNull();
  const late = jest.fn();
  h.owner.hoverMask('regions', 'regions1');
  h.owner.connect({ highlight: late });
  expect(late).toHaveBeenCalledWith(['positive', 'negative'], 1);
});

test('a hover whose loader leaves the workflow is cleared from every view', () => {
  const h = hover();
  h.owner.hoverMask('regions', 'regions0');
  h.g.change({ images: [], masks: [regional('regions', 2, prompts)] });
  expect(h.owner.hovered('regions')).toBe('regions0');
  h.g.change({ images: [], masks: [] });
  expect(h.owner.hovered('regions')).toBeNull();
  expect(h.canvas.hover).toHaveBeenLastCalledWith('regions', null);
  expect(h.highlight).toHaveBeenLastCalledWith(['positive', 'negative'], null);
  h.owner.dispose();
});
