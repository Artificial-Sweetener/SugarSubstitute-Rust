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
import { CubeSerializedDocumentReconciler } from '../../../frontend/comfyui/ui/cube/CubeSerializedDocumentReconciler.js';

describe('CubeSerializedDocumentReconciler', () => {
  test('drops a stale auxiliary document without rewriting its claimed identity', () => {
    const definition = {
      id: 'native-definition',
      extra: {
        sugarcubes_kind: 'cube',
        sugarcubes_cube: {
          cube_id: 'Artificial-Sweetener/Base-Cubes/Anima/Diffusion Upscale.cube',
          cube_version: '3.1.0',
        },
        sugarcubes_document: {
          cube_id: 'Artificial-Sweetener/Base-Cubes/Anima/Diffusion Upscale.cube',
          version: '2.3.0',
          implementation: {},
          surface: {},
          flavors: {},
        },
      },
    };
    const workflow = { definitions: { subgraphs: [definition] } };

    expect(new CubeSerializedDocumentReconciler().prepare(workflow)).toBe(1);
    expect(definition.extra).not.toHaveProperty('sugarcubes_document');
    expect(definition.extra.sugarcubes_cube).toMatchObject({ cube_version: '3.1.0' });
  });

  test('preserves an auxiliary document whose identity matches native state', () => {
    const document = {
      cube_id: 'local/personal/Test.cube',
      version: '1.0.0',
      implementation: {},
      surface: {},
      flavors: {},
    };
    const workflow = {
      definitions: {
        subgraphs: [
          {
            extra: {
              sugarcubes_kind: 'cube',
              sugarcubes_cube: {
                cube_id: document.cube_id,
                cube_version: document.version,
              },
              sugarcubes_document: document,
            },
          },
        ],
      },
    };

    expect(new CubeSerializedDocumentReconciler().prepare(workflow)).toBe(0);
    expect(
      (
        workflow.definitions.subgraphs[0].extra as {
          sugarcubes_document?: unknown;
        }
      ).sugarcubes_document,
    ).toBe(document);
  });
});
