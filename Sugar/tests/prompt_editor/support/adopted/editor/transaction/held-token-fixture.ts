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
/** Finite controlled public-view adapters exercise the real mouse factory and native immutable state without DOM/browser ownership. */
import assert from 'node:assert/strict';
import {
  Compartment,
  StateEffect,
  type EditorState,
  type TransactionSpec,
} from '@codemirror/state';
import { EditorView, type MouseSelectionStyle, type ViewUpdate } from '@codemirror/view';
import { projectionState } from './projection-state.ts';
import { prepareProjection } from '../../../../../../frontend/sugarprompt/editor/src/projection-preparation.ts';
import {
  initializeNativeStructure,
  prepareNativeStructure,
} from '../../../../../../frontend/sugarprompt/editor/src/structure.ts';
import { installProjection } from '../../../../../../frontend/sugarprompt/editor/src/rich-projection.ts';
import {
  PromptMouseSelection,
  promptMouseSelection,
} from '../../../../../../frontend/sugarprompt/editor/src/mouse-selection.ts';
import type { PromptDocumentKind } from '../../../../../../frontend/sugarprompt/core/src/index.ts';

/** Actual EventTarget delivery and callback identity expose finite listener admission/removal without browser layout. */
class OwnedEventTarget extends EventTarget {
  private readonly phases = new Map<string, boolean>();
  private readonly listeners = new Map<string, Set<EventListenerOrEventListenerObject>>();
  /** Count genuinely retained external callbacks, rather than method invocation counters. */
  get active(): number {
    return [...this.listeners.values()].reduce((total, listeners) => total + listeners.size, 0);
  }
  /** Retain the exact callback accepted by the actual EventTarget. */
  override addEventListener(
    type: string,
    callback: EventListenerOrEventListenerObject | null,
    options?: AddEventListenerOptions | boolean,
  ): void {
    super.addEventListener(type, callback, options);
    this.phases.set(type, typeof options === 'boolean' ? options : (options?.capture ?? false));
    if (callback) {
      const listeners = this.listeners.get(type) ?? new Set();
      listeners.add(callback);
      this.listeners.set(type, listeners);
    }
  }
  /** Release precisely the external callback whose delivery would otherwise survive view disposal. */
  override removeEventListener(
    type: string,
    callback: EventListenerOrEventListenerObject | null,
    options?: EventListenerOptions | boolean,
  ): void {
    super.removeEventListener(type, callback, options);
    if (callback) this.listeners.get(type)?.delete(callback);
  }
  /** Deliver the real admitted observer at the independently specified DOM propagation phase. */
  phase(type: string, event: Event, capture: boolean): void {
    if (this.phases.get(type) !== capture) return;
    for (const callback of this.listeners.get(type) ?? [])
      if (typeof callback === 'function') callback.call(this, event);
      else callback.handleEvent(event);
  }
}
/** Produce trusted-state inputs through actual semantic and projection preparation owners. */
export function prepareHeldProjection(state: EditorState): EditorState {
  state = initializeNativeStructure(state);
  return state.update({
    effects: installProjection.of(prepareProjection(state, prepareNativeStructure(state))),
  }).state;
}

/** Inert public event scalars carry the controlled logical hit position through clientX. */
export function heldTokenPointer(position: number, detail = 2, shiftKey = false): MouseEvent {
  return {
    button: 0,
    buttons: 1,
    detail,
    shiftKey,
    ctrlKey: false,
    altKey: false,
    metaKey: false,
    clientX: position,
    clientY: 0,
    target: null,
  } as MouseEvent;
}

/** Supply only supported view APIs to the real owner; actual native interaction remains a separate browser gate. */
export function heldTokenFixture(
  source = 'alpha <lora:real:1>, tail',
  initialAnchor = 0,
  kind: PromptDocumentKind = 'prompt',
) {
  let state = prepareHeldProjection(projectionState(source, new Compartment(), kind));
  state = state.update({
    selection: { anchor: initialAnchor },
    effects: StateEffect.appendConfig.of(promptMouseSelection),
  }).state;
  const document = new OwnedEventTarget(),
    window = new OwnedEventTarget();
  Object.assign(document, { defaultView: window });
  let disposed = false,
    style: MouseSelectionStyle | null = null;
  let readAdmission: (() => void) | null = null;
  let hitSide: -1 | 1 = 1;
  /** Real transactions publish state before plugin and active style updates, matching the native owner cadence. */
  function dispatch(spec: TransactionSpec): void {
    const startState = state,
      transaction = state.update(spec);
    state = transaction.state;
    const update = {
      startState,
      state,
      transactions: [transaction],
      docChanged: transaction.docChanged,
      changes: transaction.changes,
    } as unknown as ViewUpdate;
    owner.update(update);
    style?.update(update);
  }
  const view = {
    /** Access after teardown is an invariant failure, rather than an ignored private CodeMirror behavior. */
    get state() {
      assert.equal(disposed, false);
      return state;
    },
    contentDOM: { ownerDocument: document },
    compositionStarted: false,
    /** A focus action would violate the supported factory contract. */
    focus() {
      assert.fail('mouse factory cannot focus');
    },
    /** Controlled external coordinates exercise the actual style continuation and may publish a genuine nested update. */
    posAndSideAtCoords(point: { x: number }) {
      const callback = readAdmission;
      readAdmission = null;
      callback?.();
      return { pos: point.x, assoc: hitSide };
    },
    /** Public plugin transport resolves only the actual mounted native owner. */
    plugin() {
      return owner;
    },
    dispatch,
  } as unknown as EditorView;
  const owner = new PromptMouseSelection(view);
  return {
    get state() {
      return state;
    },
    owner,
    document,
    window,
    dispatch,
    view,
    /** The registered public facet chooses the actual production style factory. */
    press(event = heldTokenPointer(8)) {
      style = state.facet(EditorView.mouseSelectionStyle)[0]!(view, event);
      assert.ok(style);
      return { event, style };
    },
    /** Publish the exact selection returned by the style, through native select.pointer ownership. */
    select(style: MouseSelectionStyle, event: MouseEvent, extend = false, multiple = false) {
      dispatch({
        selection: style.get(event, extend, multiple),
        userEvent: 'select.pointer',
      });
    },
    /** A controlled read can exercise source/context reentry before style admission. */
    duringRead(callback: () => void) {
      readAdmission = callback;
    },
    /** Choose which side of the hit offset the next coordinate reads land on, as at a soft wrap. */
    hitSide(side: -1 | 1) {
      hitSide = side;
    },
    /** Source-only admission controls invoke the same registered public style factory without assuming success. */
    admitted(event = heldTokenPointer(8)) {
      return state.facet(EditorView.mouseSelectionStyle)[0]!(view, event);
    },
    /** Dispose removes every passive observer before the view becomes inaccessible. */
    destroy() {
      owner.destroy();
      disposed = true;
    },
  };
}
