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
/** One synchronous Slot read owns copied-subtree observation and releases all topology before returning. */
import {
  captureReorderLineLayout,
  type ReorderLineLayoutRead,
} from './reorder-layout-admission.js';
/** One synchronous READ composes bare ownership predicates inside the Slot's sole native mutation fence. */
export interface ReorderLayoutReadAuthority {
  /** Bare reservation authority is only an operand; native checkpoints must enclose their complete predicate in guard. */
  readonly available: () => boolean;
  /** Drain native records before and after the complete composed ownership predicate. No authority survives retirement. */
  readonly guard: (predicate: () => boolean) => boolean;
}
/** Observe only the exact readonly shell; no subscription, topology or mutable authority survives this invocation. Owner checks use cheap captured reservation facts during READ and complete original readiness once after successful cleanup. */
export function observeReorderSlotLayout<T>(
  node: HTMLElement,
  available: (phase: 'observation' | 'publication') => boolean,
  action: (topology: ReorderLineLayoutRead, authority: ReorderLayoutReadAuthority) => T,
): T | null {
  const Observer = node.ownerDocument.defaultView?.MutationObserver;
  if (!Observer) return null;
  let observer: MutationObserver | null = null,
    topology: ReorderLineLayoutRead | null = null;
  let active = true,
    dirty = false,
    failed = false;
  let primary: unknown;
  let result: T | null = null;
  const cleanup: unknown[] = [];
  /** Drain native mutation records on both sides of delegated owner checks, without copying or interpreting records. */
  const guard = (predicate: () => boolean): boolean => {
    if (!active || dirty || !observer) return false;
    if (observer.takeRecords().length) dirty = true;
    if (!active || dirty || !available('observation') || !predicate()) return false;
    if (observer.takeRecords().length) dirty = true;
    return active && !dirty && available('observation') && active && !dirty;
  };
  const authority: ReorderLayoutReadAuthority = {
    available: () => active && !dirty && available('observation'),
    guard,
  };
  /** Topology checkpoints use the same native fence as downstream composed owners. */
  const current = (): boolean => guard(authority.available);
  try {
    observer = new Observer(() => {
      dirty = true;
    });
    observer.observe(node, {
      subtree: true,
      childList: true,
      characterData: true,
      attributes: true,
    });
    if (current()) {
      topology = captureReorderLineLayout([node], current);
      if (topology && current()) {
        result = action(topology, authority);
        if (!current()) result = null;
      }
    }
  } catch (cause) {
    failed = true;
    primary = cause;
  } finally {
    active = false;
    for (const release of [() => topology?.retire(), () => observer?.disconnect()])
      try {
        release();
      } catch (cause) {
        cleanup.push(cause);
      }
  }
  if (failed) {
    if (!cleanup.length) throw primary;
    throw new AggregateError([primary, ...cleanup], 'reorder-slot-layout-read-retirement-failed');
  }
  if (cleanup.length === 1) throw cleanup[0];
  if (cleanup.length)
    throw new AggregateError(cleanup, 'reorder-slot-layout-read-retirement-failed');
  // Native disconnect can synchronously revoke or replace the original Slot reservation.
  return result !== null && !available('publication') ? null : result;
}
