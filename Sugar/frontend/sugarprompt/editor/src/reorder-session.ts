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
/** One native session owns Alt/pointer/keyboard transformation intent and genuine command publication. */
import { readReorderPointerTarget } from './reorder-pointer-target.js';
import { snapshotPresentation } from './native-snapshot-policy.js';
import {
  prepareSessionReorderApplication,
  createReorderApplicationHold,
  type ReorderApplicationHold,
} from './reorder-session-application.js';
import { Transaction } from '@codemirror/state';
import { type EditorView, type ViewUpdate } from '@codemirror/view';
import { hideReorderChip, type ReorderState } from '../../core/src/index.js';
import { currentNativeReorder, prepareReorderCommit } from './reorder-command.js';
import { publishReorderPreview, reorderPresentationEffect } from './reorder-preview.js';
import { retireReorderPublication, destroyReorderSessionOwners } from './reorder-retirement.js';
import { ReorderMeasurement } from './reorder-measurement.js';
import type { ReorderMeasureInput } from './reorder-measurement-contracts.js';
import type { ReorderArrow } from './reorder-measurement-contracts.js';
import { nativeCompositionStarted, type CommandInputSettlement } from './input-settlement.js';
import { dispatchNativeIngestion } from './native-ingestion.js';
import { prepareNativeIngestionPost } from './native-ingestion-post.js';
import { setReorderGrabCursor, releaseReorderGrabCursor } from './reorder-grab-cursor.js';
import { NativeReorderEntry, type ReorderEntryOutcome } from './reorder-entry.js';

/** Pending measured arrows have an explicit finite port interaction bound; overflow cancels the entire gesture. */
const pendingArrowCapacity = 256;
/** The mounted session retains one immutable order operand and original native selection, never another document. */
interface Session extends ReorderMeasureInput {
  readonly reservation: CommandInputSettlement;
  order: ReorderState;
  held: number;
  /** A pending primary hold binds continuation across synchronous native capture cleanup. */
  hold: ReorderApplicationHold | null;
  pointer: number | null;
  coordinates: { readonly x: number; readonly y: number } | null;
  hidden: ReorderState | null;
}
/** Mounted event routing delegates native geometry/resources to one finite measurement owner. */
export class NativeReorderSession {
  private readonly view: EditorView;
  private readonly measurement: ReorderMeasurement;
  private session: Session | null = null;
  private readonly entry: NativeReorderEntry;
  private moving = false;
  private queued: ReorderArrow[] = [];
  private released = false;
  private dropping = false;
  private disposed = false;
  private grabCursor: object | null = null;
  private refreshRequest: { readonly session: Session } | null = null;
  private retirement: { destroy: boolean } | null = null;
  private deferMeasurement = false;
  /** Wire explicit measured outcomes back to the sole current transformation owner. */
  constructor(view: EditorView) {
    this.view = view;
    this.measurement = new ReorderMeasurement(view, {
      current: (input) => this.current(input),
      collision: (input) => this.collision(input),
      prepare: (input, hidden, target, pointer) => {
        const session = this.active(input);
        return prepareSessionReorderApplication(
          session,
          hidden,
          target,
          pointer,
          () => !!session && this.current(session),
          (presentation) => !!session && this.publish(session, presentation),
        );
      },
      restore: (input) => {
        const session = this.active(input);
        if (session) {
          session.hidden = null;
          this.publish(session);
        }
      },
      commit: (input) => {
        const session = this.active(input);
        if (session) this.commit(session);
      },
      failed: () => this.close(this.deferMeasurement || this.measurement.measuring),
    });
    this.entry = new NativeReorderEntry(view, {
      measure: (start, pointer, settle) => this.measurement.entry(start, pointer, settle),
      retireMeasurement: () => this.retireMeasurement(),
      available: () => !this.disposed && this.session === null && this.retirement === null,
      complete: (outcome) => this.accept(outcome),
      failed: (reservation) => {
        if (!this.session || this.session.reservation === reservation) this.close();
      },
    });
  }
  /** Actual identity and canonical currentness fence every measured/deferred publication. */
  private active(input: ReorderMeasureInput): Session | null {
    return this.session === input ? this.session : null;
  }
  /** Dormant/foreign/mutated source or native selection cannot renew a session. */
  private current(input: ReorderMeasureInput): boolean {
    const session = this.active(input);
    return (
      !this.disposed &&
      session !== null &&
      currentNativeReorder(this.view.state, session.capture) &&
      !nativeCompositionStarted(this.view)
    );
  }
  /** External source/context/selection/focus and composition retire synchronously; clearing paint waits until update completes. */
  update(update: ViewUpdate): void {
    const previousDeferral = this.deferMeasurement;
    this.deferMeasurement = true;
    try {
      this.entry.update();
      if (
        this.session &&
        (!this.current(this.session) || (update.focusChanged && !this.view.hasFocus))
      )
        this.close(true);
      else if (
        (update.geometryChanged || update.viewportChanged) &&
        !update.transactions.some((transaction) =>
          transaction.effects.some((effect) => reorderPresentationEffect(effect)),
        )
      )
        this.deferGeometryRefresh();
    } finally {
      this.deferMeasurement = previousDeferral;
    }
  }
  /** Only one exact live session may renew geometry after the enclosing CM update unlocks. */
  private deferGeometryRefresh(): void {
    const session = this.session;
    if (session && this.current(session)) this.measurement.invalidateGeometry(session);
    if (!session || !this.current(session) || this.refreshRequest?.session === session) return;
    const request = { session };
    this.refreshRequest = request;
    queueMicrotask(() => {
      if (this.refreshRequest !== request) return;
      this.refreshRequest = null;
      if (this.session === session && this.current(session)) this.refreshGeometry();
    });
  }
  /** Entry authority retires synchronously; native publication and resource release wait outside locked updates. */
  private retireMeasurement(destroy = false): void {
    if (!this.deferMeasurement && !this.disposed && !this.retirement) {
      this.measurement.cancel();
      return;
    }
    if (this.retirement) {
      this.retirement.destroy ||= destroy;
      return;
    }
    const retirement = { destroy };
    this.measurement.invalidate();
    this.retirement = retirement;
    queueMicrotask(() => {
      if (this.retirement !== retirement) return;
      const failures: unknown[] = [];
      try {
        if (!retirement.destroy)
          try {
            this.measurement.cancel();
          } catch (cause) {
            failures.push(cause);
          }
        if (retirement.destroy)
          try {
            this.measurement.destroy();
          } catch (cause) {
            failures.push(cause);
          }
      } finally {
        if (this.retirement === retirement) this.retirement = null;
      }
      if (failures.length === 1) throw failures[0];
      if (failures.length) throw new AggregateError(failures, 'Reorder deferred retirement failed');
    });
  }
  /** Actual geometry renews pending keyboard/pointer placement and release; stationary feedback only remeasures its hidden publication. */
  refreshGeometry(): void {
    this.refreshRequest = null;
    const session = this.session;
    if (session && this.current(session)) this.measurement.refresh(session);
  }
  /** Explicit native wheel pauses only edge scrolling at the current pointer position. */
  wheel(): false {
    const session = this.session;
    if (session) this.measurement.wheel(session);
    return false;
  }
  /** Delegate complete paint publication and retain unavailable-output retirement at the gesture owner. */
  private publish(session: Session, presentation: 'complete' | 'pointer' = 'complete'): boolean {
    const outcome = publishReorderPreview(
      this.view,
      {
        token: session.token,
        capture: session.capture,
        activeChip: session.held,
        order: session.order,
      },
      presentation,
      session.hold,
      () => this.current(session),
    );
    if (outcome === 'unavailable') this.close();
    return outcome === 'published';
  }
  /** A settled canonical entry transfers its reservation and held identity to this sole gesture owner. */
  private accept(outcome: ReorderEntryOutcome): void {
    if (outcome.status === 'cancelled') {
      this.finishGrabCursor();
      return;
    }
    const { capture, reservation, pointer } = outcome;
    const session: Session = {
      token: {},
      capture,
      reservation,
      order: capture.prepared.initial,
      held: capture.active,
      pointer: null,
      hold: null,
      coordinates: null,
      hidden: null,
    };
    this.session = session;
    if (!this.publish(session)) {
      if (this.session === session) this.close();
      return;
    }
    if (pointer) {
      this.pointer(session, pointer.pointer, pointer.x, pointer.y);
      const motion = outcome.motion;
      if (motion && this.current(session)) {
        session.coordinates = { x: motion.x, y: motion.y };
        this.measurement.motion(session, false);
        this.measurement.scroll(session);
      }
    }
  }
  /** Only the session transforms the current complete order into Core's genuine hidden operand. */
  private collision(input: ReorderMeasureInput): ReorderState | null {
    const session = this.active(input);
    if (!session || !this.current(session)) return null;
    if (session.pointer !== null && session.hidden) return session.hidden;
    session.hidden = hideReorderChip(session.capture.prepared, session.order, session.held);
    if (!session.hidden || (session.pointer !== null && !this.publish(session, 'pointer')))
      return null;
    return session.hidden;
  }
  /** A new pointer hold supersedes pending keyboard intent and relinquishes the previous target/capture before changing chip identity. */
  private pointer(session: Session, id: number, x: number, y: number, held = session.held): void {
    if (!this.current(session)) return;
    const hold = (session.hold = createReorderApplicationHold(session.order));
    /** Newer same-session holds supersede this continuation before any external cleanup returns. */
    const current = (): boolean => session.hold === hold && this.current(session);
    this.moving = this.released = this.dropping = false;
    this.queued = [];
    session.pointer = null;
    this.measurement.retarget(held, { x, y });
    if (!current()) return;
    session.hidden = null;
    session.held = held;
    session.pointer = id;
    session.coordinates = { x, y };
    if (!this.publish(session) || !current()) return;
    this.measurement.begin(session);
  }
  /** Arrows retain every ordered intent and commit Alt release only after all required measurements settle. */
  private arrow(key: ReorderArrow): void {
    const session = this.session;
    if (!session || session.pointer !== null || !this.current(session)) return;
    if (this.moving) {
      if (this.queued.length === pendingArrowCapacity) {
        this.close();
        return;
      }
      this.queued.push(key);
      return;
    }
    this.moving = true;
    this.measurement.keyboard.arrow(session, key, () => {
      if (!this.current(session)) return;
      this.moving = false;
      const queued = this.queued.shift();
      if (queued) this.arrow(queued);
      else if (this.released) this.commit(session);
    });
  }
  /** Close resources before constructing and publishing one genuine isolated command and authenticated semantic link. */
  private commit(session: Session): void {
    if (!this.current(session)) {
      this.close();
      return;
    }
    const { capture, order } = session;
    this.close();
    if (!currentNativeReorder(this.view.state, capture)) return;
    const outcome = prepareReorderCommit(this.view.state, capture, order);
    if (outcome.status !== 'ready') return;
    const edit = outcome.edit,
      command = edit.transactions[0];
    if (!command) return;
    const post = prepareNativeIngestionPost(command.state, edit);
    dispatchNativeIngestion(this.view, {
      startState: edit.startState,
      edit,
      transactions: [command, post],
    });
  }
  /** Retirement releases sole ownership before observer callbacks and clears only its own current preview token. */
  close(defer = false): void {
    this.refreshRequest = null;
    this.moving = false;
    this.queued = [];
    this.released = false;
    this.dropping = false;
    const session = this.session;
    this.session = null;
    const cursor = this.grabCursor;
    this.grabCursor = null;
    const failures: unknown[] = [];
    /** Release each captured owner before deferred preview cleanup, retaining all external causes. */
    const release = (action: () => void): void => {
      try {
        action();
      } catch (cause) {
        failures.push(cause);
      }
    };
    const previousDeferral = this.deferMeasurement;
    this.deferMeasurement ||= defer;
    try {
      release(() => this.entry.cancel());
    } finally {
      this.deferMeasurement = previousDeferral;
    }

    if (session) release(() => this.entry.release(session.reservation));
    release(() => retireReorderPublication(this.view, session?.token ?? null, cursor, defer));
    if (failures.length === 1) throw failures[0];
    if (failures.length) throw new AggregateError(failures, 'Reorder session cleanup failed');
  }
  /** Destroy cannot publish to a disposed surface or allow a late read/write to revive resources. */
  destroy(): void {
    if (this.disposed) return;
    this.disposed = true;
    destroyReorderSessionOwners(
      () => this.close(true),
      this.entry,
      () => this.retireMeasurement(true),
    );
  }
  /** Dormant invalid/AltGraph entry delegates to ordinary native input; active mode consumes authoring keys. */
  keydown(event: KeyboardEvent): boolean {
    if (
      event.key === 'Alt' &&
      !event.ctrlKey &&
      !event.metaKey &&
      !event.getModifierState('AltGraph')
    )
      return this.session !== null || this.entry.pending || this.entry.begin();
    if (!this.session && !this.entry.pending) return false;
    if (event.key === 'Escape') {
      this.close();
      return true;
    }
    if (['ArrowLeft', 'ArrowRight', 'ArrowUp', 'ArrowDown'].includes(event.key)) {
      this.arrow(event.key as ReorderArrow);
      return true;
    }
    return true;
  }
  /** Alt release settles the measured placement once, even if a pointer or arrow request is still pending. */
  keyup(event: KeyboardEvent): boolean {
    if (event.key !== 'Alt') return false;
    this.finishGrabCursor();
    const session = this.session;
    if (
      session &&
      session.pointer !== null &&
      (session.hidden || this.measurement.pendingDrag(session))
    ) {
      this.dropping = true;
      this.measurement.motion(session, true);
    } else if (session && this.moving) this.released = true;
    else if (session) this.commit(session);
    else this.close();
    return session !== null;
  }
  /** Accepted current Alt mode owns primary presses until release; dormant/pending/settling entry requires pointer Alt before native defaults. */
  pointerdown(event: PointerEvent): boolean {
    const session = this.session;
    if (
      !event.isPrimary ||
      event.button !== 0 ||
      (!event.altKey &&
        (!session ||
          !this.current(session) ||
          this.session !== session ||
          this.released ||
          this.dropping)) ||
      event.ctrlKey ||
      event.metaKey
    )
      return false;
    if (this.entry.pending) this.close();
    if (!this.session) {
      const entered = this.entry.begin({
        x: event.clientX,
        y: event.clientY,
        pointer: event.pointerId,
      });
      if (entered) this.startGrabCursor();
      return entered || this.blocksInput();
    }
    const id = readReorderPointerTarget(this.session.capture, event, () =>
      this.measurement.visualHit(event.clientX, event.clientY),
    );
    if (id === null) return true;
    this.pointer(this.session, event.pointerId, event.clientX, event.clientY, id);
    if (this.session?.pointer === event.pointerId) this.startGrabCursor();
    return true;
  }
  /** Captured pointer motion coalesces one native measure and resumes scroll only at a real edge. */
  pointermove(event: PointerEvent): boolean {
    const session = this.session;
    if (!session)
      return this.entry.motion({
        x: event.clientX,
        y: event.clientY,
        pointer: event.pointerId,
      });
    if (session.pointer !== event.pointerId) return false;
    if (this.dropping) return true;
    session.coordinates = { x: event.clientX, y: event.clientY };
    this.measurement.motion(session, false);
    this.measurement.scroll(session);
    return true;
  }
  /** Final pointer coordinates are measured before drop; a released pending entry is canceled. */
  pointerup(event: PointerEvent): boolean {
    const session = this.session;
    if (!session) {
      if (!this.entry.ownsPointer(event.pointerId)) return false;
      this.close();
      return true;
    }
    if (session.pointer !== event.pointerId) return false;
    if (this.dropping) return true;
    this.finishGrabCursor();
    session.coordinates = { x: event.clientX, y: event.clientY };
    if (!session.hidden && !this.measurement.pendingDrag(session)) {
      session.pointer = null;
      session.coordinates = null;
      session.hold = null;
      this.measurement.retarget();
      return true;
    }
    this.dropping = true;
    this.measurement.motion(session, true);
    return true;
  }
  /** Derived primary-press metadata uses supported native transactions and one session-owned token. */
  private startGrabCursor(): void {
    const token = {};
    this.grabCursor = token;
    try {
      this.view.dispatch({
        effects: setReorderGrabCursor.of(token),
        annotations: [Transaction.addToHistory.of(false), snapshotPresentation.of(true)],
      });
    } catch (cause) {
      this.failCursorPublication(cause, token);
    }
  }
  /** Actual primary/Alt release relinquishes cursor immediately while final drop geometry retains its independent finite owner. */
  private finishGrabCursor(): void {
    const token = this.grabCursor;
    if (!token || this.disposed) return;
    try {
      const transaction = releaseReorderGrabCursor(this.view.state, token);
      if (transaction) this.view.dispatch(transaction);
    } catch (cause) {
      this.failCursorPublication(cause, token);
    }
    if (this.grabCursor === token) this.grabCursor = null;
  }
  /** A failed issued cursor publication retires only its still-owned gesture and preserves ordered cleanup causes. */
  private failCursorPublication(cause: unknown, token: object): never {
    if (this.grabCursor === token) {
      try {
        this.close();
      } catch (cleanup) {
        throw new AggregateError([cause, cleanup], 'Reorder cursor publication cleanup failed');
      }
    }
    throw cause;
  }
  /** Browser source-producing events are consumed only while this finite interaction owns input. */
  blocksInput(): boolean {
    return this.session !== null || this.entry.pending;
  }
  /** Implicit release after matched pointerup retains only the already-reserved final measurement. */
  pointerLost(event: PointerEvent): boolean {
    if (this.entry.ownsPointer(event.pointerId)) {
      this.close();
      return false;
    }
    if (this.session?.pointer !== event.pointerId) return false;
    if (!this.dropping) this.close();
    return false;
  }
}
