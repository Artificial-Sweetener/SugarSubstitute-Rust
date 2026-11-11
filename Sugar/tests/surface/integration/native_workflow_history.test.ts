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
/** Prove native graph transactions capture once across current and legacy Comfy history hooks. */
import { expect, jest, test } from '@jest/globals';
import { ComfyCanvasHistoryAdapter } from '../../../frontend/comfyui/ui/surface/ComfyCanvasHistoryAdapter.js';

/** Model the host tracker depth and capture behavior without implementing an alternative history. */
function fixture(legacy: boolean) {
  let depth = 0;
  const capture = jest.fn();
  const tracker = {
    _restoringState: false,
    beforeChange: () => {
      depth++;
    },
    afterChange: () => {
      if (--depth === 0) capture();
    },
  };
  const before = legacy ? () => tracker.beforeChange() : undefined;
  const after = legacy ? () => tracker.afterChange() : undefined;
  const canvas = {
    onBeforeChange: before,
    onAfterChange: after,
    emitBeforeChange: jest.fn(),
    emitAfterChange: jest.fn(),
  };
  const workflow = { changeTracker: tracker };
  const app = { canvas, extensionManager: { workflow: { activeWorkflow: workflow } } };
  const beforeCommit = jest.fn();
  const adapter = new ComfyCanvasHistoryAdapter(app, document, beforeCommit);
  adapter.install();
  return {
    tracker,
    canvas,
    workflow,
    app,
    adapter,
    capture,
    beforeCommit,
    depth: () => depth,
    before,
    after,
  };
}

test.each([false, true])('records one edit with existing native history callbacks=%s', (legacy) => {
  const f = fixture(legacy);
  f.canvas.onBeforeChange?.();
  f.canvas.onBeforeChange?.();
  f.canvas.onAfterChange?.();
  expect(f.capture).not.toHaveBeenCalled();
  f.canvas.onAfterChange?.();
  expect(f.depth()).toBe(0);
  expect(f.capture).toHaveBeenCalledTimes(1);
  f.adapter.dispose();
  expect(f.canvas.onBeforeChange).toBe(f.before);
  expect(f.canvas.onAfterChange).toBe(f.after);
});

test('captures canvas event transactions and nested graph callbacks only once', () => {
  const f = fixture(false);
  f.canvas.emitBeforeChange();
  f.canvas.onBeforeChange?.();
  f.canvas.onAfterChange?.();
  expect(f.capture).not.toHaveBeenCalled();
  f.canvas.emitAfterChange();
  expect(f.depth()).toBe(0);
  expect(f.capture).toHaveBeenCalledTimes(1);
  f.adapter.dispose();
});

test('finishes dependent mutations before the native tracker captures the entry', () => {
  const f = fixture(false);
  f.beforeCommit.mockImplementation(() => {
    expect(f.depth()).toBe(1);
    expect(f.capture).not.toHaveBeenCalled();
  });
  f.canvas.emitBeforeChange();
  f.canvas.emitAfterChange();
  expect(f.beforeCommit).toHaveBeenCalledTimes(1);
  expect(f.capture).toHaveBeenCalledTimes(1);
  f.adapter.dispose();
});

test('ignores standalone refreshes and passive restoration, then captures the next user transaction', () => {
  const f = fixture(false);
  f.canvas.onAfterChange?.();
  f.tracker._restoringState = true;
  f.canvas.onBeforeChange?.();
  f.canvas.onAfterChange?.();
  f.tracker._restoringState = false;
  f.adapter.setConfiguring(true);
  f.canvas.onBeforeChange?.();
  f.canvas.onAfterChange?.();
  expect(f.capture).not.toHaveBeenCalled();
  f.adapter.setConfiguring(false);
  f.canvas.onBeforeChange?.();
  f.canvas.onAfterChange?.();
  expect(f.capture).toHaveBeenCalledTimes(1);
  f.adapter.dispose();
});

test('balances the original tracker if the active workflow changes inside a transaction', () => {
  const f = fixture(false);
  f.canvas.onBeforeChange?.();
  const next = { beforeChange: jest.fn(), afterChange: jest.fn(), _restoringState: false };
  Object.assign(f.app.extensionManager.workflow, { activeWorkflow: { changeTracker: next } });
  f.canvas.onAfterChange?.();
  expect(f.capture).toHaveBeenCalledTimes(1);
  expect(next.afterChange).not.toHaveBeenCalled();
  f.adapter.dispose();
});

test('retains another extension callback chain and stops capturing after disposal', () => {
  const f = fixture(false);
  const begin = f.canvas.onBeforeChange;
  const finish = f.canvas.onAfterChange;
  const otherBegin = () => begin?.();
  const otherFinish = () => finish?.();
  f.canvas.onBeforeChange = otherBegin;
  f.canvas.onAfterChange = otherFinish;
  f.adapter.dispose();
  expect(f.canvas.onBeforeChange).toBe(otherBegin);
  f.canvas.onBeforeChange?.();
  f.canvas.onAfterChange?.();
  expect(f.capture).not.toHaveBeenCalled();
});

test.each(['before', 'after'])(
  'balances native history when the existing %s callback throws',
  (phase) => {
    const f = fixture(false);
    f.adapter.dispose();
    const failure = (): never => {
      throw new Error('Native callback failed');
    };
    if (phase === 'before') f.canvas.onBeforeChange = failure;
    else f.canvas.onAfterChange = failure;
    f.adapter.install();
    try {
      expect(() => {
        f.canvas.onBeforeChange?.();
        f.canvas.onAfterChange?.();
      }).toThrow('Native callback failed');
      expect(f.depth()).toBe(0);
    } finally {
      f.adapter.dispose();
    }
  },
);
