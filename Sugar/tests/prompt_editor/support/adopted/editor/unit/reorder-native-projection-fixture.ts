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
import { drawNativeReorderMetadata } from './reorder-native-metadata-fixture.ts';
/** Shared real Core/StateField publication transports one finite native DOM/observer lifetime. */
import assert from 'node:assert/strict';
import { StateEffect, type TransactionSpec } from '@codemirror/state';
import type { ViewUpdate } from '@codemirror/view';
import { moveReorderChip } from '../../../../../../frontend/sugarprompt/core/src/index.ts';
import { captureReorder, reorderState } from '../transaction/reorder-fixture.ts';
import { motionPaintFixture } from './reorder-motion-paint-fixture.ts';
import { prepareReorderPreview } from '../../../../../../frontend/sugarprompt/editor/src/reorder-preview-preparation.ts';
import { createReorderApplicationHold } from '../../../../../../frontend/sugarprompt/editor/src/reorder-session-application.ts';
import { NativeReorderProjection } from '../../../../../../frontend/sugarprompt/editor/src/reorder-native-projection.ts';
import {
  installReorderPreview,
  reorderPreview,
  reorderPreviewExtension,
  publishReorderPreview,
} from '../../../../../../frontend/sugarprompt/editor/src/reorder-preview.ts';

/** The owner receives actual state declarations and source positions through one controlled native DOM/observer boundary. */
export function nativeFixture(source = 'a, b, c') {
  const f = motionPaintFixture();
  let state = reorderState(source).state.update({
    effects: StateEffect.appendConfig.of(reorderPreviewExtension),
  }).state;
  let hostOpacity = '1';
  const nativeStyle = f.document.defaultView.getComputedStyle.bind(f.document.defaultView);
  /** The native CSS boundary resolves the issued whole-value gate without mutating managed line nodes. */
  f.document.defaultView.getComputedStyle = (node) => {
    const style = nativeStyle(node);
    if (node !== f.line) return style;
    const opacity = state.field(reorderPreview).mask?.coverage === 'value' ? '0' : hostOpacity;
    return {
      ...style,
      opacity,
      getPropertyValue: (name: string) =>
        name === 'opacity' ? opacity : style.getPropertyValue(name),
    };
  };
  const capture = captureReorder(state),
    pickup = prepareReorderPreview(
      { token: {}, capture, activeChip: 0, order: capture.prepared.initial },
      'pointer',
    );
  assert.ok(pickup);
  state = state.update({ effects: installReorderPreview.of(pickup) }).state;
  const before = state.field(reorderPreview),
    hold = createReorderApplicationHold(pickup.order),
    moved = moveReorderChip(capture.prepared, pickup.order, 0, {
      kind: 'row',
      row: 0,
      insertion: 2,
    });
  assert.ok(moved);
  let sourceReads = 0,
    disconnectFailure: Error | null = null,
    positionFailure: Error | null = null,
    positionHook: (() => void) | null = null;
  const observers: Observer[] = [];
  /** Controlled observer transports native invalidation and raw disposal failure, without inventing source or layout. */
  class Observer {
    readonly callback: () => void;
    records: MutationRecord[] = [];
    disconnected = false;
    /** Each actual production observer gets one independent finite transport instance. */
    constructor(callback: () => void) {
      this.callback = callback;
      observers.push(this);
    }
    /** Observation construction has no side effects in this explicit native boundary. */
    observe(): void {}
    /** Currentness consumes the exact pending mutation records once. */
    takeRecords(): MutationRecord[] {
      const records = this.records;
      this.records = [];
      return records;
    }
    /** Teardown exposes its original native failure for the owner contract. */
    disconnect(): void {
      this.disconnected = true;
      if (disconnectFailure) throw disconnectFailure;
    }
  }
  Object.defineProperty(f.document.defaultView, 'MutationObserver', {
    value: Observer,
  });
  const positions = new Map<HTMLElement, { from: number; to: number }>(
    f.chips.map((node, id) => [node as unknown as HTMLElement, { from: id * 3, to: id * 3 + 1 }]),
  );
  Object.defineProperties(f.view, {
    state: { get: () => state },
    hasFocus: { value: false },
    /** Supported CM positions are supplied independently of the admitted native roster. */
    posAtDOM: {
      value: (node: HTMLElement, offset: number) => {
        sourceReads++;
        const hook = positionHook;
        positionHook = null;
        hook?.();
        if (positionFailure) throw positionFailure;
        const position = positions.get(node);
        assert.ok(position);
        return offset === 0 ? position.from : position.to;
      },
    },
    /** Native transactions remain the only source/preview owner; plugin update is transported explicitly. */
    dispatch: {
      value: (spec: TransactionSpec) => {
        const transaction = state.update(spec);
        state = transaction.state;
        drawNativeReorderMetadata(f, state);
        const update = {
          docChanged: transaction.docChanged,
          viewportChanged: false,
          geometryChanged: transaction.docChanged,
        } as ViewUpdate;
        owner.update(update);
        f.attachment.update(update);
      },
    },
  });
  drawNativeReorderMetadata(f, state);
  const owner = new NativeReorderProjection(f.view);
  /** Actual publication prepares a changed proposal beneath the immutable genuine native pickup. */
  const publish = (order = moved, nextHold = hold) => {
    assert.equal(
      publishReorderPreview(f.view, { ...pickup, order }, 'pointer', nextHold, () => true),
      'published',
    );
    return state.field(reorderPreview);
  };
  return {
    f,
    owner,
    pickup,
    before,
    hold,
    moved,
    publish,
    positions,
    observers,
    /** Resolve a controlled host opacity before native issue admission. */
    setOpacity(value: string) {
      hostOpacity = value;
    },
    get state() {
      return state;
    },
    get sourceReads() {
      return sourceReads;
    },
    /** Reenter the native admission boundary once through supported CM position observation. */
    onPosition(hook: () => void) {
      positionHook = hook;
    },
    /** Fail only the explicit native positioning boundary. */
    failPosition(error: Error) {
      positionFailure = error;
    },
    /** Fail only the explicit native observer retirement boundary. */
    failDisconnect(error: Error) {
      disconnectFailure = error;
    },
  };
}
