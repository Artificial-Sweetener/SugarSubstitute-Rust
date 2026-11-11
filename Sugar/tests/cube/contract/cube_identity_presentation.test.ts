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
/** Verify shared Cube identity preserves model metadata and compact version presentation. */

import {
  formatCubeVersionText,
  resolveCubeIdentityPresentation,
} from '../../../frontend/comfyui/ui/cube/CubeIdentityPresentation.js';

describe('resolveCubeIdentityPresentation', () => {
  test('separates the instance title from the centered definition and normalized version', () => {
    const identity = resolveCubeIdentityPresentation({
      metadata: {
        cube_id: 'Artificial-Sweetener/Base-Cubes/Text to Image.cube',
        default_alias: 'SDXL/Text to Image',
        target_model: 'SDXL',
        instance_alias: 'Hero image',
        cube_version: 'v2.3.1',
      },
      instanceTitle: 'Hero image',
      fallbackDefinitionTitle: 'Text to Image',
    });

    expect(identity.instanceTitle).toBe('Hero image');
    expect(identity.definitionTitle).toBe('SDXL/Text to Image');
    expect(identity.targetModel).toBe('SDXL');
    expect(identity.versionText).toBe('v2.3.1');
    expect(identity.definitionLine).toBe('SDXL/Text to Image v2.3.1');
    expect(identity.definitionModelTitle).toMatchObject({
      modelText: 'SDXL',
      nameText: 'Text to Image',
      suffixText: 'v2.3.1',
      usesModelPill: true,
    });
    expect(identity.instanceModelTitle).toMatchObject({
      modelText: 'SDXL',
      nameText: 'Hero image',
      usesModelPill: true,
    });
    expect(identity.awaitingFirstSave).toBe(false);
    expect(identity.sourceLine).toBe('from Base-Cubes by Artificial-Sweetener');
    expect(identity.icon.kind).toBe('initials');
    expect(identity.icon.initials).toBe('TI');
  });

  test('describes personal Cubes naturally without treating later changes as a first save', () => {
    const identity = resolveCubeIdentityPresentation({
      metadata: {
        cube_id: 'local/personal/Detailer.cube',
        default_alias: 'Detailer',
        target_model: 'SDXL',
        has_saveable_changes: true,
      },
      instanceTitle: 'Detailer',
      fallbackDefinitionTitle: 'Fallback',
    });

    expect(identity.definitionLine).toBe('SDXL Detailer');
    expect(identity.definitionModelTitle.usesModelPill).toBe(true);
    expect(identity.awaitingFirstSave).toBe(false);
    expect(identity.sourceLine).toBe('Personal Cube');
  });

  test('labels a workflow-only Cube draft as unsaved', () => {
    const identity = resolveCubeIdentityPresentation({
      metadata: { default_alias: 'Untitled Cube' },
      instanceTitle: 'Untitled Cube',
      fallbackDefinitionTitle: 'Untitled Cube',
    });

    expect(identity.awaitingFirstSave).toBe(true);
    expect(identity.sourceLine).toBe('Workflow only');
  });

  test('retains the definition asset icon used by former group chrome', () => {
    const identity = resolveCubeIdentityPresentation({
      metadata: {
        cube_id: 'demo',
        default_alias: 'Detailer',
        icon: {
          kind: 'asset',
          url: '/sugarcubes/assets/icon?cube_id=demo',
          media_type: 'image/png',
        },
      },
      instanceTitle: 'Detailer',
      fallbackDefinitionTitle: 'Fallback',
    });

    expect(identity.icon).toMatchObject({
      kind: 'asset',
      url: '/sugarcubes/assets/icon?cube_id=demo',
      initials: 'DE',
    });
    expect(identity.sourceLine).toBe('Unknown source');
    expect(identity.awaitingFirstSave).toBe(false);
  });

  test('projects transient wild classification as an icon state without changing source text', () => {
    const identity = resolveCubeIdentityPresentation({
      metadata: {
        cube_id: 'Artificial-Sweetener/Base-Cubes/Text to Image.cube',
        default_alias: 'Text to Image',
      },
      instanceTitle: 'Text to Image',
      fallbackDefinitionTitle: 'Text to Image',
      fallbackSource: { libraryClass: 'none' },
    });

    expect(identity.sourceLine).toBe('from Base-Cubes by Artificial-Sweetener');
    expect(identity.isWild).toBe(true);
    expect(identity.isCaptured).toBe(false);
  });

  test('projects transient captured classification without changing source text', () => {
    const identity = resolveCubeIdentityPresentation({
      metadata: {
        cube_id: 'Artificial-Sweetener/Base-Cubes/Text to Image.cube',
        default_alias: 'Text to Image',
      },
      instanceTitle: 'Text to Image',
      fallbackDefinitionTitle: 'Text to Image',
      fallbackSource: { libraryClass: 'captured' },
    });

    expect(identity.sourceLine).toBe('from Base-Cubes by Artificial-Sweetener');
    expect(identity.isWild).toBe(false);
    expect(identity.isCaptured).toBe(true);
  });
});

test.each([
  ['3.1.0', 'v3.1.0'],
  [' v3.1.0 ', 'v3.1.0'],
  ['V3.1.0', 'V3.1.0'],
  ['', ''],
  [null, ''],
  [42, ''],
])('formats version %p as %p without changing persisted metadata', (value, expected) => {
  const metadata = { cube_version: value };
  expect(formatCubeVersionText(metadata)).toBe(expected);
  expect(metadata.cube_version).toBe(value);
});

test('retains a declared model after instance rename and rejects non-text model metadata', () => {
  for (const target of ['  SDXL  ', null, { name: 'SDXL' }]) {
    const identity = resolveCubeIdentityPresentation({
      metadata: { target_model: target },
      instanceTitle: 'Renamed cube',
      fallbackDefinitionTitle: 'SDXL/Detailer',
    });
    expect(identity.targetModel).toBe(typeof target === 'string' ? 'SDXL' : '');
    expect(identity.instanceModelTitle.usesModelPill).toBe(typeof target === 'string');
    expect(identity.instanceModelTitle.nameText).toBe('Renamed cube');
  }
});

test.each(['Anima/Contextual Upscale 2', 'Final polish', '<img src=x>'])(
  'keeps stored identity intact for %s',
  (instanceTitle) => {
    const metadata = Object.freeze({
      target_model: 'Anima',
      default_alias: 'Anima/Contextual Upscale',
      cube_version: '1.1.0',
    });
    const identity = resolveCubeIdentityPresentation({
      metadata,
      instanceTitle,
      fallbackDefinitionTitle: 'Fallback',
    });
    expect(identity.instanceTitle).toBe(instanceTitle);
    expect(identity.instanceModelTitle.modelText).toBe('Anima');
    expect(identity.definitionTitle).toBe(metadata.default_alias);
    expect(identity.versionText).toBe('v1.1.0');
  },
);
