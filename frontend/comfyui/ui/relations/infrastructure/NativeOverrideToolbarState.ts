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
/** Persist toolbar placement in the same browser-local scope as Comfy's Run panel. */
import { isRecord } from '../../types/common.js';
import type {
  OverrideToolbarPlacementState,
  OverrideToolbarPlacementStore,
  OverrideToolbarFloatingAnchor,
} from '../presentation/OverrideToolbarPlacementState.js';
import type { OverrideToolbarDisclosureStore } from '../presentation/OverrideToolbarDisclosure.js';

const KEY = 'SugarCubes.GlobalOverrides.Placement';
const COLLAPSED_KEY = 'SugarCubes.GlobalOverrides.Collapsed';

/** Validate browser storage before exposing remembered placement to presentation. */
export class NativeOverrideToolbarState
  implements OverrideToolbarPlacementStore, OverrideToolbarDisclosureStore
{
  /** Keep browser storage access and failure diagnostics behind the infrastructure boundary. */
  constructor(
    private readonly getStorage: () => Storage | null,
    private readonly logger: Pick<Console, 'warn'>,
  ) {}

  /** Default new and invalid preferences to the native top dock. */
  read(): OverrideToolbarPlacementState {
    try {
      const raw = this.getStorage()?.getItem(KEY);
      if (!raw) return { docked: true, floating: null, anchor: null };
      const value: unknown = JSON.parse(raw);
      if (!isRecord(value) || typeof value.docked !== 'boolean')
        throw new Error('Invalid override toolbar placement.');
      const floating = value.floating;
      const anchor = readAnchor(value.anchor);
      if (floating === null) return { docked: value.docked, floating: null, anchor: null };
      if (
        !isRecord(floating) ||
        typeof floating.x !== 'number' ||
        typeof floating.y !== 'number' ||
        !Number.isFinite(floating.x) ||
        !Number.isFinite(floating.y)
      )
        throw new Error('Invalid override toolbar floating coordinates.');
      return { docked: value.docked, floating: { x: floating.x, y: floating.y }, anchor };
    } catch (error: unknown) {
      this.logger.warn('SugarCubes override toolbar placement could not be restored', {
        operation: 'read',
        error,
      });
      return { docked: true, floating: null, anchor: null };
    }
  }

  /** Save completed user placement without persisting temporary layout fallbacks. */
  write(state: OverrideToolbarPlacementState): void {
    try {
      this.getStorage()?.setItem(KEY, JSON.stringify(state));
    } catch (error: unknown) {
      this.logger.warn('SugarCubes override toolbar placement could not be saved', {
        operation: 'write',
        error,
      });
    }
  }

  /** Restore compact presentation independently of floating placement and workflow values. */
  readCollapsed(): boolean {
    try {
      const raw = this.getStorage()?.getItem(COLLAPSED_KEY);
      if (!raw) return false;
      const value: unknown = JSON.parse(raw);
      if (typeof value !== 'boolean') throw new Error('Invalid override toolbar disclosure.');
      return value;
    } catch (error: unknown) {
      this.logger.warn('SugarCubes override toolbar disclosure could not be restored', {
        operation: 'readCollapsed',
        error,
      });
      return false;
    }
  }

  /** Persist only the user's disclosure action without replacing saved placement. */
  writeCollapsed(collapsed: boolean): void {
    try {
      this.getStorage()?.setItem(COLLAPSED_KEY, JSON.stringify(collapsed));
    } catch (error: unknown) {
      this.logger.warn('SugarCubes override toolbar disclosure could not be saved', {
        operation: 'writeCollapsed',
        error,
      });
    }
  }
}

/** Validate added anchoring metadata while preserving existing browser-local coordinate records. */
function readAnchor(value: unknown): OverrideToolbarFloatingAnchor | null {
  if (value === undefined || value === null) return null;
  if (!isRecord(value)) throw new Error('Invalid override toolbar anchor.');
  const edge = value.edge;
  if (
    (edge !== 'left' && edge !== 'right' && edge !== 'top' && edge !== 'bottom') ||
    typeof value.distance !== 'number' ||
    !Number.isFinite(value.distance) ||
    value.distance < 0 ||
    typeof value.horizontalRatio !== 'number' ||
    !Number.isFinite(value.horizontalRatio) ||
    value.horizontalRatio < 0 ||
    value.horizontalRatio > 1 ||
    typeof value.verticalRatio !== 'number' ||
    !Number.isFinite(value.verticalRatio) ||
    value.verticalRatio < 0 ||
    value.verticalRatio > 1
  )
    throw new Error('Invalid override toolbar anchor.');
  return {
    edge,
    distance: value.distance,
    horizontalRatio: value.horizontalRatio,
    verticalRatio: value.verticalRatio,
  };
}

/** Resolve the toolbar's own browser storage while keeping restricted-access failures observable. */
export function createNativeOverrideToolbarState(
  document: Document,
  logger: Pick<Console, 'warn'>,
): NativeOverrideToolbarState {
  return new NativeOverrideToolbarState(() => document.defaultView?.localStorage ?? null, logger);
}
