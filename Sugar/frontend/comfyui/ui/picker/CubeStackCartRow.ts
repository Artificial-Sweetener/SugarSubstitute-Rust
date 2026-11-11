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
/** Project shared Cube identity into immutable cart and rail row metadata. */
import { resolveCubeIdentityPresentation } from '../cube/CubeIdentityPresentation.js';
import type { UnknownRecord } from '../types/common.js';
import type { CubeStackRowModel } from '../surface/stack/CubeStackView.js';

/** Share normal Cube title, source and icon semantics with the rail and native node chrome. */
export function cubeStackCartRow(
  id: string,
  metadata: UnknownRecord,
  label: string,
  bypassed: boolean,
): CubeStackRowModel {
  const identity = resolveCubeIdentityPresentation({
    metadata,
    instanceTitle: label,
    fallbackDefinitionTitle: label,
  });
  return {
    id,
    label: identity.instanceTitle,
    secondary: [identity.versionText, identity.sourceLine].filter(Boolean).join(' · '),
    tooltip: [identity.instanceTitle, identity.definitionLine, identity.sourceLine]
      .filter(Boolean)
      .join('\n'),
    icon: identity.icon,
    targetModel: identity.targetModel,
    bypassed,
    selected: false,
    severity: identity.awaitingFirstSave ? 'warning' : 'none',
  };
}
