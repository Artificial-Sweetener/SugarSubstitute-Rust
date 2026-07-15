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

import { describe, expect, test } from '@jest/globals';
import {
  WORKFLOW_WIDGET_VALUES_KEY,
  attachWorkflowWidgetSnapshots,
  captureNodeWidgetValues,
  rebindSubgraphWidgetValues,
} from '../../frontend/comfyui/ui/graph/WidgetSnapshots.js';

describe('widget snapshots', () => {
  test('captures serializable live widgets by stable name', () => {
    const node = {
      id: 42,
      widgets: [
        { name: 'steps', value: 30 },
        { name: 'cfg', value: 7.0 },
        { name: 'Create cube', value: null, type: 'button' },
        { name: 'decoration', value: 'ignored', options: { serialize: false } },
      ],
    };

    expect(captureNodeWidgetValues(node)).toEqual({ steps: 30, cfg: 7.0 });
  });

  test('attaches snapshots to workflow nodes without changing cube schema data', () => {
    const workflow: { nodes: Array<Record<string, unknown>> } = {
      nodes: [{ id: 42, type: 'KSampler' }],
    };
    const graph = {
      _nodes: [
        {
          id: 42,
          widgets: [
            { name: 'steps', value: 30 },
            { name: 'sampler_name', value: 'euler' },
          ],
        },
      ],
    };

    attachWorkflowWidgetSnapshots(workflow, graph);

    expect(workflow.nodes[0][WORKFLOW_WIDGET_VALUES_KEY]).toEqual({
      steps: 30,
      sampler_name: 'euler',
    });
  });

  test('rejects duplicate widget identities instead of shifting values', () => {
    const node = {
      id: 42,
      widgets: [
        { name: 'cfg', value: 7 },
        { name: 'cfg', value: 8 },
      ],
    };

    expect(() => captureNodeWidgetValues(node)).toThrow("duplicate widget name 'cfg'");
  });

  test('rebinds reordered widgets and leaves added widgets at current defaults', () => {
    const subgraph = {
      nodes: [
        {
          id: 7,
          type: 'Sampler',
          inputs: [
            { name: 'steps', widget: { name: 'steps' } },
            { name: 'method', widget: { name: 'method' } },
          ],
          widgets_values: [30, 'dpmpp'],
        },
      ],
    };

    rebindSubgraphWidgetValues(subgraph, () => ({
      widgets: [
        { name: 'method', value: 'euler' },
        { name: 'cfg', value: 7 },
        { name: 'steps', value: 20 },
      ],
    }));

    expect(subgraph.nodes[0].widgets_values).toEqual(['dpmpp', 7, 30]);
  });

  test('ignores removed and renamed values instead of assigning them by position', () => {
    const subgraph = {
      nodes: [
        {
          id: 7,
          type: 'Sampler',
          inputs: [
            { name: 'removed', widget: { name: 'removed' } },
            { name: 'old_name', widget: { name: 'old_name' } },
          ],
          widgets_values: [99, 'old'],
        },
      ],
    };

    rebindSubgraphWidgetValues(subgraph, () => ({
      widgets: [{ name: 'new_name', value: 'current-default' }],
    }));

    expect(subgraph.nodes[0].widgets_values).toEqual(['current-default']);
  });

  test('uses current local resource defaults for null persisted placeholders', () => {
    const subgraph = {
      nodes: [
        {
          id: 7,
          type: 'CheckpointLoaderSimple',
          inputs: [{ name: 'ckpt_name', widget: { name: 'ckpt_name' } }],
          widgets_values: [null],
        },
      ],
    };

    rebindSubgraphWidgetValues(subgraph, () => ({
      widgets: [{ name: 'ckpt_name', value: 'local-default.safetensors' }],
    }));

    expect(subgraph.nodes[0].widgets_values).toEqual(['local-default.safetensors']);
  });

  test('reconstructs current non-serialized companion widgets without shifting values', () => {
    const subgraph = {
      nodes: [
        {
          id: 7,
          type: 'KSampler',
          inputs: [
            { name: 'seed', widget: { name: 'seed' } },
            { name: 'steps', widget: { name: 'steps' } },
          ],
          widgets_values: [null, 30],
        },
      ],
    };

    rebindSubgraphWidgetValues(subgraph, () => ({
      widgets: [
        { name: 'seed', value: 99 },
        {
          name: 'control_after_generate',
          value: 'randomize',
          options: { serialize: false },
        },
        { name: 'steps', value: 20 },
      ],
    }));

    expect(subgraph.nodes[0].widgets_values).toEqual([99, 'randomize', 30]);
  });
});
