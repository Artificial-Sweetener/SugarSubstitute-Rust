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
/** Supported-view geometry adapters connect the real mouse, deletion, projection and native history owners without a browser. */
import { EditorSelection, StateEffect, Transaction, type TransactionSpec } from '@codemirror/state';
import { Direction, EditorView, type MouseSelectionStyle, type ViewUpdate } from '@codemirror/view';
import { PromptMouseSelection } from '../../../../../../frontend/sugarprompt/editor/src/mouse-selection.ts';
import {
  promptHistory,
  historyTransactions,
} from '../../../../../../frontend/sugarprompt/editor/src/prompt-history.ts';
import { nativeSourceCommands } from '../../../../../../frontend/sugarprompt/editor/src/native-source-commands.ts';
import { deletionFixture } from './rich-deletion-fixture.ts';

/** One real document/history fixture accepts controlled hit positions and LTR visual steps; native browser proof still owns actual geometry. */
export function structuralExposureFixture(source: string) {
  const native = deletionFixture(source);
  const document = new EventTarget();
  Object.assign(document, { defaultView: new EventTarget() });
  let style: MouseSelectionStyle | null = null;
  const boundary: unknown = Object.create(EditorView.prototype);
  if (!(boundary instanceof EditorView)) throw new Error('Structural view adapter unavailable');
  const view = new Proxy(boundary, {
    /** Adapt only supported geometry and observer transport while every document/selection/history update remains native. */
    get(_target, property) {
      if (property === 'contentDOM') return { ownerDocument: document };
      if (property === 'hasFocus') return true;
      if (property === 'posAtCoords') return (point: { x: number }) => point.x;
      if (property === 'posAndSideAtCoords')
        return (point: { x: number }) => ({ pos: point.x, assoc: 1 });
      if (property === 'textDirectionAt') return () => Direction.LTR;
      if (property === 'bidiSpans')
        return (line: { length: number }) => [{ from: 0, to: line.length, level: 0 }];
      if (property === 'moveByChar')
        return (
          range: ReturnType<typeof EditorSelection.cursor>,
          forward: boolean,
          group?: (initial: string) => (next: string) => boolean,
        ) => {
          const head = Math.max(
            0,
            Math.min(native.state().doc.length, range.head + (forward ? 1 : -1)),
          );
          group?.(
            native.state().doc.sliceString(Math.min(range.head, head), Math.max(range.head, head)),
          );
          return EditorSelection.cursor(head, forward ? -1 : 1);
        };
      if (property === 'dispatch') return dispatch;
      return Reflect.get(native.view, property);
    },
  });
  /** Feed the complete supported native history transaction expansion to all genuine current owners. */
  function dispatch(spec: TransactionSpec | Transaction): void {
    const first = spec instanceof Transaction ? spec : native.state().update(spec);
    for (const transaction of historyTransactions([first])) {
      const startState = native.state();
      native.view.dispatch(transaction);
      const update = {
        startState,
        state: native.state(),
        transactions: [transaction],
        docChanged: transaction.docChanged,
        changes: transaction.changes,
      } as unknown as ViewUpdate;
      mouse?.update(update);
      style?.update(update);
    }
  }
  native.view.dispatch({ effects: StateEffect.appendConfig.of(promptHistory) });
  const mouse = new PromptMouseSelection(view);
  let disposed = false;
  const commands = nativeSourceCommands(
    () => view,
    () => undefined,
    () => {
      if (disposed) throw new Error('Structural fixture disposed');
    },
    {
      /** No external host continuation is admitted by this native owner fixture. */
      validate: () => false,
      /** No host handoff is retained; selection/source retirement remains in the actual native transaction owners. */
      accept() {},
    },
  );
  return {
    view,
    commands: commands.api,
    state: native.state,
    dispatch,
    /** Invoke the actual production pointer style and its native selection result from a controlled hit coordinate. */
    press(position: number, detail: number): void {
      const event = {
        button: 0,
        buttons: 1,
        detail,
        shiftKey: false,
        ctrlKey: false,
        altKey: false,
        metaKey: false,
        clientX: position,
        clientY: 0,
        target: null,
      } as MouseEvent;
      style = mouse!.style(event);
      if (!style) throw new Error('Structural pointer style unavailable');
      dispatch({
        selection: style.get(event, false, false),
        userEvent: 'select.pointer',
      });
    },
    /** Release the actual mouse reservation using its admitted passive observer. */
    release(): void {
      document.dispatchEvent(new Event('mouseup'));
    },
    /** Run genuine semantic preparation and native measured publication through the existing fixture owner. */
    settle(): void {
      native.drain();
      native.measure();
      native.drain();
    },
    /** Release both genuine finite owners, preserving primary and cleanup causes. */
    destroy(): void {
      if (disposed) return;
      disposed = true;
      const causes: unknown[] = [];
      for (const release of [
        () => commands.dispose(),
        () => mouse.destroy(),
        () => native.destroy(),
      ]) {
        try {
          release();
        } catch (cause) {
          causes.push(cause);
        }
      }
      if (causes.length === 1) throw causes[0];
      if (causes.length)
        throw new AggregateError(causes, 'Structural exposure fixture cleanup failed');
    },
  };
}
