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
/** Present architecture and family assignments through the shared masonry context menu. */
import type { ContextMenuEntry } from '../menus/ContextMenu.js';
import type { ModelCatalogSnapshot } from './ModelCatalog.js';
import type { ModelPickerBinding } from './ModelPickerBinding.js';
import { isCubeNode } from '../cube/node/ComfyCubeNodeFactory.js';
import { locateModelGraph } from './ComfyModelGraph.js';
import { assignCubeModelRequirement, cubeModelRequirement } from './CubeModelRequirements.js';
import type { ModelRequirement } from '../cube/ModelRequirement.js';
import { isScopedModelKind } from './ModelPickerCompatibilityScope.js';

/** Offer one portable scope owner only on Cube-face masonry, preserving loose node behavior. */
export function modelScopeMenu(
  app: unknown,
  binding: ModelPickerBinding,
  snapshot: ModelCatalogSnapshot,
  changed: () => void,
): readonly ContextMenuEntry[] {
  if (
    !isScopedModelKind(binding.kind) ||
    !binding.anchor.closest('.sugarcubes-cube-face') ||
    !isCubeNode(locateModelGraph(app, binding)?.scope.owner)
  )
    return [];
  const current = cubeModelRequirement(app, binding);
  const families = snapshot.relationships?.families.filter((family) => family.architecture) ?? [];
  const architectures = [
    ...new Set([
      ...families.map((family) => family.architecture),
      ...snapshot.items.flatMap((item) => (item.architecture ? [item.architecture.base] : [])),
    ]),
  ].sort();
  /** Apply policy before notifying the field owner to resolve installed selections. */
  const assign = (requirement: ModelRequirement): void => {
    assignCubeModelRequirement(app, binding, requirement);
    changed();
  };
  return [
    {
      label: 'Cube models',
      children: [
        {
          label: 'Unfiltered',
          checked: current?.architecture === '' && current.family === '',
          select: () => assign({ architecture: '', family: '' }),
        },
        ...architectures.map((architecture) => ({
          label: architecture,
          children: [
            {
              label: `All ${architecture} models`,
              checked: current?.architecture === architecture && !current.family,
              select: () => assign({ architecture, family: '' }),
            },
            ...families
              .filter((family) => family.architecture === architecture)
              .map((family) => ({
                label: family.name,
                checked: current?.family === family.key,
                select: () => assign({ architecture, family: family.key }),
              })),
          ],
        })),
      ],
    },
  ];
}
