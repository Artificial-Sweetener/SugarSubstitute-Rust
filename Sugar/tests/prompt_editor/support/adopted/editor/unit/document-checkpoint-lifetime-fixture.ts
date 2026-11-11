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
/** Controlled native measurement, input and timer transports prove checkpoint ownership without browser claims. */
import assert from 'node:assert/strict';
import { EditorState, Transaction } from '@codemirror/state';
import type { EditorView } from '@codemirror/view';
import { promptHistory } from '../../../../../../frontend/sugarprompt/editor/src/prompt-history.ts';
import { exactSourceExtension } from '../../../../../../frontend/sugarprompt/editor/src/exact-source.ts';
import { authorshipExtension } from '../../../../../../frontend/sugarprompt/editor/src/authorship.ts';
import { nativeInputSettlement } from '../../../../../../frontend/sugarprompt/editor/src/input-settlement.ts';
import { NativeDocumentCheckpoints } from '../../../../../../frontend/sugarprompt/editor/src/native-document-checkpoints.ts';
import type { FocusClock } from '../../../../../../frontend/sugarprompt/editor/src/native-focus.ts';

/** Use public CodeMirror measurement signatures at the inert transport seam. */
type Measure = NonNullable<Parameters<EditorView['requestMeasure']>[0]>;
/** Named injected collaborator boundaries retain their actual failure and reentry ordering. */
type Stage =
  | 'reserve'
  | 'allocate'
  | 'subscribe'
  | 'measure'
  | 'finish'
  | 'dispatch'
  | 'liveness'
  | 'composition'
  | 'view';

/** Each external collaborator may fail or reenter without implementing editing or history itself. */
export function fixture() {
  let alive = true,
    id = 0,
    finished = 0,
    cancelled = 0,
    removed = 0,
    cleared = 0,
    dispatched = 0;
  const hooks = new Map<Stage, () => void>();
  const cleanup = new Map<string, () => void>();
  const timers = new Map<unknown, () => void>();
  const measurements: Measure[] = [];
  const listeners = new Set<EventListenerOrEventListenerObject>();
  const controller = new AbortController();
  const signal = controller.signal;
  const subscribe = signal.addEventListener.bind(signal);
  const unsubscribe = signal.removeEventListener.bind(signal);
  // Genuine platform cancellation remains authoritative; only subscription allocation and release are controlled.
  Object.assign(signal, {
    addEventListener(...args: Parameters<AbortSignal['addEventListener']>) {
      const listener = args[1];
      if (listener) listeners.add(listener);
      subscribe(...args);
      hooks.get('subscribe')?.();
    },
    removeEventListener(...args: Parameters<AbortSignal['removeEventListener']>) {
      removed++;
      cleanup.get('listener')?.();
      unsubscribe(...args);
      if (args[1]) listeners.delete(args[1]);
    },
  });
  const input = {
    hasCompositionInput() {
      hooks.get('composition')?.();
      return transport.compositionStarted;
    },
    cancelCompositionInput() {},
    hasHeldKey: () => false,
    reserveCommand() {
      hooks.get('reserve')?.();
      return {
        finish() {
          finished++;
          hooks.get('finish')?.();
        },
        cancel() {
          cancelled++;
          cleanup.get('input')?.();
        },
      };
    },
  };
  let state = EditorState.create({
    doc: 'current',
    extensions: [
      exactSourceExtension('current'),
      promptHistory,
      authorshipExtension,
      nativeInputSettlement.of(input),
    ],
  });
  const transport = {
    get state() {
      return state;
    },
    compositionStarted: false,
    dispatch(transaction: Transaction) {
      dispatched++;
      hooks.get('dispatch')?.();
      state = transaction.state;
    },
    requestMeasure(request: Measure) {
      measurements.push(request);
      hooks.get('measure')?.();
    },
  };
  const clock: FocusClock = {
    set(callback, milliseconds) {
      assert.equal(milliseconds, 1000);
      const handle = ++id;
      timers.set(handle, callback);
      hooks.get('allocate')?.();
      return handle;
    },
    clear(handle) {
      cleared++;
      cleanup.get('timer')?.();
      timers.delete(handle);
    },
  };
  // The transport owns no native DOM or history; actual immutable CodeMirror transactions remain authoritative.
  const view = transport as unknown as EditorView;
  const owner = new NativeDocumentCheckpoints(
    {
      view: () => {
        hooks.get('view')?.();
        return view;
      },
      alive: () => {
        hooks.get('liveness')?.();
        return alive;
      },
    },
    clock,
  );
  return {
    owner,
    hooks,
    cleanup,
    timers,
    listeners,
    signal,
    transport,
    measurements,
    counts: () => ({ finished, cancelled, removed, cleared, dispatched }),
    flush() {
      for (const request of measurements.splice(0)) request.write?.(request.read(view), view);
    },
    deadline() {
      for (const callback of [...timers.values()]) callback();
    },
    abort() {
      controller.abort();
    },
    dispose() {
      alive = false;
      owner.dispose();
    },
  };
}
