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
/** Controlled native measurement/clock transport proves passive preview lifetime against actual accepted CodeMirror state. */
import { type EditorState } from '@codemirror/state';
import { type EditorView, type ViewUpdate } from '@codemirror/view';
import { WeightPreview } from '../../../../../../frontend/sugarprompt/editor/src/weight-preview.ts';
import { weightStateFor } from '../transaction/weight-command-state.ts';
import { prepareWeightChange } from '../../../../../../frontend/sugarprompt/editor/src/weight-command.ts';
import { editorFreshness } from '../../../../../../frontend/sugarprompt/editor/src/freshness.ts';
import { weightSession } from '../../../../../../frontend/sugarprompt/editor/src/weight-session.ts';

/** Supported requestMeasure transport observes only the public read/write protocol. */
type Measure = NonNullable<Parameters<EditorView['requestMeasure']>[0]>;

/** Prepare one real pointer-owned source mutation and canonical post-state. */
export function adjusted(
  state: EditorState = weightStateFor('(cat:1.05), (dog:2.00)'),
  range = { from: 1, to: 4 },
): EditorState {
  return prepareWeightChange(
    state,
    { kind: 'step', direction: 'increase' },
    {
      owner: 'pointer',
      freshness: state.field(editorFreshness),
      kind: 'emphasis',
      range,
    },
  ).at(-1)!.state;
}

/** Literal external DOM/clock boundary retains finite observable resources without pretending to certify browser layout. */
export function previewFixture() {
  let state = adjusted(),
    removed = 0,
    nextTimer = 0,
    retired = false;
  const jobs: Measure[] = [],
    timers = new Map<number, { callback: () => void; delay: number }>(),
    listeners = new Map<string, EventListenerOrEventListenerObject>(),
    root = {
      style: {} as Record<string, string>,
      textContent: '',
      hidden: true,
      className: '',
      setAttribute(_name: string, _value: string) {},
      getBoundingClientRect: () => ({ left: 0, top: 0, width: 50, height: 20 }),
      remove() {
        removed++;
      },
    };
  const document = {
    body: { append(_element: unknown) {} },
    createElement: () => root,
    defaultView: {
      addEventListener(type: string, listener: EventListenerOrEventListenerObject) {
        listeners.set(type, listener);
      },
      removeEventListener(type: string) {
        listeners.delete(type);
      },
      setTimeout(callback: () => void, delay: number) {
        timers.set(++nextTimer, { callback, delay });
        return nextTimer;
      },
      clearTimeout(id: number) {
        timers.delete(id);
      },
    },
  };
  const viewport = { left: 10, top: 20, width: 200, height: 100 };
  const view = {
    get state() {
      if (retired) throw new Error('retired-preview-state-access');
      return state;
    },
    dom: { ownerDocument: document },
    contentDOM: {},
    scrollDOM: {
      getBoundingClientRect: () => viewport,
    },
    composing: false,
    compositionStarted: false,
    scaleY: 1,
    plugin() {
      if (retired) throw new Error('retired-preview-plugin-access');
      return owner;
    },
    requestMeasure(request: Measure) {
      jobs.push(request);
    },
  } as unknown as EditorView;
  const original = globalThis.getComputedStyle;
  globalThis.getComputedStyle = () =>
    ({
      fontFamily: 'Segoe UI',
      fontSize: '14px',
      color: 'rgb(0, 0, 0)',
      colorScheme: 'light',
    }) as CSSStyleDeclaration;
  const owner = new WeightPreview(view);
  return {
    owner,
    view,
    root,
    viewport,
    jobs,
    timers,
    listeners,
    get state() {
      return state;
    },
    get removed() {
      return removed;
    },
    /** Deliver one genuine read and write; tests may retain either phase to prove supersession. */
    measure() {
      const job = jobs.shift()!;
      job.write?.(job.read(view), view);
    },
    /** Actual current native post-state replaces only the test transport's state observation. */
    publish(next: EditorState) {
      const before = state;
      state = next;
      owner.update({
        startState: before,
        state,
        selectionSet: !before.selection.eq(state.selection),
        geometryChanged: false,
        viewportChanged: false,
      } as ViewUpdate);
    },
    /** Teardown restores the external global and releases the actual preview owner. */
    destroy() {
      owner.destroy();
      globalThis.getComputedStyle = original;
    },
    session: () => state.field(weightSession)!,
    /** A retired native boundary rejects observations rather than relying on post-destroy implementation behavior. */
    retire() {
      owner.destroy();
      retired = true;
    },
    /** Reject native state/plugin observations while retaining teardown transport for clear-only retirement proof. */
    rejectObservations() {
      retired = true;
    },
  };
}
