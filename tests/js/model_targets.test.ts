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
  deriveCubeIdFromRoute,
  deriveFilenameFromRoute,
  deriveRouteFromCubeId,
  deriveTargetModelCubeId,
  deriveTargetModelFromCubeId,
  deriveTargetModelFromRoute,
  defaultSupportedModelsForTarget,
  normalizeCubeRoute,
  normalizeSupportedModels,
  normalizeTargetModel,
  TARGET_MODEL_OPTIONS,
  validateCubeRouteIdentity,
} from '../../web/comfyui/ui/core/ModelTargets.js';

describe('ModelTargets helpers', () => {
  test('normalizes path-safe target model labels', () => {
    expect(normalizeTargetModel('  SDXL   ')).toBe('SDXL');
    expect(normalizeTargetModel('sdxl 1.0')).toBe('SDXL');
    expect(normalizeTargetModel('Wan  Video')).toBe('Wan Video');
  });

  test.each(['Bad/Model', 'Bad\\Model', 'Bad:Model', 'Bad.'])(
    'rejects path-unsafe target model %s',
    (value) => {
      expect(() => normalizeTargetModel(value)).toThrow(/target model/i);
    },
  );

  test('normalizes supported models with target first', () => {
    expect(normalizeSupportedModels(['SD 1.5', 'SDXL'], { targetModel: 'SDXL' })).toEqual([
      'SDXL',
      'SD 1.5',
    ]);
  });

  test('collapses supported model family aliases', () => {
    expect(
      normalizeSupportedModels(['SDXL 1.0', 'SD 1.5', 'sdxl 1.0'], {
        targetModel: 'SDXL',
      }),
    ).toEqual(['SDXL', 'SD 1.5']);
  });

  test('does not force Any into supported models', () => {
    expect(normalizeSupportedModels('SDXL, Flux', { targetModel: 'Any' })).toEqual([
      'SDXL',
      'Flux',
    ]);
  });

  test('returns authoring defaults for known target models', () => {
    expect(defaultSupportedModelsForTarget('SDXL')).toEqual(['SDXL', 'SD 1.5']);
    expect(defaultSupportedModelsForTarget('Flux')).toEqual(['Flux']);
    expect(defaultSupportedModelsForTarget('Any')).toEqual([]);
  });

  test('offers Anima as a first-class authoring target', () => {
    expect(TARGET_MODEL_OPTIONS).toContain('Anima');
    expect(defaultSupportedModelsForTarget('Anima')).toEqual(['Anima']);
  });

  test('offers SeedVR2 as a first-class authoring target', () => {
    expect(TARGET_MODEL_OPTIONS).toContain('SeedVR2');
    expect(defaultSupportedModelsForTarget('SeedVR2')).toEqual(['SeedVR2']);
  });

  test('derives route from source-relative path without extension', () => {
    expect(deriveRouteFromCubeId('Artificial-Sweetener/Base-Cubes/SDXL/Text to Image.cube')).toBe(
      'SDXL/Text to Image',
    );
    expect(deriveRouteFromCubeId('local/personal/Flux/Image to Image.cube')).toBe(
      'Flux/Image to Image',
    );
  });

  test('normalizes cube routes', () => {
    expect(normalizeCubeRoute('  SDXL / Text   to Image.cube ')).toBe('SDXL/Text to Image');
    expect(() => normalizeCubeRoute('SDXL//Text')).toThrow(/route/i);
  });

  test('derives target model from first route segment', () => {
    expect(
      deriveTargetModelFromCubeId('Artificial-Sweetener/Base-Cubes/Tools/Mask/Inpaint.cube'),
    ).toBe('Tools');
    expect(deriveTargetModelFromRoute('SDXL/Text to Image')).toBe('SDXL');
    expect(deriveTargetModelFromRoute('Text to Image')).toBe('');
  });

  test('derives filename and cube id from route', () => {
    expect(deriveFilenameFromRoute('SDXL/Text to Image')).toBe('Text to Image.cube');
    expect(
      deriveCubeIdFromRoute({
        sourceCubeId: 'Artificial-Sweetener/Base-Cubes/Old.cube',
        route: 'SDXL/Text to Image',
      }),
    ).toBe('Artificial-Sweetener/Base-Cubes/SDXL/Text to Image.cube');
  });

  test('derives target model cube ids', () => {
    expect(
      deriveTargetModelCubeId({
        sourceCubeId: 'Artificial-Sweetener/Base-Cubes/Text to Image.cube',
        targetModel: 'SDXL',
        defaultAlias: 'Text to Image',
      }),
    ).toBe('Artificial-Sweetener/Base-Cubes/SDXL/Text to Image.cube');
    expect(
      deriveTargetModelCubeId({
        sourceCubeId: 'local/personal/Flux/Old.cube',
        targetModel: 'SDXL',
        defaultAlias: 'Image to Image',
      }),
    ).toBe('local/personal/SDXL/Image to Image.cube');
  });

  test('validates route identity', () => {
    expect(() =>
      validateCubeRouteIdentity(
        'Artificial-Sweetener/Base-Cubes/SDXL/Text to Image.cube',
        'SDXL/Text to Image',
      ),
    ).not.toThrow();
    expect(() =>
      validateCubeRouteIdentity(
        'Artificial-Sweetener/Base-Cubes/SDXL/Text to Image.cube',
        'Text to Image',
      ),
    ).toThrow(/route/i);
  });
});
