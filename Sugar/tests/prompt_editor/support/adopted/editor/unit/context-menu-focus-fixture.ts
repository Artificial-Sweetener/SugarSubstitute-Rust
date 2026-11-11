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
/** Actual Surface/Focus/Menu owners share controlled platform listeners and measurement delivery; no native DOM/scroll behavior is certified. */
import { Transaction, type TransactionSpec } from '@codemirror/state';
import type { ViewUpdate } from '@codemirror/view';
import { NativeContextMenu } from '../../../../../../frontend/sugarprompt/editor/src/context-menu.ts';
import {
  contextMenu,
  installContextMenu,
} from '../../../../../../frontend/sugarprompt/editor/src/context-menu-state.ts';
import { nativeSurfaceFixture } from './native-surface-fixture.ts';
import { nativeFocusHandoff } from '../../../../../../frontend/sugarprompt/editor/src/focus-handoff.ts';
import type { FocusHandoff } from '../../../../../../frontend/sugarprompt/editor/src/focus-handoff-contracts.ts';
import { installLoraPresentation } from '../../../../../../frontend/sugarprompt/editor/src/lora-presentation.ts';

/** Only the external instanceof/closest transport is supplied for listener routing. */
class ListenerElement {
  /** This closed ordinary-text specimen has no input, chip or overlay descendant. */
  closest(): null {
    return null;
  }
}
/** Deliver real owner requests and supported measurement callbacks, keeping CM Undo scroll separate from NativeFocus reveal effects. */
export function menuFocusFixture() {
  const native = nativeSurfaceFixture(),
    view = native.surface.view;
  const descriptor = Object.getOwnPropertyDescriptor(globalThis, 'Element');
  Object.defineProperty(globalThis, 'Element', {
    configurable: true,
    value: ListenerElement,
  });
  const listeners = new Map<string, Set<unknown>>();
  /** Retain the exact external listener identity under its original event name. */
  function add(name: string, callback: unknown): void {
    const entries = listeners.get(name) ?? new Set();
    entries.add(callback);
    listeners.set(name, entries);
  }
  /** Release only the exact owned external identity. */
  function remove(name: string, callback: unknown): void {
    listeners.get(name)?.delete(callback);
  }
  const window = { addEventListener: add, removeEventListener: remove };
  const document = {
    defaultView: window,
    addEventListener: add,
    removeEventListener: remove,
  };
  Object.assign(view.dom, { ownerDocument: document });
  Object.assign(view, {
    contentDOM: { contains: () => true },
    coordsAtPos: () => ({ left: 10, right: 11, top: 20, bottom: 39 }),
    posAtCoords: () => 0,
  });
  const focus = view.state.facet(nativeFocusHandoff);
  if (!focus) throw new Error('actual-surface-focus-facet-missing');
  const capture = focus.capture;
  const capabilities: FocusHandoff[] = [];
  /** Observe exact production capability identity; admission and focus still delegate to the actual Surface. */
  focus.capture = (purpose) => {
    const handoff = capture(purpose);
    capabilities.push(handoff);
    return handoff;
  };
  const owner = new NativeContextMenu(view, () => undefined);
  const dispatch = view.dispatch.bind(view);
  let reveals = 0,
    undoScrolls = 0;
  const pendingUndo: (() => void)[] = [];
  /** Model an independently delivered platform scroll, without inventing geometry or changing source. */
  function scroll(): void {
    for (const callback of listeners.get('scroll') ?? [])
      if (typeof callback === 'function') callback({ target: null, composedPath: () => [] });
  }
  Object.assign(view, {
    /** Production transactions/history remain untouched; only their external scroll delivery is controlled. */
    dispatch(input: Transaction | TransactionSpec) {
      const startState = view.state;
      const transaction = input instanceof Transaction ? input : startState.update(input);
      dispatch(transaction);
      /** Actual transaction facts supply the complete public update shape; viewport/focus flags remain controlled platform observations. */
      const update: ViewUpdate = {
        view,
        startState,
        state: view.state,
        transactions: [transaction],
        changes: transaction.changes,
        viewportChanged: false,
        viewportMoved: false,
        heightChanged: false,
        geometryChanged: transaction.docChanged,
        focusChanged: false,
        docChanged: transaction.docChanged,
        selectionSet: transaction.selection !== undefined,
      };
      owner.update(update);
      if (transaction.scrollIntoView) {
        undoScrolls++;
        pendingUndo.push(scroll);
      }
      // In this closed fixture the only non-menu/resource effect producer is the actual NativeFocus owner.
      if (
        transaction.effects.length &&
        transaction.effects.every(
          (effect) => !effect.is(installContextMenu) && !effect.is(installLoraPresentation),
        )
      ) {
        reveals++;
        scroll();
      }
    },
  });
  return {
    native,
    view,
    owner,
    counts: () => ({
      reveals,
      undoScrolls,
      pendingUndo: pendingUndo.length,
      captures: capabilities.length,
    }),
    /** Observe only the actual last issued capability for release/transfer assertions. */
    captured: () => capabilities.at(-1),
    /** Invoke the actual local keyboard owner, not a private request method. */
    local(): boolean {
      return owner.key({
        key: 'F10',
        shiftKey: true,
        ctrlKey: false,
        altKey: false,
        metaKey: false,
        target: null,
        preventDefault() {},
      } as unknown as KeyboardEvent);
    },
    /** Invoke actual pointer targeting/refocus transport using a closed inert ordinary-text hit. */
    pointer(): boolean {
      return owner.context({
        button: 2,
        clientX: 20,
        clientY: 30,
        target: new ListenerElement(),
        preventDefault() {},
      } as unknown as MouseEvent);
    },
    /** Route a genuine outside listener call; no managed DOM or source is changed. */
    outside(): void {
      for (const callback of listeners.get('pointerdown') ?? [])
        if (typeof callback === 'function') callback({ target: null, composedPath: () => [] });
    },
    /** Restore through the existing owner key path after controlled popup detachment. */
    escape(): boolean {
      native.root.activeElement = native.body;
      return owner.key({
        key: 'Escape',
        target: null,
        preventDefault() {},
      } as unknown as KeyboardEvent);
    },
    /** Deliver captured CM-owned Undo scroll independently of focus measurement. */
    deliverUndoScroll(): void {
      pendingUndo.shift()?.();
    },
    /** Observe real menu publication through its actual state field. */
    installed() {
      return view.state.field(contextMenu);
    },
    /** Every real owner and original global transport is independently retired. */
    destroy(): void {
      const causes: unknown[] = [];
      for (const release of [
        () => owner.destroy(),
        () => native.surface.dispose(),
        () => {
          if (descriptor) Object.defineProperty(globalThis, 'Element', descriptor);
          else Reflect.deleteProperty(globalThis, 'Element');
        },
      ]) {
        try {
          release();
        } catch (cause) {
          causes.push(cause);
        }
      }
      if (causes.length === 1) throw causes[0];
      if (causes.length) throw new AggregateError(causes, 'menu-focus-fixture-retirement-failed');
    },
  };
}
