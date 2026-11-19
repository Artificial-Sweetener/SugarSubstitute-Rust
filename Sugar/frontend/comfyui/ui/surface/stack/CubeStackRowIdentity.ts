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
/** Say how one Cube reads on a stack row, wherever that row is shown. */
import type { CubeIdentityPresentation } from '../../cube/CubeIdentityPresentation.js';
import type { CubeStackRowModel } from './CubeStackView.js';

/** The part of a stack row that comes from the Cube's identity alone. */
export type CubeStackRowIdentity = Pick<
  CubeStackRowModel,
  'label' | 'secondary' | 'tooltip' | 'icon' | 'targetModel' | 'severity'
>;

/**
 * Give the rail and the cart the same reading of a Cube: its name without the model route, the
 * model as the chip on its icon, its version and pack beneath the name, and the full title,
 * definition and source in the tooltip.
 */
export function cubeStackRowIdentity(
  identity: CubeIdentityPresentation,
  cubeId: unknown,
): CubeStackRowIdentity {
  return {
    label: identity.instanceModelTitle.nameText,
    secondary: [identity.versionText, formatStackPackText(cubeId)].filter(Boolean).join(' · '),
    tooltip: [identity.instanceTitle, identity.definitionLine, identity.sourceLine]
      .filter(Boolean)
      .join('\n'),
    icon: identity.icon,
    targetModel: identity.targetModel,
    severity: identity.awaitingFirstSave ? 'warning' : 'none',
  };
}

/** Show the compact repository or local namespace retained by canonical and legacy Cube ids. */
function formatStackPackText(cubeId: unknown): string {
  if (typeof cubeId !== 'string') return '';
  const parts = cubeId
    .replace(/\\/g, '/')
    .split('/')
    .map((part) => part.trim())
    .filter(Boolean);
  if (parts.length < 2) return '';
  const pack = parts.length >= 3 ? parts[1] : parts[0];
  return (pack ?? '')
    .replace(/[\s_]+/g, '-')
    .replace(/-+/g, '-')
    .replace(/^-|-$/g, '')
    .toLowerCase();
}
