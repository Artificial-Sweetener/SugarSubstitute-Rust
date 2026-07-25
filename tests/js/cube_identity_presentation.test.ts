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
/** Verify Cube identity chrome retains the former group-chrome semantics. */

import { resolveCubeIdentityPresentation } from '../../frontend/comfyui/ui/cube/CubeIdentityPresentation.js';

describe('resolveCubeIdentityPresentation', () => {
  test('separates the instance title from the centered definition and normalized version', () => {
    const identity = resolveCubeIdentityPresentation({
      metadata: {
        cube_id: 'Artificial-Sweetener/Base-Cubes/Text to Image.cube',
        default_alias: 'SDXL/Text to Image',
        instance_alias: 'Hero image',
        cube_version: 'v2.3.1',
      },
      instanceTitle: 'Hero image',
      fallbackDefinitionTitle: 'Text to Image',
    });

    expect(identity.instanceTitle).toBe('Hero image');
    expect(identity.definitionTitle).toBe('SDXL/Text to Image');
    expect(identity.versionText).toBe('version 2.3.1');
    expect(identity.definitionLine).toBe('SDXL/Text to Image version 2.3.1');
    expect(identity.sourceLine).toBe('from Base-Cubes by Artificial-Sweetener');
    expect(identity.icon.kind).toBe('initials');
    expect(identity.icon.initials).toBe('TI');
  });

  test('retains the former local namespace source line', () => {
    const identity = resolveCubeIdentityPresentation({
      metadata: {
        cube_id: 'local/personal/Detailer.cube',
        default_alias: 'Detailer',
      },
      instanceTitle: 'Detailer',
      fallbackDefinitionTitle: 'Fallback',
    });

    expect(identity.definitionLine).toBe('Detailer');
    expect(identity.sourceLine).toBe('from local personal');
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
    expect(identity.sourceLine).toBe('from Unknown');
  });
});
