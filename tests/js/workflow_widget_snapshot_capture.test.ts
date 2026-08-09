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
} from '../../frontend/comfyui/ui/graph/WorkflowWidgetSnapshotCapture.js';

describe('workflow widget snapshot capture', () => {
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

  test('attaches snapshots to live subgraph nodes by definition id', () => {
    const workflow: {
      nodes: Array<Record<string, unknown>>;
      definitions: {
        subgraphs: Array<{ id: string; nodes: Array<Record<string, unknown>> }>;
      };
    } = {
      nodes: [],
      definitions: {
        subgraphs: [
          {
            id: 'cube-definition',
            nodes: [
              {
                id: 124,
                type: 'KSamplerSelect',
                inputs: [
                  {
                    name: 'sampler_name',
                    link: 267,
                    widget: { name: 'sampler_name' },
                  },
                ],
                widgets_values: ['euler'],
              },
              {
                id: 125,
                type: 'UNETLoader',
                inputs: [],
                widgets_values: ['model.safetensors', 'default'],
              },
            ],
          },
        ],
      },
    };
    const graph = {
      _nodes: [],
      _subgraphs: new Map([
        [
          'cube-definition',
          {
            _nodes: [
              { id: 124, widgets: [{ name: 'sampler_name', value: 'euler' }] },
              {
                id: 125,
                widgets: [
                  { name: 'unet_name', value: 'model.safetensors' },
                  { name: 'weight_dtype', value: 'default' },
                ],
              },
            ],
          },
        ],
      ]),
    };

    attachWorkflowWidgetSnapshots(workflow, graph);

    expect(workflow.definitions.subgraphs[0].nodes[0][WORKFLOW_WIDGET_VALUES_KEY]).toEqual({
      sampler_name: 'euler',
    });
    expect(workflow.definitions.subgraphs[0].nodes[1][WORKFLOW_WIDGET_VALUES_KEY]).toEqual({
      unet_name: 'model.safetensors',
      weight_dtype: 'default',
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
});
