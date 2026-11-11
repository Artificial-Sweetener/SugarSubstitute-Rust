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
/** Session delegates its complete Core preparation/publication responsibility without exposing geometry or another state owner. */
import {
  dropReorderChip,
  valueReorderVacancy,
  type PreparedValueReorder,
  type ReorderDropTarget,
  type ReorderState,
} from '../../core/src/index.js';
import { sameReorderTarget } from './reorder-placement-topology.js';
import { sameReorderOrder } from './reorder-preview-preparation.js';
import type { ReorderApplyOutcome, ReorderMeasureInput } from './reorder-measurement-contracts.js';

/** One immutable primary hold retains its exact authored vacancy through provisional pointer proposals. */
export interface ReorderApplicationHold {
  readonly order: ReorderState;
  /** Reuse the actual Core vacancy only under this hold's exact hidden operand and held identity. */
  vacancy(hidden: ReorderState, held: number): ReorderDropTarget | null;
  /** Reuse one actual immutable Core result under exact source, hidden, held and copied target operands. */
  drop(
    prepared: PreparedValueReorder,
    hidden: ReorderState,
    held: number,
    target: ReorderDropTarget,
  ): ReorderState | null;
}
/** Each immutable primary hold retains one Core vacancy and one Core drop result; changed operands replace their finite entries. */
export function createReorderApplicationHold(order: ReorderState): ReorderApplicationHold {
  let memo: {
    readonly hidden: ReorderState;
    readonly held: number;
    readonly target: ReorderDropTarget | null;
  } | null = null;
  let dropped: {
    readonly prepared: PreparedValueReorder;
    readonly hidden: ReorderState;
    readonly held: number;
    readonly target: ReorderDropTarget;
    readonly result: ReorderState | null;
  } | null = null;
  return Object.freeze({
    order,
    /** Core remains the sole vacancy derivation owner, including unavailable results. */
    vacancy(hidden: ReorderState, held: number): ReorderDropTarget | null {
      if (memo?.hidden === hidden && memo.held === held) return memo.target;
      const target = valueReorderVacancy(order, hidden, held);
      memo = { hidden, held, target: target && Object.freeze(target) };
      return memo.target;
    },
    /** Core alone validates and transforms each new operand set; memoized results grant no Session publication authority. */
    drop(
      prepared: PreparedValueReorder,
      hidden: ReorderState,
      held: number,
      target: ReorderDropTarget,
    ): ReorderState | null {
      const captured = Object.freeze({ ...target }),
        retained = dropped;
      if (
        retained?.prepared === prepared &&
        retained.hidden === hidden &&
        retained.held === held &&
        sameReorderTarget(retained.target, captured)
      )
        return retained.result;
      const result = dropReorderChip(prepared, hidden, held, captured);
      dropped = { prepared, hidden, held, target: captured, result };
      return result;
    },
  });
}
/** Only the actual Session supplies these writable transform operands and exact primary-hold identity. */
export interface ReorderApplicationInput extends ReorderMeasureInput {
  order: ReorderState;
  hidden: ReorderState | null;
  readonly hold: ReorderApplicationHold | null;
}
/** Compute once through Core, then consume the exact result only under its original native Session/hold/source identities. */
export function prepareSessionReorderApplication(
  session: ReorderApplicationInput | null,
  hidden: ReorderState,
  target: ReorderDropTarget,
  pointer: boolean,
  current: () => boolean,
  publish: (presentation: 'pointer' | 'complete') => boolean,
): ReorderApplyOutcome {
  if (!session) return Object.freeze({ kind: 'unavailable' });
  const { order, held, hold, capture, token } = session,
    pointerId = session.pointer;
  let consumed = false;
  /** External native currentness cannot admit a replaced hold or mutate this captured Core operand. */
  const matches = (): boolean =>
    !consumed &&
    session.order === order &&
    session.hidden === hidden &&
    session.held === held &&
    session.hold === hold &&
    session.capture === capture &&
    session.token === token &&
    session.pointer === pointerId;
  /** Recheck captured identities after delegated Session currentness, including synchronous retirement/reentry. */
  const admitted = (): boolean => matches() && current() && matches();
  if (!admitted()) return Object.freeze({ kind: 'unavailable' });
  const dropped =
    pointer && hold
      ? hold.drop(capture.prepared, hidden, held, target)
      : dropReorderChip(capture.prepared, hidden, held, target);
  if (!dropped || !admitted()) return Object.freeze({ kind: 'unavailable' });
  const vacancy = pointer && hold ? hold.vacancy(hidden, held) : null,
    moved = hold && vacancy && sameReorderTarget(vacancy, target) ? hold.order : dropped;
  if (!admitted()) return Object.freeze({ kind: 'unavailable' });
  if (pointer && sameReorderOrder(order, moved))
    return Object.freeze({ kind: 'unchanged', current: admitted });
  return Object.freeze({
    kind: 'changed',
    current: admitted,
    /** Consume before native publication; any unexpected publication failure remains owned by the existing observation boundary. */
    apply(): boolean {
      if (!admitted()) return false;
      consumed = true;
      session.order = moved;
      if (!pointer) session.hidden = null;
      return publish(pointer ? 'pointer' : 'complete');
    },
  });
}
