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
/** Genuine clipboard orchestration settles native blur without deferred CodeMirror updates. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { StateEffect } from '@codemirror/state';
import { undoDepth, redoDepth } from '@codemirror/commands';
import type { EditorView } from '@codemirror/view';
import {
  clipboardCommands,
  NativeClipboard,
} from '../../../../../../frontend/sugarprompt/editor/src/native-clipboard.ts';
import { projectionState } from '../transaction/projection-state.ts';

/** Literal ignored-abort I/O supplies only the external window; real CM state and clipboard orchestration remain authoritative. */
function fixture() {
  const state = projectionState('preserved source').update({
    effects: StateEffect.appendConfig.of(clipboardCommands(() => undefined)),
  }).state;
  let enter!: () => void,
    release!: () => void,
    reads = 0;
  const entered = new Promise<void>((resolve) => {
      enter = resolve;
    }),
    gate = new Promise<void>((resolve) => {
      release = resolve;
    });
  const clipboard = {
    async read() {
      reads++;
      enter();
      await gate;
      return [
        {
          types: ['text/plain'],
          getType: () => Promise.resolve(new Blob(['late clipboard'])),
        },
      ];
    },
  };
  let focused = false;
  const view = {
    state,
    dom: {
      ownerDocument: {
        defaultView: { navigator: { clipboard }, DOMException },
      },
    },
    composing: false,
    compositionStarted: false,
    get hasFocus() {
      return focused;
    },
    dispatch() {
      assert.fail('A retired paste cannot dispatch');
    },
  } as unknown as EditorView;
  const owner = new NativeClipboard(view);
  return {
    owner,
    view,
    state,
    entered,
    release,
    reads: () => reads,
    focus() {
      focused = true;
    },
    dispose() {
      release();
      owner.destroy();
    },
  };
}

for (const retirement of ['blur', 'destroy', 'refocus', 'supersede'] as const)
  test(`native ${retirement} settles the correct ignored-abort paste without a focus-change update`, async () => {
    const c = fixture();
    try {
      const pending = c.owner.execute('paste');
      await c.entered;
      c.owner.blur();
      if (retirement === 'blur') {
        assert.deepEqual(await pending, { status: 'cancelled' });
      } else if (retirement === 'destroy') {
        c.owner.destroy();
        assert.deepEqual(await pending, { status: 'disposed' });
      } else if (retirement === 'refocus') {
        c.focus();
        await Promise.resolve();
        c.owner.destroy();
        assert.deepEqual(await pending, { status: 'disposed' });
      } else {
        const latest = c.owner.execute('paste');
        assert.deepEqual(await pending, { status: 'cancelled' });
        assert.equal(c.reads(), 2);
        c.owner.destroy();
        assert.deepEqual(await latest, { status: 'disposed' });
      }
      assert.equal(c.view.state, c.state);
      assert.deepEqual([undoDepth(c.state), redoDepth(c.state)], [0, 0]);
      c.release();
      assert.equal(c.view.state, c.state);
    } finally {
      c.dispose();
    }
  });
