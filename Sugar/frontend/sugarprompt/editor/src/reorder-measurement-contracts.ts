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
/** Genuine session operands and typed callbacks shared by mounted and keyboard measurement owners. */
import type { ReorderState, ReorderDropTarget } from '../../core/src/index.js';
import type { NativeReorderCapture } from './reorder-command.js';

/** The session supplies read-only actual transform/intent identities; measurement never authors an order. */
export interface ReorderMeasureInput {
  /** One mounted gesture token fences live proposed publication from a newer or foreign paint. */
  readonly token: object;
  readonly capture: NativeReorderCapture;
  readonly order: ReorderState;
  readonly held: number;
  readonly hidden: ReorderState | null;
  readonly pointer: number | null;
  readonly coordinates: { readonly x: number; readonly y: number } | null;
}
/** A pure Session-issued Core result owns no geometry; only a still-current changed result may publish once. */
export type ReorderApplyOutcome =
  | { readonly kind: 'unavailable' }
  | { readonly kind: 'unchanged'; readonly current: () => boolean }
  | {
      readonly kind: 'changed';
      readonly current: () => boolean;
      readonly apply: () => boolean;
    };
/** Explicit callbacks return every transform/publication decision to the sole native session. */
export interface ReorderMeasureOwner {
  /** Fence the actual current session and canonical target before native publication. */
  current(input: ReorderMeasureInput): boolean;
  /** Prepare the genuine hidden Core operand; pointer holds publish their complete held preview, keyboard keeps complete live paint. */
  collision(input: ReorderMeasureInput): ReorderState | null;
  /** Session prepares the genuine immutable Core result; pointer READ can omit unused display geometry before publication. */
  prepare(
    input: ReorderMeasureInput,
    hidden: ReorderState,
    target: ReorderDropTarget,
    pointer: boolean,
  ): ReorderApplyOutcome;
  /** Restore the complete current order when no measured destination is available. */
  restore(input: ReorderMeasureInput): void;
  /** Compile and dispatch the genuine current command exactly once. */
  commit(input: ReorderMeasureInput): void;
  /** Retire the exact session after an unexpected native resource or publication failure. */
  failed(): void;
}

/** Native directional keys admitted by the one reorder session and measured keyboard owner. */
export type ReorderArrow = 'ArrowLeft' | 'ArrowRight' | 'ArrowUp' | 'ArrowDown';
