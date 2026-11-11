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
/** Finite native keyboard intent delegates hidden geometry while retaining complete live publication. */
import { requireValue } from '../../core/src/required.js';
import type { EditorView } from '@codemirror/view';
import type { ReorderState } from '../../core/src/index.js';
import { reorderPreview } from './reorder-preview.js';
import type { ReorderMeasuredGeometry, ReorderRect } from './reorder-geometry.js';
import { reorderKeyboardTarget } from './reorder-keyboard-geometry.js';
import type {
  ReorderArrow,
  ReorderMeasureInput,
  ReorderMeasureOwner,
} from './reorder-measurement-contracts.js';
import type { ReorderMotionOrigin } from './reorder-motion-origin.js';
import { ReorderCollisionBase } from './reorder-collision-base.js';
import type { ReorderBaseContext } from './reorder-base-context.js';
/** A resolved keyboard destination borrows one exact current measured collision output. */
interface ReorderKeyboardGeometryLease {
  readonly context: ReorderBaseContext;
  readonly rects: readonly ReorderRect[];
  readonly target: ReturnType<typeof reorderKeyboardTarget>;
}
/** Mounted measurement supplies supported scheduling and observable resource failures. */
export interface ReorderKeyboardTransport {
  /** Preserve unexpected causes through the mounted cleanup owner. */
  observe<T>(action: () => T): T;
  /** Schedule a genuine supported native read/write request. */
  schedule<T>(request: {
    read(view: EditorView): T;
    write?(measured: T, view: EditorView): void;
    key?: unknown;
  }): void;
  /** Read current owner-created fragments and viewport in the native read phase. */
  geometry(): ReorderMeasuredGeometry;
  /** Read current visual display positions before genuine hide/transform; collision geometry remains native. */
  before(): ReorderMotionOrigin;
  propose(input: ReorderMeasureInput, origin: ReorderMotionOrigin): void;
}
/** Pointer retarget, cancellation and teardown retire the complete ordered keyboard lifetime. */
export class ReorderKeyboardMeasurement {
  private readonly view: EditorView;
  private readonly owner: ReorderMeasureOwner;
  private readonly transport: ReorderKeyboardTransport;
  private readonly base: ReorderCollisionBase;
  private epoch = 0;
  private disposed = false;
  private verticalX: number | null = null;
  private intent: {
    readonly input: ReorderMeasureInput;
    readonly key: ReorderArrow;
    readonly held: number;
    readonly order: ReorderState;
    readonly settled: () => void;
    readonly preferredX: number | null;
    hidden: ReorderState | null;
    seed?: {
      readonly x: number;
      readonly y: number;
    };
    origin?: ReorderMotionOrigin;
  } | null = null;
  /** Bind the actual view, sole session transform authority and finite transport. */
  constructor(view: EditorView, owner: ReorderMeasureOwner, transport: ReorderKeyboardTransport) {
    this.view = view;
    this.owner = owner;
    this.transport = transport;
    this.base = new ReorderCollisionBase(view);
  }
  /** Geometry supersession renews the same genuine hidden publication and retires old read/write/deferred authority. */
  invalidateGeometry(input: ReorderMeasureInput): void {
    if (this.intent?.input === input) this.intent = { ...this.intent };
  }
  /** Renew the retained arrow after synchronous native-batch invalidation. */
  refresh(input: ReorderMeasureInput): boolean {
    const previous = this.intent;
    if (this.disposed || previous?.input !== input || !this.owner.current(input)) return false;
    const renewed = { ...previous };
    this.intent = renewed;
    if (renewed.hidden) this.measure(renewed);
    else this.prepare(renewed);
    return true;
  }
  /** Capture one FIFO arrow intent; the semantic Core slot, not absent held-chip CSS, is its origin. */
  arrow(input: ReorderMeasureInput, key: ReorderArrow, settled: () => void): void {
    const intent = {
      input,
      key,
      settled,
      held: input.held,
      order: input.order,
      preferredX: this.verticalX,
      hidden: null,
    };
    this.intent = intent;
    this.prepare(intent);
  }
  /** Admission prepares one genuine hidden operand while the complete live publication remains visible. */
  private prepare(intent: NonNullable<ReorderKeyboardMeasurement['intent']>): void {
    const epoch = this.epoch;
    this.transport.schedule({
      key: intent.input,
      read: () => {
        if (!this.current(intent, epoch)) return null;
        return this.transport.observe(() => {
          intent.origin = this.transport.before();
          if (intent.preferredX !== null) return null;
          const held = this.transport
            .geometry()
            .rects.find((rect) => rect.kind === 'chip' && rect.index === intent.held);
          return held
            ? {
                x: (held.left + held.right) / 2,
                y: (held.top + held.bottom) / 2,
              }
            : null;
        });
      },
      write: (seed) => {
        queueMicrotask(() => {
          if (!this.current(intent, epoch)) return;
          if (seed) intent.seed = seed;
          else delete intent.seed;
          this.transport.observe(() => {
            const hidden = this.owner.collision(intent.input);
            if (!hidden || !this.current(intent, epoch) || intent.input.hidden !== hidden) return;
            intent.hidden = hidden;
            if (!this.base.prepare(intent.input)) {
              this.finish(intent, null);
              return;
            }
            this.measure(intent);
          });
        });
      },
    });
  }
  /** The shared canonical collision owner measures hidden layout independently of complete live paint. */
  private measure(intent: NonNullable<ReorderKeyboardMeasurement['intent']>): void {
    const epoch = this.epoch,
      { input, hidden } = intent;
    if (!hidden) return;
    const current = (): boolean =>
      this.current(intent, epoch) &&
      input.hidden === hidden &&
      this.view.state.field(reorderPreview, false)?.value?.order === intent.order;
    this.transport.schedule({
      key: input,
      read: () => {
        if (!current()) return null;
        return this.transport.observe(() => {
          const context = this.base.contextAt(input);
          return context ? { context, rects: this.base.current(input, context) } : null;
        });
      },
      write: (measured) => {
        if (!current()) return;
        if (!measured) {
          this.finish(intent, null);
          return;
        }
        this.transport.observe(() => {
          if (!measured.rects) this.base.mount(input, measured.context);
          this.measureBase(intent, measured.context);
        });
      },
    });
  }
  /** Existing native calibration read/write phases preserve actual ancestor placement before target resolution. */
  private measureBase(
    intent: NonNullable<ReorderKeyboardMeasurement['intent']>,
    context: ReorderBaseContext,
  ): void {
    const epoch = this.epoch,
      { input } = intent;
    this.transport.schedule({
      key: input,
      read: () =>
        this.transport.observe(() => {
          if (!this.current(intent, epoch)) return null;
          const read = this.base.measure(input, context);
          return read.kind === 'ready'
            ? {
                kind: 'target' as const,
                context,
                rects: read.rects,
                target: reorderKeyboardTarget(
                  intent.order,
                  requireValue(intent.hidden, 'reorder-keyboard-measurement'),
                  intent.held,
                  this.base.placements(input, context),
                  intent.key,
                  intent.preferredX,
                  intent.seed,
                ),
              }
            : read;
        }),
      write: (read) => {
        if (!this.current(intent, epoch) || !read) return;
        this.transport.observe(() => {
          if (read.kind === 'calibration' && this.base.align(read))
            this.measureBase(intent, context);
          else if (!context.current()) this.measure(intent);
          else this.finish(intent, read.kind === 'target' ? read : null);
        });
      },
    });
  }
  /** One current deferred outcome applies Core authority or restores complete paint and settles FIFO intent once. */
  private finish(
    intent: NonNullable<ReorderKeyboardMeasurement['intent']>,
    measured: ReorderKeyboardGeometryLease | null,
  ): void {
    const epoch = this.epoch,
      { input, hidden } = intent;
    queueMicrotask(() => {
      if (!this.current(intent, epoch) || input.hidden !== hidden) return;
      this.transport.observe(() => {
        if (!measured) this.complete(intent, null);
        else if (this.base.current(input, measured.context) !== measured.rects)
          this.measure(intent);
        else this.verify(intent, measured);
      });
    });
  }
  /** A final native read validates actual computed context; the deferred gap retains the same pure DOM and rectangle leases. */
  private verify(
    intent: NonNullable<ReorderKeyboardMeasurement['intent']>,
    measured: ReorderKeyboardGeometryLease,
  ): void {
    const epoch = this.epoch,
      { input, hidden } = intent;
    this.transport.schedule({
      key: input,
      read: () =>
        this.transport.observe(() => {
          if (!this.current(intent, epoch) || input.hidden !== hidden) return false;
          const context = this.base.contextAt(input);
          return (
            context?.key === measured.context.key &&
            this.base.current(input, context) === measured.rects
          );
        }),
      write: (fresh) =>
        queueMicrotask(() => {
          if (!this.current(intent, epoch) || input.hidden !== hidden) return;
          this.transport.observe(() => {
            if (!fresh || this.base.current(input, measured.context) !== measured.rects)
              this.measure(intent);
            else this.complete(intent, measured.target);
          });
        }),
    });
  }
  /** Only a currently admitted borrowed output settles the sole FIFO source transformation owner. */
  private complete(
    intent: NonNullable<ReorderKeyboardMeasurement['intent']>,
    measured: ReturnType<typeof reorderKeyboardTarget>,
  ): void {
    const { input, hidden } = intent;
    this.intent = null;
    if (measured?.target) {
      this.verticalX = measured.preferredX;
      const outcome = this.owner.prepare(
        input,
        requireValue(hidden, 'reorder-keyboard-measurement'),
        measured.target,
        false,
      );
      if (outcome.kind === 'changed') outcome.apply();
    } else this.owner.restore(input);
    if (intent.origin && this.owner.current(input)) this.transport.propose(input, intent.origin);
    intent.settled();
  }
  /** Source, pointer mode, held identity, order and native lifetime share one pending authority. */
  private current(
    intent: NonNullable<ReorderKeyboardMeasurement['intent']>,
    epoch: number,
  ): boolean {
    const { input } = intent;
    const preview = this.view.state.field(reorderPreview, false)?.value;
    return (
      !this.disposed &&
      epoch === this.epoch &&
      this.intent === intent &&
      input.pointer === null &&
      input.held === intent.held &&
      input.order === intent.order &&
      preview?.token === input.token &&
      preview.capture === input.capture &&
      preview.order === intent.order &&
      this.owner.current(input)
    );
  }
  /** Retire all pending callbacks and the preferred physical column. */
  cancel(): void {
    ++this.epoch;
    this.intent = null;
    this.verticalX = null;
    this.base.destroy();
  }
  /** Teardown cannot settle or reopen an old keyboard intent. */
  destroy(): void {
    this.disposed = true;
    this.cancel();
  }
}
