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
import { rebindSubgraphWidgetValues } from '../../../frontend/comfyui/ui/graph/SubgraphWidgetValueRebinder.js';

describe('subgraph widget value rebinding', () => {
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

  test('keeps linked fields in the complete current widget layout', () => {
    const subgraph = {
      nodes: [
        {
          id: 146,
          type: 'NestedSamplerSubgraph',
          inputs: [
            { name: 'sampler_name', link: 272, widget: { name: 'sampler_name' } },
            { name: 'width', link: 273, widget: { name: 'width' } },
            { name: 'steps', link: 275, widget: { name: 'steps' } },
          ],
          widgets_values: [],
        },
      ],
    };

    rebindSubgraphWidgetValues(subgraph, () => ({
      widgets: [
        { name: 'sampler_name', value: 'euler' },
        { name: 'width', value: 1024 },
        { name: 'steps', value: 20 },
      ],
    }));

    expect(subgraph.nodes[0].widgets_values).toEqual(['euler', 1024, 20]);
  });

  test('restores authored subgraph input defaults by name after fields are reordered', () => {
    const subgraph = {
      links: [
        {
          id: 273,
          origin_id: -10,
          origin_slot: 0,
          target_id: 146,
          target_slot: 0,
          type: 'INT',
        },
      ],
      nodes: [
        {
          id: 146,
          type: 'EmptyLatentImage',
          inputs: [
            { name: 'width', link: 273, widget: { name: 'width' } },
            { name: 'batch_size', widget: { name: 'batch_size' } },
          ],
          widgets_values: [1080, 1],
        },
      ],
    };

    rebindSubgraphWidgetValues(subgraph, () => ({
      widgets: [
        { name: 'batch_size', value: 1 },
        { name: 'height', value: 512 },
        { name: 'width', value: 512 },
      ],
    }));

    expect(subgraph.nodes[0].widgets_values).toEqual([1, 512, 1080]);
  });

  test('uses linked defaults as anchors without shifting interleaved named values', () => {
    const subgraph = {
      nodes: [
        {
          id: 151,
          type: 'SimpleSyrup.ResizeImageToTarget',
          inputs: [
            { name: 'width', link: 3478, widget: { name: 'width' } },
            { name: 'height', link: 3482, widget: { name: 'height' } },
            { name: 'resize_mode', widget: { name: 'resize_mode' } },
            { name: 'sampling', link: 3213, widget: { name: 'sampling' } },
            { name: 'processor', widget: { name: 'processor' } },
            { name: 'divisible_by', widget: { name: 'divisible_by' } },
            { name: 'crop_position', widget: { name: 'crop_position' } },
          ],
          widgets_values: ['Keep AR', 'gpu', 2, 'center'],
        },
      ],
    };

    rebindSubgraphWidgetValues(subgraph, () => ({
      widgets: [
        { name: 'width', value: 512 },
        { name: 'height', value: 512 },
        { name: 'resize_mode', value: 'Crop' },
        { name: 'sampling', value: 'lanczos' },
        { name: 'processor', value: 'cpu' },
        { name: 'divisible_by', value: 8 },
        { name: 'crop_position', value: 'top' },
      ],
    }));

    expect(subgraph.nodes[0].widgets_values).toEqual([
      512,
      512,
      'Keep AR',
      'lanczos',
      'gpu',
      2,
      'center',
    ]);
  });

  test('discards stale linked values in favor of the current host default', () => {
    const subgraph = {
      nodes: [
        {
          id: 124,
          type: 'KSamplerSelect',
          inputs: [{ name: 'sampler_name', link: 267, widget: { name: 'sampler_name' } }],
          widgets_values: ['stale-saved-value'],
        },
      ],
    };

    rebindSubgraphWidgetValues(subgraph, () => ({
      widgets: [{ name: 'sampler_name', value: 'current-default' }],
    }));

    expect(subgraph.nodes[0].widgets_values).toEqual(['current-default']);
  });
});
