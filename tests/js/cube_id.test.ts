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
  deriveCubeIdFromDefaultAlias,
  deriveSourceAuthorLabel,
  normalizeDefaultAliasTitle,
  parseCanonicalCubeId,
  suggestCanonicalCubePath,
} from '../../frontend/comfyui/ui/core/CubeId.js';

describe('CubeId identity derivation', () => {
  test.each([
    ['text to image', 'Text to Image'],
    ['diffusion upscale', 'Diffusion Upscale'],
    ['automask detailer', 'Automask Detailer'],
    ['Text to Image', 'Text to Image'],
    [' text to image ', 'Text to Image'],
    ['text_to_image', 'text_to_image'],
    ['sdxl lora image', 'SDXL LoRA Image'],
    ['IPAdapter helper', 'IPAdapter Helper'],
    ['Flux .1 D', 'Flux .1 D'],
  ])('normalizes authored display title %s', (value, expected) => {
    expect(normalizeDefaultAliasTitle(value)).toBe(expected);
  });

  test('derives a GitHub cube id from a default alias at source root', () => {
    expect(
      deriveCubeIdFromDefaultAlias(
        'Artificial-Sweetener/Base-Cubes/text to image.cube',
        'Text to Image XL',
      ),
    ).toBe('Artificial-Sweetener/Base-Cubes/Text to Image XL.cube');
  });

  test('preserves parent folders when deriving a GitHub cube id', () => {
    expect(
      deriveCubeIdFromDefaultAlias(
        'Artificial-Sweetener/Base-Cubes/generation/text_to_image.cube',
        'Text to Image XL',
      ),
    ).toBe('Artificial-Sweetener/Base-Cubes/generation/Text to Image XL.cube');
  });

  test('derives a local cube id from a default alias', () => {
    expect(
      deriveCubeIdFromDefaultAlias('local/personal/text_to_image.cube', 'Text to Image XL'),
    ).toBe('local/personal/Text to Image XL.cube');
  });

  test('uses the fallback cube filename for empty default aliases', () => {
    expect(suggestCanonicalCubePath('   ')).toBe('cube.cube');
    expect(deriveCubeIdFromDefaultAlias('local/personal/text_to_image.cube', '   ')).toBe(
      'local/personal/cube.cube',
    );
  });

  test('preserves exact spaces and underscores in default aliases', () => {
    expect(suggestCanonicalCubePath('  Text to Image XL  ')).toBe('Text to Image XL.cube');
    expect(suggestCanonicalCubePath('  text_to_image  ')).toBe('text_to_image.cube');
    expect(suggestCanonicalCubePath('Diffusion Upscale.cube')).toBe('Diffusion Upscale.cube');
  });

  test('rejects unsafe filename text instead of rewriting it', () => {
    expect(() => suggestCanonicalCubePath('Bad/Name')).toThrow(/filename/i);
    expect(() => suggestCanonicalCubePath('Bad\\Name')).toThrow(/filename/i);
    expect(() => suggestCanonicalCubePath('Bad:Name')).toThrow(/filename/i);
  });

  test('derives source author labels from canonical cube ids', () => {
    expect(deriveSourceAuthorLabel('Artificial-Sweetener/Base-Cubes/text to image.cube')).toBe(
      'Artificial-Sweetener/Base-Cubes',
    );
    expect(deriveSourceAuthorLabel('local/personal/text_to_image.cube')).toBe('local');
  });

  test('rejects invalid current cube ids during derivation', () => {
    expect(() => deriveCubeIdFromDefaultAlias('invalid', 'Demo')).toThrow(/canonical/);
  });

  test('reserves flavors as a source root while allowing flavors.cube filenames', () => {
    expect(() => parseCanonicalCubeId('local/flavors/foo.cube')).toThrow(/reserved/);
    expect(() => parseCanonicalCubeId('flavors/base-cubes/foo.cube')).toThrow(/reserved/);
    expect(parseCanonicalCubeId('local/personal/flavors.cube').path).toBe('flavors.cube');
  });
});
