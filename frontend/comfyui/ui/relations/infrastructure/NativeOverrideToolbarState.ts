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
} from '../presentation/OverrideToolbarPlacementState.js';

const KEY = 'SugarCubes.GlobalOverrides.Placement';

/** Validate browser storage before exposing remembered placement to presentation. */
export class NativeOverrideToolbarState implements OverrideToolbarPlacementStore {
  /** Keep browser storage access and failure diagnostics behind the infrastructure boundary. */
  constructor(
    private readonly getStorage: () => Storage | null,
    private readonly logger: Pick<Console, 'warn'>,
  ) {}

  /** Default new and invalid preferences to the native top dock. */
  read(): OverrideToolbarPlacementState {
    try {
      const raw = this.getStorage()?.getItem(KEY);
      if (!raw) return { docked: true, floating: null };
      const value: unknown = JSON.parse(raw);
      if (!isRecord(value) || typeof value.docked !== 'boolean')
        throw new Error('Invalid override toolbar placement.');
      const floating = value.floating;
      if (floating === null) return { docked: value.docked, floating: null };
      if (
        !isRecord(floating) ||
        typeof floating.x !== 'number' ||
        typeof floating.y !== 'number' ||
        !Number.isFinite(floating.x) ||
        !Number.isFinite(floating.y)
      )
        throw new Error('Invalid override toolbar floating coordinates.');
      return { docked: value.docked, floating: { x: floating.x, y: floating.y } };
    } catch (error: unknown) {
      this.logger.warn('SugarCubes override toolbar placement could not be restored', {
        operation: 'read',
        error,
      });
      return { docked: true, floating: null };
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
}

/** Resolve the toolbar's own browser storage while keeping restricted-access failures observable. */
export function createNativeOverrideToolbarState(
  document: Document,
  logger: Pick<Console, 'warn'>,
): NativeOverrideToolbarState {
  return new NativeOverrideToolbarState(() => document.defaultView?.localStorage ?? null, logger);
}
