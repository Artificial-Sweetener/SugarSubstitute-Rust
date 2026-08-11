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
/** Verify legacy group metadata becomes native Cube instance metadata. */

import { mapLegacyCubeIdentity } from '../../../frontend/comfyui/ui/cube/migration/LegacyCubeIdentityMapper.js';
import type { LegacyCubePlan } from '../../../frontend/comfyui/ui/cube/migration/LegacyCubeWorkflowExtractor.js';

test('retains structured definition, instance, and surface metadata', () => {
  const identity = mapLegacyCubeIdentity({
    key: 'instance-9',
    cubeId: '',
    cubeVersion: '',
    title: 'Fallback title',
    metadata: {
      definition: {
        cube_id: 'local/personal/Detailer.cube',
        cube_version: '1.2.3',
        default_alias: 'Detailer',
      },
      instance: { instance_alias: 'My Detailer' },
      surface_state: { schema: 1, revealed: { sampler: true } },
      target_model: 'SDXL',
    },
    position: [0, 0],
    size: [720, 480],
    nodes: [],
    groups: [],
    internalLinks: [],
    inputs: [],
    outputs: [],
  } satisfies LegacyCubePlan);

  expect(identity).toMatchObject({
    cubeId: 'local/personal/Detailer.cube',
    cubeVersion: '1.2.3',
    instanceId: 'instance-9',
    defaultAlias: 'Detailer',
    instanceAlias: 'My Detailer',
    metadata: {
      target_model: 'SDXL',
      surface_state: { schema: 1, revealed: { sampler: true } },
    },
  });
});
