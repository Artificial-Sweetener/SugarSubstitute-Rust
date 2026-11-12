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
/** One prepared native LoRA role separates full-chip activation from numeric mutation feedback. */
import {
  StateEffect,
  StateField,
  Transaction,
  EditorState,
  type Extension,
} from '@codemirror/state';
import {
  Decoration,
  EditorView,
  ViewPlugin,
  type DecorationSet,
  type ViewUpdate,
} from '@codemirror/view';
import type { SourceRange } from '../../core/src/index.js';
import { editorFreshness, type EditorFreshness } from './freshness.js';
import { featureProfile } from './features.js';
import { snapshotPresentation } from './native-snapshot-policy.js';
import { weightEditingAllowed } from './weight-target.js';
import { exactWeightDraft } from './exact-weight-draft.js';
import { richRendering } from './presentation.js';
import { semanticStructures } from './structure.js';
import type { PointerTarget } from './pointer-targets.js';
import {
  richProjection,
  unmeasuredProjection,
  currentCanonicalProjection,
} from './rich-projection.js';
import { SourceWidget, type SourceToken } from './projection-widgets.js';
import { LoraWidget } from './lora-widget.js';
import type { PreparedProjection } from './projection-contracts.js';
import { reorderPreview } from './reorder-preview.js';
import { reorderGrabCursor } from './reorder-grab-cursor.js';

/** A logical-document UTF-16 hover retains the actual existing source-qualified pointer target. */
export interface LoraHover extends SourceRange {
  readonly freshness: EditorFreshness;
  readonly target: PointerTarget;
}
/** Native observations publish one bounded hover; they never authorize mutation. */
export const setLoraHover = StateEffect.define<LoraHover | null>();
/** CodeMirror and native window focus observations supply no parallel caret identity. */
export const setLoraFocus = StateEffect.define<boolean>();
/** Actual document focus remains distinct from CM content focus while its existing numeric input owns focus. */
export const setLoraApplicationFocus = StateEffect.define<boolean>();
/** One field retains bounded native pointer/focus observations; current selection and canonical indexes derive activation. */
interface LoraIntent {
  readonly hover: LoraHover | null;
  readonly focused: boolean;
  readonly applicationFocused: boolean;
}
/** Source/presentation permissions and the actual spatial owner admit passive LoRA feedback. */
function allowed(state: EditorState): boolean {
  return (
    weightEditingAllowed(state, 'lora') &&
    state.facet(richRendering) &&
    !state.field(reorderPreview, false)?.value &&
    !state.field(reorderGrabCursor, false)
  );
}
/** One captured canonical token is classified through its actual atomic prefix without revisiting its ancestors. */
function tokenRole(
  projection: PreparedProjection,
  from: number,
  token: SourceToken,
): 'lora' | 'other' | null {
  let role: 'lora' | 'other' | null = null;
  projection.allAtomic.between(from, from, (_start, _end, value) => {
    const widget: unknown = value.spec.widget;
    if (widget instanceof SourceWidget && widget.token === token)
      role = widget instanceof LoraWidget ? 'lora' : 'other';
  });
  return role;
}
/** Exact canonical membership resolves one token once before the shared actual-widget identity gate. */
function projectedRole(
  projection: PreparedProjection,
  from: number,
  to: number,
): 'lora' | 'other' | null {
  let selected: SourceToken | null = null;
  projection.tokens.between(from, from, (start, end, token) => {
    if (start === from && end === to) selected = token;
  });
  return selected ? tokenRole(projection, from, selected) : null;
}
/** Look up one current logical-document UTF-16 point without parsing, geometry or a token roster copy. */
export function loraHoverAt(state: EditorState, position: number): LoraHover | null {
  const freshness = state.field(editorFreshness),
    prepared = state.field(semanticStructures).native,
    projection = currentCanonicalProjection(state);
  if (!allowed(state) || prepared?.sourceIdentity !== freshness.source || !projection) return null;
  let result: LoraHover | null = null;
  prepared.pointerTargets.between(position, position, (from, to, target) => {
    if (
      target.kind === 'lora' &&
      from === position &&
      projectedRole(projection, from, to) === 'lora'
    )
      result = { from, to, target, freshness };
  });
  return result;
}
/** Source/context and disabled presentation retire pointer intent before another paint can reuse it. */
export const loraIntent = StateField.define<LoraIntent>({
  /** No pointer or native focus is assumed before the real view mounts. */
  create: () => ({ hover: null, focused: false, applicationFocused: false }),
  /** Accept only the existing current index member; source/caret/history remain CodeMirror-owned. */
  update(value, transaction) {
    let hover = value.hover,
      focused = value.focused,
      applicationFocused = value.applicationFocused;
    if (hover) {
      const current = loraHoverAt(transaction.state, hover.from);
      if (
        hover.freshness !== transaction.state.field(editorFreshness) ||
        current?.target !== hover.target ||
        current.to !== hover.to
      )
        hover = null;
    }
    for (const effect of transaction.effects) {
      if (effect.is(setLoraFocus)) focused = effect.value;
      if (effect.is(setLoraApplicationFocus)) applicationFocused = effect.value;
      if (effect.is(setLoraHover)) {
        const next = effect.value,
          current = next ? loraHoverAt(transaction.state, next.from) : null;
        hover =
          next &&
          current?.target === next.target &&
          current.to === next.to &&
          next.freshness === current.freshness
            ? next
            : null;
      }
    }
    return hover === value.hover &&
      focused === value.focused &&
      applicationFocused === value.applicationFocused
      ? value
      : { hover, focused, applicationFocused };
  },
});
/** Current native caret/association resolves projection-token precedence; ordinary text leaves hover eligible. */
export function activeLoraRange(state: EditorState): SourceRange | null {
  const intent = state.field(loraIntent, false),
    prepared = state.field(semanticStructures).native,
    projection = currentCanonicalProjection(state),
    freshness = state.field(editorFreshness);
  if (
    !intent ||
    !intent.applicationFocused ||
    !allowed(state) ||
    prepared?.sourceIdentity !== freshness.source ||
    projection?.sourceIdentity !== freshness.source ||
    !projection.loraEnabled ||
    !projection.hasLoras
  )
    return null;
  const draft = state.field(exactWeightDraft, false);
  if (draft?.phase === 'editing' && draft.freshness === freshness)
    return draft.kind === 'lora' &&
      projectedRole(projection, draft.range.from, draft.range.to) === 'lora'
      ? draft.range
      : null;
  const selection = state.selection.main;
  if (intent.focused && selection.empty) {
    let focused: {
      readonly from: number;
      readonly to: number;
      readonly token: SourceToken;
    } | null = null;
    projection.tokens.between(selection.head, selection.head, (from, to, token) => {
      if (
        (from === selection.head && selection.assoc < 0) ||
        (to === selection.head && selection.assoc > 0)
      )
        return;
      if (
        !focused ||
        to - from < focused.to - focused.from ||
        (to - from === focused.to - focused.from && from === selection.head)
      )
        focused = { from, to, token };
    });
    const current = focused as {
      readonly from: number;
      readonly to: number;
      readonly token: SourceToken;
    } | null;
    if (current) {
      const role = tokenRole(projection, current.from, current.token);
      if (role) return role === 'lora' ? { from: current.from, to: current.to } : null;
    }
  }
  return intent.hover?.freshness === freshness
    ? { from: intent.hover.from, to: intent.hover.to }
    : null;
}
/** Inclusive outer marks cover whole replacement widgets at their exact boundaries without editing managed DOM or numeric descriptors. */
export function loraActiveDecorations(state: EditorState): DecorationSet {
  const range = activeLoraRange(state);
  return range
    ? Decoration.set([
        Decoration.mark({ class: 'sp-lora-active', inclusive: true }).range(range.from, range.to),
      ])
    : Decoration.none;
}
/** One native view owns pointer coordinates and finite application-focus listeners, retaining no measured geometry. */
export class LoraActiveView {
  private readonly view: EditorView;
  private disposed = false;
  private generation = 0;
  private focusGeneration = 0;
  private pointer: Readonly<{ x: number; y: number }> | null = null;
  private pendingHover = false;
  private readonly window: Window;
  /** Window loss retires both input intents even if the native content element retains DOM focus. */
  private readonly blur = (): void => {
    this.pointer = null;
    this.pendingHover = false;
    this.generation++;
    this.publish(null, false, false);
  };
  /** Window restoration reads the existing native focus owner; it cannot reactivate departed hover. */
  private readonly focus = (): void => {
    if (!this.disposed)
      this.publish(null, this.view.hasFocus, this.view.dom.ownerDocument.hasFocus());
  };
  /** Register every listener under one all-attempt admission/retirement boundary. */
  constructor(view: EditorView) {
    this.view = view;
    const window = view.dom.ownerDocument.defaultView;
    if (!window) throw new Error('LORA_ACTIVE_WINDOW_UNAVAILABLE');
    this.window = window;
    try {
      window.addEventListener('blur', this.blur);
      window.addEventListener('focus', this.focus);
      this.synchronizeFocus();
    } catch (primary) {
      const failures: unknown[] = [primary];
      this.disposed = true;
      this.unobserve(failures);
      throw failures.length === 1
        ? primary
        : new AggregateError(failures, 'LORA_ACTIVE_ADMISSION_FAILED');
    }
  }
  /** Native pointer ancestry identifies only a genuine CodeMirror chip; external readonly copies have no hover authority. */
  hover(event: MouseEvent): void {
    if (this.disposed) return;
    const target = event.target;
    this.generation++;
    this.pendingHover = false;
    const chip = target instanceof Element ? target.closest<HTMLElement>('.sp-lora-chip') : null;
    const next =
      chip && this.view.contentDOM.contains(chip)
        ? loraHoverAt(this.view.state, this.view.posAtDOM(chip, 0))
        : null;
    this.pointer =
      next && Number.isFinite(event.clientX) && Number.isFinite(event.clientY)
        ? { x: event.clientX, y: event.clientY }
        : null;
    this.publish(next);
  }
  /** Departure and native authoring retire hover immediately rather than waiting for an unrelated frame publication. */
  leave(): void {
    this.pointer = null;
    this.pendingHover = false;
    this.generation++;
    this.publish(null);
  }
  /** Revalidate stationary hover in CM's READ phase; incidental widget updates retain the same actual hit while relocation retires it. */
  update(update: ViewUpdate): void {
    if (this.disposed) return;
    this.synchronizeFocus();
    const state = update.state,
      captured = state.field(loraIntent).hover,
      point = this.pointer;
    if (!captured || !point) {
      this.pendingHover = false;
      return;
    }
    if (!update.geometryChanged && !update.viewportChanged && !this.pendingHover) return;
    this.pendingHover = true;
    const generation = ++this.generation;
    const current = (): boolean =>
      !this.disposed &&
      generation === this.generation &&
      this.pointer === point &&
      this.view.state === state &&
      state.field(loraIntent).hover === captured;
    this.view.requestMeasure({
      key: this,
      /** One exact native hit and prepared point lookup discover no roster, rectangles or source. */
      read: () => {
        if (!current()) return null;
        const hit = this.view.dom.ownerDocument.elementFromPoint(point.x, point.y);
        if (!current()) return null;
        const chip = hit?.closest<HTMLElement>('.sp-lora-chip');
        if (!current()) return null;
        const contained = chip && this.view.contentDOM.contains(chip);
        if (!current()) return null;
        const position = chip && contained ? this.view.posAtDOM(chip, 0) : null;
        if (!current()) return null;
        const next = position === null ? null : loraHoverAt(state, position);
        return current() ? next : null;
      },
      /** Only this exact request may publish after CM's write phase; focus/source/intent replacement revokes old work. */
      write: (next) =>
        queueMicrotask(() => {
          if (!current()) return;
          this.pendingHover = false;
          this.publish(
            next?.target === captured.target &&
              next.freshness === captured.freshness &&
              next.from === captured.from &&
              next.to === captured.to
              ? captured
              : null,
          );
        }),
    });
  }
  /** Rebuilt CM plugins may inherit native focus without a focus event; only exact current post-update work may synchronize it. */
  private synchronizeFocus(): void {
    const state = this.view.state,
      generation = ++this.focusGeneration;
    queueMicrotask(() => {
      if (this.disposed || generation !== this.focusGeneration || this.view.state !== state) return;
      this.publish(
        state.field(loraIntent).hover,
        this.view.hasFocus,
        this.view.dom.ownerDocument.hasFocus(),
      );
    });
  }
  /** No-op observations reuse field/mark output; passive effects never create document-history entries. */
  private publish(hover: LoraHover | null, focused?: boolean, applicationFocused?: boolean): void {
    if (this.disposed) return;
    const current = this.view.state.field(loraIntent);
    const same =
      current.hover?.target === hover?.target && current.hover?.freshness === hover?.freshness;
    if (
      same &&
      (focused === undefined || focused === current.focused) &&
      (applicationFocused === undefined || applicationFocused === current.applicationFocused)
    )
      return;
    this.generation++;
    this.view.dispatch({
      effects: [
        setLoraHover.of(hover),
        ...(focused === undefined ? [] : [setLoraFocus.of(focused)]),
        ...(applicationFocused === undefined
          ? []
          : [setLoraApplicationFocus.of(applicationFocused)]),
      ],
      annotations: [Transaction.addToHistory.of(false), snapshotPresentation.of(true)],
    });
  }
  /** Every listener removal is attempted independently, preserving raw primary and cleanup causes. */
  private unobserve(failures: unknown[]): void {
    for (const [name, listener] of [
      ['blur', this.blur],
      ['focus', this.focus],
    ] as const) {
      try {
        this.window.removeEventListener(name, listener);
      } catch (cause) {
        failures.push(cause);
      }
    }
  }
  /** Teardown cancels queued invalidation before independently removing both native listeners. */
  destroy(): void {
    if (this.disposed) return;
    this.disposed = true;
    this.pointer = null;
    this.pendingHover = false;
    this.generation++;
    this.focusGeneration++;
    const failures: unknown[] = [];
    this.unobserve(failures);
    if (failures.length === 1) throw failures[0];
    if (failures.length) throw new AggregateError(failures, 'LORA_ACTIVE_RETIREMENT_FAILED');
  }
}
/** Native focus effects and one prepared outer-mark provider consume the same finite LoRA intent owner. */
export const loraActiveFeedback: Extension = [
  loraIntent,
  EditorView.focusChangeEffect.of((_state, focused) => setLoraFocus.of(focused)),
  EditorView.outerDecorations.compute(
    [
      loraIntent,
      exactWeightDraft,
      semanticStructures,
      richProjection,
      unmeasuredProjection,
      reorderPreview,
      reorderGrabCursor,
      featureProfile,
      richRendering,
      EditorState.readOnly,
      editorFreshness,
      'selection',
    ],
    loraActiveDecorations,
  ),
  ViewPlugin.fromClass(LoraActiveView, {
    eventObservers: {
      /** Title, backing and numeric cells share actual chip hover identity. */
      mousemove(event) {
        this.hover(event);
      },
      /** A real surface departure clears the sole pointer role synchronously. */
      mouseleave() {
        this.leave();
      },
      /** Native authoring switches focus/caret intent without retaining a stale pointer target. */
      keydown() {
        this.leave();
      },
      /** Existing native scroll ownership invalidates physical pointer placement. */
      scroll() {
        this.leave();
      },
      /** Content focus transfer cannot retain departed hover. */
      blur() {
        this.leave();
      },
    },
  }),
];
