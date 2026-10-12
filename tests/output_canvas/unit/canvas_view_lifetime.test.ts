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
/** Prove independent views and transactional surface replacement. */

import { CanvasViewRegistry } from '../../../frontend/comfyui/ui/output_canvas/application/CanvasViewRegistry.js';
import {
  canvasSessionId,
  windowId,
} from '../../../frontend/comfyui/ui/output_canvas/domain/CanvasIdentity.js';
import {
  outputSourceId,
  workflowSessionId,
} from '../../../frontend/comfyui/ui/execution/domain/ExecutionIdentity.js';

/** Return deterministic unique keys without introducing ambient identity state. */
function registry(): CanvasViewRegistry {
  let next = 0;
  return new CanvasViewRegistry(() => String(++next));
}

describe('canvas view lifetime', () => {
  test('moves a view only after destination readiness and preserves selection', () => {
    const owner = registry();
    const state = owner.create(canvasSessionId(workflowSessionId('first')));
    const first = owner.prepareAttachment(state.viewId, windowId('primary'));
    expect(owner.accepts(first)).toBe(false);
    expect(owner.commitAttachment(first)).toBeNull();
    const selected = owner.update(state.viewId, (current) => ({
      ...current,
      selection: { sourceId: outputSourceId('7:23'), batchIndex: 2 },
      follow: 'manual',
      comparison: {
        secondary: { sourceId: outputSourceId('42'), batchIndex: 2 },
        orientation: 'horizontal',
        split: 0.7,
      },
    }));
    const destination = owner.prepareAttachment(state.viewId, windowId('detached'));
    expect(owner.accepts(first)).toBe(true);
    expect(owner.accepts(destination)).toBe(false);
    expect(owner.read(state.viewId)).toBe(selected);

    expect(owner.commitAttachment(destination)).toBe(first);
    expect(owner.accepts(first)).toBe(false);
    expect(owner.accepts(destination)).toBe(true);
    expect(owner.releaseAttachment(first)).toBe(false);
    expect(owner.read(state.viewId)).toBe(selected);
    expect(() => owner.commitAttachment(first)).toThrow('stale');
  });

  test('keeps the old surface usable when a destination fails and rejects delayed readiness', () => {
    const owner = registry();
    const state = owner.create(canvasSessionId(workflowSessionId('first')));
    const first = owner.prepareAttachment(state.viewId, windowId('primary'));
    owner.commitAttachment(first);
    const failed = owner.prepareAttachment(state.viewId, windowId('detached'));
    expect(owner.cancelAttachment(failed)).toBe(true);
    expect(owner.accepts(first)).toBe(true);
    expect(() => owner.commitAttachment(failed)).toThrow('stale');
    const replacement = owner.prepareAttachment(state.viewId, windowId('retry'));
    expect(replacement.generation).toBeGreaterThan(failed.generation);
    owner.commitAttachment(replacement);
    expect(owner.cancelAttachment(failed)).toBe(false);
    expect(owner.accepts(replacement)).toBe(true);
  });

  test('creates independent views and closes only the explicitly owned view', () => {
    const owner = registry();
    const session = canvasSessionId(workflowSessionId('first'));
    const a = owner.create(session);
    const b = owner.create(session);
    const other = owner.create(canvasSessionId(workflowSessionId('other')));
    let changed = 0;
    const stop = owner.subscribe(b.viewId, () => ++changed);
    owner.update(a.viewId, (current) => ({ ...current, follow: 'manual', linked: false }));
    expect(owner.read(b.viewId)).toBe(b);
    expect(owner.read(other.viewId)).toBe(other);
    expect(changed).toBe(0);
    owner.remove(a.viewId);
    expect(() => owner.read(a.viewId)).toThrow('unavailable');
    owner.update(b.viewId, (current) => ({ ...current, mode: 'overview' }));
    expect(changed).toBe(1);
    stop();
    owner.update(b.viewId, (current) => ({ ...current, mode: 'image' }));
    expect(changed).toBe(1);
  });

  test('rejects identity changes and concurrent attachment transactions', () => {
    const owner = registry();
    const a = owner.create(canvasSessionId(workflowSessionId('first')));
    const b = owner.create(canvasSessionId(workflowSessionId('other')));
    expect(() => owner.update(a.viewId, () => b)).toThrow('identity');
    const address = owner.prepareAttachment(a.viewId, windowId('primary'));
    expect(() => owner.prepareAttachment(a.viewId, windowId('other'))).toThrow('pending');
    owner.commitAttachment(address);
    expect(owner.accepts({ ...address, sessionId: b.sessionId })).toBe(false);
    expect(owner.accepts({ ...address, windowId: windowId('forged') })).toBe(false);
    expect(owner.releaseAttachment(address)).toBe(true);
    expect(owner.read(a.viewId)).toBe(a);
    expect(owner.accepts(address)).toBe(false);
  });

  test('publishes nested immutable state without retaining mutable caller selections', () => {
    const owner = registry();
    const state = owner.create(canvasSessionId(workflowSessionId('first')));
    const selection = { sourceId: outputSourceId('7'), batchIndex: 1 };
    const secondary = { sourceId: outputSourceId('42'), batchIndex: 0 };
    const published = owner.update(state.viewId, (current) => ({
      ...current,
      selection,
      comparison: { secondary, orientation: 'vertical', split: 0.5 },
    }));
    selection.batchIndex = 3;
    secondary.batchIndex = 2;
    expect(published.selection?.batchIndex).toBe(1);
    expect(published.comparison?.secondary.batchIndex).toBe(0);
    expect(Object.isFrozen(published.selection)).toBe(true);
    expect(Object.isFrozen(published.comparison)).toBe(true);
    expect(Object.isFrozen(published.comparison?.secondary)).toBe(true);
  });
});
