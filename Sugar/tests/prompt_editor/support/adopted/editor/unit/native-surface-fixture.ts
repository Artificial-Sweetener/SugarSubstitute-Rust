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
/** Finite inert native transport delegates document, history and original focus intent to their actual production owners. */
import { EditorSelection, EditorState, Transaction, type TransactionSpec } from '@codemirror/state';
import type { EditorView } from '@codemirror/view';
import {
  NativeSurface,
  type NativeViewFactory,
} from '../../../../../../frontend/sugarprompt/editor/src/native-surface.ts';
import { historyTransactions } from '../../../../../../frontend/sugarprompt/editor/src/prompt-history.ts';
import { hostFixture } from '../transaction/host-document-fixture.ts';

/** The seam supplies focus and measurement observations only; CodeMirror owns all state, selection and history. */
export function nativeSurfaceFixture() {
  const body = {},
    external = {},
    root = { activeElement: null as unknown };
  const parent = { ownerDocument: { body } } as unknown as HTMLElement;
  const measurements: (() => void)[] = [];
  let focusout: (() => void) | undefined;
  let focuses = 0,
    publications = 0,
    destroyed = 0;
  const create: NativeViewFactory = (_parent, initial) => {
    let state = initial;
    const content = {};
    const view = {
      root,
      dom: { contains: (value: unknown) => value === content },
      scrollDOM: { clientHeight: 50 },
      /** Expose the current real immutable native state. */
      get state() {
        return state;
      },
      /** Observe the controlled destination without geometry or platform input. */
      get hasFocus() {
        return root.activeElement === content;
      },
      plugin: () => null,
      composing: false,
      compositionStarted: false,
      /** Preserve native transaction identity and delegate history batching to its actual owner. */
      dispatch(input: Transaction | readonly Transaction[] | TransactionSpec) {
        const transactions: readonly Transaction[] = Array.isArray(input)
          ? (input as readonly Transaction[])
          : [input instanceof Transaction ? input : state.update(input as TransactionSpec)];
        state = historyTransactions(transactions).at(-1)!.state;
      },
      /** Adopt the prepared native baseline without rebuilding source or history. */
      setState(value: EditorState) {
        state = value;
      },
      /** Record focus delivery to the current finite surface. */
      focus() {
        root.activeElement = content;
        focuses++;
      },
      scrollSnapshot: () => ({}),
      /** Retain supported native measurement callbacks for deterministic owner-controlled delivery. */
      requestMeasure(request: NonNullable<Parameters<EditorView['requestMeasure']>[0]>) {
        measurements.push(() => {
          const value = request.read(view as unknown as EditorView);
          request.write?.(value, view as unknown as EditorView);
        });
      },
      /** Deliver one synchronous focusout during native view retirement. */
      destroy() {
        destroyed++;
        if (root.activeElement === content) {
          root.activeElement = body;
          const callback = focusout;
          focusout = undefined;
          callback?.();
        }
      },
    };
    return view as unknown as EditorView;
  };
  const host = hostFixture('abcd');
  let state = host.state();
  state = state.update({
    changes: { from: 0, insert: 'x' },
    userEvent: 'input.type',
  }).state;
  state = state.update({ selection: EditorSelection.single(4, 1) }).state;
  const surface = new NativeSurface(parent, state, () => publications++, create);
  return {
    surface,
    root,
    body,
    external,
    host,
    /** Arm one independently controlled host callback for the next retirement. */
    focusout(callback: () => void) {
      focusout = callback;
    },
    counts: () => ({
      focuses,
      publications,
      destroyed,
      measures: measurements.length,
    }),
    /** Flush only actual scheduled native measurement callbacks and their publication microtasks. */
    async flush() {
      while (measurements.length) {
        measurements.shift()!();
        await Promise.resolve();
      }
    },
  };
}
