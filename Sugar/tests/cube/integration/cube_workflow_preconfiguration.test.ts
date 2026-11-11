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
/** Verify graph-independent Cube workflow pre-configuration. */

import { jest } from '@jest/globals';

import { CubeWorkflowPreconfiguration } from '../../../frontend/comfyui/ui/cube/CubeWorkflowPreconfiguration.js';
import type { LegacyCubeWorkflowExtractor } from '../../../frontend/comfyui/ui/cube/migration/LegacyCubeWorkflowExtractor.js';
import type { CubeSerializedDefinitionPresentationAdapter } from '../../../frontend/comfyui/ui/cube/CubeSerializedDefinitionPresentationAdapter.js';
import { CubeSerializedWidgetRehydrator } from '../../../frontend/comfyui/ui/cube/CubeSerializedWidgetRehydrator.js';

describe('CubeWorkflowPreconfiguration', () => {
  test('transfers legacy extraction exactly once without registering node types', () => {
    const batch = { plans: [{ key: 'legacy' }], connections: [], warnings: [] };
    const extractInPlace = jest.fn(() => batch);
    const prepareDefinitions = jest.fn(() => 1);
    const service = new CubeWorkflowPreconfiguration(
      { extractInPlace } as unknown as LegacyCubeWorkflowExtractor,
      { prepare: prepareDefinitions } as unknown as CubeSerializedDefinitionPresentationAdapter,
    );
    const workflow = {};

    expect(service.prepare(workflow)).toBe(1);
    expect(prepareDefinitions).toHaveBeenCalledWith(workflow);
    expect(extractInPlace).toHaveBeenCalledWith(workflow);
    expect(prepareDefinitions.mock.invocationCallOrder[0]).toBeLessThan(
      extractInPlace.mock.invocationCallOrder[0]!,
    );
    expect(service.takeLegacyBatch()).toBe(batch);
    expect(service.takeLegacyBatch()).toBeNull();
  });

  test('rehydrates saved Cube widgets by name before Comfy configuration', () => {
    let classifiedWorkflow: unknown;
    const classify = jest.fn((value: unknown) => {
      classifiedWorkflow = JSON.parse(JSON.stringify(value)) as unknown;
    });
    const workflow = savedCubeWorkflow();
    const rehydrator = new CubeSerializedWidgetRehydrator((type) => {
      if (type === 'SimpleSyrup.SimpleLoadAnima') {
        return {
          widgets: [
            { name: 'diffusion_model', value: 'current-model.safetensors' },
            { name: 'quantization', value: 'Original' },
            { name: 'diffusion_weight_dtype', value: 'default' },
            { name: 'text_encoder', value: 'auto' },
            { name: 'text_encoder_device', value: 'default' },
            { name: 'vae', value: 'auto' },
          ],
        };
      }
      if (type === 'SimpleSyrup.LoadMaskBatch') {
        return {
          widgets: [
            { name: 'image', value: [] },
            { name: 'channel', value: 'alpha' },
            { name: 'upload', value: 'image', options: { serialize: false } },
            {
              name: 'simple_syrup_replace_media',
              value: 'ordered_media',
              options: { serialize: false },
            },
            {
              name: 'simple_syrup_add_media',
              value: 'ordered_media',
              options: { serialize: false },
            },
          ],
        };
      }
      return null;
    });
    const service = new CubeWorkflowPreconfiguration(
      {
        extractInPlace: () => ({ plans: [], connections: [], warnings: [] }),
      } as unknown as LegacyCubeWorkflowExtractor,
      { prepare: () => 1 } as unknown as CubeSerializedDefinitionPresentationAdapter,
      null,
      { begin: classify } as never,
      rehydrator,
    );

    service.prepare(workflow);

    const node = workflow.definitions.subgraphs[0]?.nodes[0];
    expect(node?.widgets_values).toEqual([
      'Anima/model.safetensors',
      'Original',
      'default',
      'auto',
      'default',
      'auto',
    ]);
    expect(workflow.definitions.subgraphs[0]?.nodes[1]?.widgets_values).toEqual([
      ['ComfyUI_temp_xfbcy_00001_.png', 'ComfyUI_temp_pkslb_00001_.png'],
      'alpha',
      'image',
      'ordered_media',
      'ordered_media',
    ]);
    expect(classify).toHaveBeenCalledTimes(1);
    expect(
      (classifiedWorkflow as ReturnType<typeof savedCubeWorkflow>).definitions.subgraphs[0]
        ?.nodes[0]?.widgets_values,
    ).toEqual(['Anima/model.safetensors', 'default', 'auto', 'default', 'auto', 'auto']);
  });

  test('keeps current serialized widget identities authoritative after save and reopen', () => {
    const workflow = savedCubeWorkflow();
    const definition = workflow.definitions.subgraphs[0]!;
    definition.nodes[0]!.inputs = [
      serializedWidgetInput('diffusion_model'),
      serializedWidgetInput('quantization'),
      serializedWidgetInput('diffusion_weight_dtype'),
      serializedWidgetInput('text_encoder'),
      serializedWidgetInput('text_encoder_device'),
      serializedWidgetInput('vae'),
    ];
    definition.nodes[0]!.widgets_values = [
      'Anima/model.safetensors',
      'Original',
      'default',
      'auto',
      'default',
      'auto',
    ];
    const rehydrator = new CubeSerializedWidgetRehydrator((type) =>
      type === 'SimpleSyrup.SimpleLoadAnima'
        ? {
            widgets: [
              { name: 'diffusion_model', value: 'current-model.safetensors' },
              { name: 'quantization', value: 'Original' },
              { name: 'diffusion_weight_dtype', value: 'default' },
              { name: 'text_encoder', value: 'auto' },
              { name: 'text_encoder_device', value: 'default' },
              { name: 'vae', value: 'auto' },
            ],
          }
        : null,
    );

    rehydrator.prepare(workflow);

    expect(definition.nodes[0]?.widgets_values).toEqual([
      'Anima/model.safetensors',
      'Original',
      'default',
      'auto',
      'default',
      'auto',
    ]);
  });
});

/** Build one Comfy-serialized widget input carrying its stable identity. */
function serializedWidgetInput(name: string) {
  return { name, type: 'COMBO', widget: { name }, link: null };
}

/** Build a pre-quantization Cube snapshot carrying stable widget identities. */
function savedCubeWorkflow() {
  return {
    nodes: [],
    definitions: {
      subgraphs: [
        {
          id: 'cube-definition',
          extra: {
            sugarcubes_kind: 'cube',
            sugarcubes_cube: {
              cube_id: 'Artificial-Sweetener/Base-Cubes/Anima/Prompt by Region.cube',
              definitions: {
                'SimpleSyrup.SimpleLoadAnima': {
                  input: {
                    required: {
                      diffusion_model: ['LIST'],
                      diffusion_weight_dtype: ['LIST'],
                      text_encoder: ['LIST'],
                      text_encoder_device: ['LIST'],
                      vae: ['LIST'],
                    },
                  },
                  input_order: {
                    required: [
                      'diffusion_model',
                      'diffusion_weight_dtype',
                      'text_encoder',
                      'text_encoder_device',
                      'vae',
                    ],
                  },
                },
                'SimpleSyrup.LoadMaskBatch': {
                  input: {
                    required: {
                      image: ['LIST'],
                      channel: ['LIST'],
                    },
                  },
                  input_order: { required: ['image', 'channel'] },
                },
              },
            },
          },
          nodes: [
            {
              id: 'models',
              type: 'SimpleSyrup.SimpleLoadAnima',
              inputs: [] as ReturnType<typeof serializedWidgetInput>[],
              widgets_values: [
                'Anima/model.safetensors',
                'default',
                'auto',
                'default',
                'auto',
                'auto',
              ],
            },
            {
              id: 'load-mask',
              type: 'SimpleSyrup.LoadMaskBatch',
              inputs: [] as ReturnType<typeof serializedWidgetInput>[],
              widgets_values: [
                ['ComfyUI_temp_xfbcy_00001_.png', 'ComfyUI_temp_pkslb_00001_.png'],
                'alpha',
                'image',
                'ordered_media',
                'ordered_media',
              ],
            },
          ],
          links: [],
        },
      ],
    },
  };
}
