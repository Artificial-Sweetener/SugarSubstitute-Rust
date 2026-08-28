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
/** Define Cube face titlebar actions without owning their implementations. */

import type { UnknownRecord } from '../types/common.js';
import type { ComfyPrimeIconName } from './ComfyPrimeIcons.js';

export type CubeSwapDirection = 'left' | 'right';

export interface CubeFaceGraphSummary {
  inputCount?: number;
  markerIds?: unknown[];
  nodeIds?: unknown[];
  outputCount?: number;
}

export interface CubeFaceChromeMetadata extends UnknownRecord {
  graphSummary?: CubeFaceGraphSummary;
  instance_id?: string;
}

export interface CubeFaceActionAnchor {
  left: number;
  top: number;
  right: number;
  bottom: number;
}

export interface CubeFaceChromeActions {
  onAddCube?(metadata: CubeFaceChromeMetadata, anchor: CubeFaceActionAnchor): void;
  onSwapLeft?(metadata: CubeFaceChromeMetadata): void;
  onSwapRight?(metadata: CubeFaceChromeMetadata): void;
  canSwap?(metadata: CubeFaceChromeMetadata, direction: CubeSwapDirection): boolean;
}

export type CubeFaceTitlebarActionKey = 'swap-left' | 'swap-right' | 'add-cube';

export type CubeFaceActionIcon = ComfyPrimeIconName | 'cube-plus';

export interface CubeFaceTitlebarAction {
  key: CubeFaceTitlebarActionKey;
  icon: CubeFaceActionIcon;
  ariaLabel: string;
  title: string;
}

const SWAP_ACTIONS: ReadonlyArray<{
  key: Extract<CubeFaceTitlebarActionKey, 'swap-left' | 'swap-right'>;
  direction: CubeSwapDirection;
  callback: 'onSwapLeft' | 'onSwapRight';
  icon: ComfyPrimeIconName;
  ariaLabel: string;
}> = Object.freeze([
  {
    key: 'swap-left',
    direction: 'left',
    callback: 'onSwapLeft',
    icon: 'arrow-left',
    ariaLabel: 'Swap Cube left',
  },
  {
    key: 'swap-right',
    direction: 'right',
    callback: 'onSwapRight',
    icon: 'arrow-right',
    ariaLabel: 'Swap Cube right',
  },
]);

const ADD_ACTION: CubeFaceTitlebarAction = Object.freeze({
  key: 'add-cube',
  icon: 'cube-plus',
  ariaLabel: 'Add Cube after this Cube',
  title: 'Add Cube',
});

/** Return titlebar actions currently available for one Cube instance. */
export function resolveCubeFaceTitlebarActions(
  metadata: CubeFaceChromeMetadata,
  actions: CubeFaceChromeActions | null | undefined,
): CubeFaceTitlebarAction[] {
  const result: CubeFaceTitlebarAction[] = [];
  for (const action of SWAP_ACTIONS) {
    const callback = actions?.[action.callback];
    if (!callback) continue;
    const allowed = actions?.canSwap ? actions.canSwap(metadata, action.direction) : true;
    if (!allowed) continue;
    result.push({
      key: action.key,
      icon: action.icon,
      ariaLabel: action.ariaLabel,
      title: action.ariaLabel,
    });
  }
  if (actions?.onAddCube) result.push(ADD_ACTION);
  return result;
}

/** Invoke one available titlebar action through the authoritative action owner. */
export function dispatchCubeFaceTitlebarAction(
  key: CubeFaceTitlebarActionKey,
  metadata: CubeFaceChromeMetadata,
  actions: CubeFaceChromeActions | null | undefined,
  anchor?: CubeFaceActionAnchor,
): boolean {
  if (key === 'swap-left') {
    if (!actions?.onSwapLeft) return false;
    actions.onSwapLeft(metadata);
    return true;
  }
  if (key === 'swap-right') {
    if (!actions?.onSwapRight) return false;
    actions.onSwapRight(metadata);
    return true;
  }
  if (key === 'add-cube') {
    if (!actions?.onAddCube || !anchor) return false;
    actions.onAddCube(metadata, anchor);
    return true;
  }
  return false;
}
