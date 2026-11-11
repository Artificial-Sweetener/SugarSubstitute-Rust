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
/** Sole finite native entry ownership settles input before handing canonical capture to the gesture. */
import type { EditorState } from '@codemirror/state';
import type { EditorView } from '@codemirror/view';
import {
  prepareNativeReorder,
  reorderAllowed,
  type NativeReorderCapture,
} from './reorder-command.js';
import {
  nativeInputSettlement,
  nativeCompositionStarted,
  type CommandInputSettlement,
} from './input-settlement.js';
import { semanticStructures } from './structure.js';
import { editorFreshness } from './freshness.js';
import {
  currentCanonicalProjection,
  projectsRichSyntax,
  richProjection,
} from './rich-projection.js';

/** Resource rebinding can retire rich paint after syntax is ready; entry waits for the complete current preparation. */
function preparationReady(state: EditorState): boolean {
  return (
    state.field(semanticStructures).native !== null &&
    (!state.field(richProjection, false) ||
      !projectsRichSyntax(state) ||
      currentCanonicalProjection(state) !== null)
  );
}

/** A primary press uses native pointer identity and viewport CSS-pixel coordinates. */
export interface ReorderEntryPointer {
  readonly x: number;
  readonly y: number;
  readonly pointer: number;
}
/** Accepted capture transfers reservation and original/latest physical coordinates; cancellation retains no source. */
export type ReorderEntryOutcome =
  | {
      readonly status: 'accepted';
      readonly capture: NativeReorderCapture;
      readonly reservation: CommandInputSettlement;
      readonly pointer: ReorderEntryPointer | undefined;
      readonly motion: ReorderEntryPointer | undefined;
    }
  | { readonly status: 'cancelled' };
/** Downward capture/measurement adapters never import the mounted gesture owner. */
export interface ReorderEntryPorts {
  /** Geometry read and deferred settlement use the canonical native measurement owner. */
  measure(
    start: EditorState,
    pointer: ReorderEntryPointer | undefined,
    settle: (position: number | null) => void,
  ): void;
  /** Retire measurements before external reservation cancellation can reenter. */
  retireMeasurement(): void;
  /** A current gesture excludes competing pending admission. */
  available(): boolean;
  /** Consume the genuine capture or typed source-free cancellation. */
  complete(outcome: ReorderEntryOutcome): void;
  /** Retire only current pending/accepted work associated with this failed reservation. */
  failed(reservation: CommandInputSettlement): void;
}
/** Keep original native admission immutable while finite readiness and physical motion advance. */
interface PendingEntry {
  readonly epoch: number;
  readonly start: EditorState;
  readonly reservation: CommandInputSettlement;
  readonly pointer: ReorderEntryPointer | undefined;
  motion: ReorderEntryPointer | undefined;
  waiting: boolean;
  queued: boolean;
  measurement: object | null;
}

/** One entry owns reservation issuance, callback freshness and exactly-once external retirement. */
export class NativeReorderEntry {
  private readonly view: EditorView;
  private readonly ports: ReorderEntryPorts;
  private receipt: PendingEntry | null = null;
  private epoch = 0;
  private disposed = false;
  private readonly cancelled = new WeakSet<CommandInputSettlement>();
  /** Bind the mounted view and downward adapters without a runtime Session dependency. */
  constructor(view: EditorView, ports: ReorderEntryPorts) {
    this.view = view;
    this.ports = ports;
  }
  /** Pending preparation or measurement retains exclusive native input authority. */
  get pending(): boolean {
    return this.receipt !== null;
  }
  /** Only matching pending primary motion updates retained physical coordinates. */
  motion(pointer: ReorderEntryPointer): boolean {
    if (!this.ownsPointer(pointer.pointer)) return false;
    if (this.receipt) this.receipt.motion = pointer;
    return true;
  }
  /** Release/loss targets the actual pending press rather than unrelated keyboard entry. */
  ownsPointer(id: number): boolean {
    return this.receipt?.pointer?.pointer === id;
  }
  /** Reserve genuine native input once; Session routes superseding pointer entry through retirement. */
  begin(pointer?: ReorderEntryPointer): boolean {
    if (
      this.disposed ||
      this.receipt ||
      !this.ports.available() ||
      nativeCompositionStarted(this.view) ||
      !reorderAllowed(this.view.state)
    )
      return false;
    const start = this.view.state;
    const reservation = start.facet(nativeInputSettlement).reserveCommand(this.view);
    const receipt: PendingEntry = {
      epoch: ++this.epoch,
      start,
      reservation,
      pointer,
      motion: pointer,
      waiting: !preparationReady(start),
      queued: false,
      measurement: null,
    };
    this.receipt = receipt;
    if (!receipt.waiting) this.measure(receipt);
    return true;
  }
  /** Resume the original pending press after semantic publication, outside the enclosing native update. */
  update(): void {
    const receipt = this.receipt;
    if (!receipt?.waiting || receipt.queued) return;
    receipt.queued = true;
    queueMicrotask(() => {
      receipt.queued = false;
      if (this.disposed || this.receipt !== receipt) return;
      const state = this.view.state;
      const composing = nativeCompositionStarted(this.view);
      if (
        this.disposed ||
        this.receipt !== receipt ||
        composing ||
        this.view.state !== state ||
        state.doc !== receipt.start.doc ||
        state.selection !== receipt.start.selection ||
        state.field(editorFreshness) !== receipt.start.field(editorFreshness) ||
        !reorderAllowed(state)
      ) {
        if (this.receipt === receipt) this.retire(receipt);
        return;
      }
      if (!preparationReady(state)) return;
      receipt.waiting = false;
      this.measure(receipt);
    });
  }
  /** Keep measurement failures under the same reservation cleanup owner as settlement failures. */
  private measure(receipt: PendingEntry): void {
    const measurement = {};
    receipt.measurement = measurement;
    try {
      this.ports.measure(receipt.start, receipt.pointer, (position) => {
        if (receipt.measurement !== measurement) return;
        receipt.measurement = null;
        this.settle(receipt, position);
      });
    } catch (cause) {
      if (this.cancelled.has(receipt.reservation)) throw cause;
      this.fail(receipt, cause);
    }
  }
  /** Canonical source/selection admission rejects every superseded native callback before capture handoff. */
  private settle(receipt: PendingEntry, position: number | null): void {
    if (this.disposed || this.receipt !== receipt) return;
    const { start, reservation, pointer } = receipt;
    try {
      const unchanged = (): boolean =>
        this.view.state.doc === start.doc &&
        this.view.state.selection === start.selection &&
        !nativeCompositionStarted(this.view);
      if (!unchanged()) {
        this.retire(receipt);
        return;
      }
      if (!preparationReady(this.view.state)) {
        receipt.waiting = true;
        this.update();
        return;
      }
      reservation.finish();
      if (this.disposed || this.receipt !== receipt) {
        this.release(reservation);
        return;
      }
      this.receipt = null;
      if (pointer && !unchanged()) {
        this.retire(receipt);
        return;
      }
      this.view.focus();
      if (this.disposed || receipt.epoch !== this.epoch) {
        this.release(reservation);
        return;
      }
      if (pointer && !unchanged()) {
        this.retire(receipt);
        return;
      }
      const state = this.view.state,
        semantic = state.field(semanticStructures).native;
      const capture =
        semantic && position !== null
          ? prepareNativeReorder(
              state,
              {
                values: semantic.nativeValues,
                semantics: semantic.documentSemantics,
                enabled: true,
                composing: false,
              },
              pointer ? { from: position, to: position } : state.selection.main,
            )
          : null;
      if (!capture) {
        this.retire(receipt);
        return;
      }
      this.ports.complete({
        status: 'accepted',
        capture,
        reservation,
        pointer,
        motion: receipt.motion,
      });
    } catch (cause) {
      this.fail(receipt, cause);
    }
  }
  /** Release failed admission exactly once without retiring a newer entry created by external cleanup. */
  private fail(receipt: PendingEntry, cause: unknown): never {
    const failures: unknown[] = [cause];
    if (!this.disposed && receipt.epoch === this.epoch) {
      this.receipt = null;
      try {
        this.ports.failed(receipt.reservation);
      } catch (cleanup) {
        failures.push(cleanup);
      }
    }
    try {
      this.release(receipt.reservation);
    } catch (cleanup) {
      failures.push(cleanup);
    }
    if (failures.length === 1) throw cause;
    throw new AggregateError(failures, 'Reorder entry cleanup failed');
  }
  /** Expected rejection retires measurement first and preserves all cancellation failures before typed publication. */
  private retire(receipt: PendingEntry): void {
    const failures: unknown[] = [];
    try {
      this.cancel();
    } catch (cause) {
      failures.push(cause);
    }
    try {
      this.release(receipt.reservation);
    } catch (cause) {
      failures.push(cause);
    }
    if (failures.length === 1) throw failures[0];
    if (failures.length > 1) throw new AggregateError(failures, 'Reorder entry rejection failed');
    if (!this.disposed && receipt.epoch + 1 === this.epoch)
      this.ports.complete({ status: 'cancelled' });
  }
  /** Accepted gestures return reservation cleanup here; one reservation cancels at most once despite reentry/failure. */
  release(reservation: CommandInputSettlement): void {
    if (this.cancelled.has(reservation)) return;
    this.cancelled.add(reservation);
    reservation.cancel();
  }
  /** Retire identity and native measurement before external cancellation can admit a newer receipt. */
  cancel(): void {
    ++this.epoch;
    const receipt = this.receipt;
    this.receipt = null;
    const failures: unknown[] = [];
    try {
      this.ports.retireMeasurement();
    } catch (cause) {
      failures.push(cause);
    }
    try {
      if (receipt) this.release(receipt.reservation);
    } catch (cause) {
      failures.push(cause);
    }
    if (failures.length === 1) throw failures[0];
    if (failures.length > 1) throw new AggregateError(failures, 'Reorder entry retirement failed');
  }
  /** Mounted teardown prevents late callback publication and future admission. */
  destroy(): void {
    this.disposed = true;
    this.cancel();
  }
}
