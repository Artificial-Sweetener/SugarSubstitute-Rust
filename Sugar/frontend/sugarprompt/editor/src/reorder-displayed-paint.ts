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
/** Immutable Scene READ operations borrow exact presentation admission; they own no source, scheduler or retained cache. */
import { requireValue } from '../../core/src/required.js';
import type { EditorView } from '@codemirror/view';
import { captureReorderMotionGrab, type ReorderMotionScene } from './reorder-motion-scene.js';
import type { ReorderGrabGeometry } from './reorder-grab-geometry.js';
import type { ReorderGrabPaint } from './reorder-grab-contracts.js';
import type { ReorderMotionFrame } from './reorder-motion-clock.js';
import type { ReorderLandingObservation } from './reorder-landing-paint-lease.js';
import type { ReorderPreview, ReorderPointerPaint } from './reorder-preview-contracts.js';
import type { ReorderMeasureInput } from './reorder-measurement-contracts.js';
import type { ReorderRect } from './reorder-geometry.js';
import type { ReorderMotionOrigin } from './reorder-motion-origin.js';
import { reorderPreview } from './reorder-preview.js';
import { readReorderMotionOrigin } from './reorder-motion-origin.js';
import { reorderGrabGeometry } from './reorder-grab-geometry.js';
/** The presentation owns this accepted stamp; observations borrow its exact identity and currentness. */
export interface ReorderAcceptedDisplayPaint {
  readonly preview: ReorderPreview;
  readonly input: ReorderMeasureInput;
  readonly current: () => boolean;
  readonly rects: readonly ReorderRect[] | null;
}
/** Read-only live providers retain all mutable publication and clock authority in Presentation. */
export interface ReorderDisplayedPaintPorts {
  readonly live: () => boolean;
  readonly scene: () => ReorderMotionScene | null;
  readonly frame: () => ReorderMotionFrame | null;
  readonly input: () => ReorderMeasureInput | null;
  readonly accepted: () => ReorderAcceptedDisplayPaint | null;
  readonly current: (input: ReorderMeasureInput) => boolean;
  readonly resourceCurrent: () => boolean;
}
/** An accepted whole Scene may retain display admission while a compatible later proposal is preparing. */
export interface ReorderRetainedDisplayProposal {
  readonly scene: ReorderMotionScene;
  readonly pointer: ReorderPointerPaint | null;
  readonly preview: ReorderPreview;
  readonly held: number;
  readonly primary: number | null;
  readonly retained: () => boolean;
}
/** Borrow one exact read-only Scene/input/frame issue; delegated checks cannot authorize a replacement. */
function displayedLease(ports: ReorderDisplayedPaintPorts) {
  const scene = ports.scene(),
    frame = ports.frame(),
    input = ports.input();
  if (!scene || !input) return null;
  const held = input.held,
    pointer = input.pointer,
    order = input.order;
  const owns = (): boolean =>
    ports.live() &&
    ports.scene() === scene &&
    ports.frame() === frame &&
    ports.input() === input &&
    input.held === held &&
    input.pointer === pointer &&
    input.order === order;
  const current = (): boolean =>
    owns() && ports.current(input) && owns() && ports.resourceCurrent() && owns();
  return current() ? { scene, frame, input, held, current } : null;
}
/** READ composes canonical fallback with the exact issued display frame; reentry rejects borrowed geometry. */
export function readDisplayedReorderOrigin(
  view: EditorView,
  ports: ReorderDisplayedPaintPorts,
): ReorderMotionOrigin {
  const display = displayedLease(ports);
  const geometry = readReorderMotionOrigin(
    view,
    display?.scene.whole
      ? { current: display.current, rects: display.scene.geometry }
      : ports.accepted(),
    display?.scene ?? null,
    display?.frame ?? null,
  );
  return display && !display.current() ? [] : geometry;
}
/** A marker borrows the exact accepted whole endpoint, never a preparing or replaced publication. */
export function readCurrentDisplayedReorderPaint(
  view: EditorView,
  ports: ReorderDisplayedPaintPorts,
  input: ReorderMeasureInput,
): ReorderLandingObservation | null {
  const display = displayedLease(ports),
    accepted = ports.accepted();
  if (!display?.scene.whole || display.input !== input || accepted?.input !== input) return null;
  const preview = accepted.preview;
  const current = (): boolean =>
    ports.accepted() === accepted &&
    view.state.field(reorderPreview, false)?.value === preview &&
    display.current() &&
    ports.accepted() === accepted &&
    view.state.field(reorderPreview, false)?.value === preview;
  return readDisplayedReorderPaint(display.scene, preview, display.held, current);
}
/** A genuine press samples painted identity under the same immutable borrowed frame lease. */
export function hitCurrentDisplayedReorderPaint(
  view: EditorView,
  ports: ReorderDisplayedPaintPorts,
  x: number,
  y: number,
): number | null {
  const display = displayedLease(ports);
  if (!display?.frame) return null;
  const hit = hitDisplayedReorderPaint(view, display.scene, display.frame, display.held, x, y);
  return display.current() ? hit : null;
}
/** Borrow the actual hotspot and its exact Scene; only Presentation may pause the clock and retain the grab. */
export function borrowDisplayedReorderGrab(
  view: EditorView,
  ports: ReorderDisplayedPaintPorts,
  held: number,
  press: {
    readonly x: number;
    readonly y: number;
  },
) {
  const display = displayedLease(ports);
  if (!display) return null;
  const geometry = grabDisplayedReorderPaint(view, display.scene, display.frame, held, press);
  return geometry && display.current()
    ? { scene: display.scene, geometry, current: display.current }
    : null;
}
/** Exact accepted whole pixels can authorize the existing clock without promoting a later proposal's metadata. */
export function retainedDisplayedReorderScene(
  view: EditorView,
  ports: ReorderDisplayedPaintPorts,
  { scene, pointer, preview, held, primary, retained }: ReorderRetainedDisplayProposal,
): boolean {
  const latest = ports.input();
  if (!scene.whole || !pointer || !latest || primary === null) return false;
  const owns = (): boolean => {
    const state = view.state.field(reorderPreview, false);
    return (
      ports.live() &&
      ports.input() === latest &&
      ports.scene() === scene &&
      latest.capture === preview.capture &&
      latest.held === held &&
      latest.pointer === primary &&
      state?.pointer?.hold === pointer.hold &&
      state.pointer.anchor === pointer.anchor &&
      state.paint === pointer.anchor
    );
  };
  return owns() && ports.current(latest) && retained() && ports.resourceCurrent() && owns();
}
/** Complete native endpoint geometry belongs to the issued scene, including static and partly clipped groups. */
function readDisplayedReorderPaint(
  scene: ReorderMotionScene,
  preview: ReorderPreview,
  held: number,
  current: () => boolean,
): ReorderLandingObservation | null {
  if (!scene.whole || !scene.endpoint || !current()) return null;
  const { viewport, color } = scene.endpoint;
  return current()
    ? {
        preview,
        held,
        color,
        current,
        rects: scene.geometry.filter((rect) => rect.index === held),
        geometry: requireValue(scene.layout, 'reorder-displayed-paint'),
        viewport: {
          left: viewport.left,
          right: viewport.right,
          top: viewport.top,
          bottom: viewport.bottom,
        },
      }
    : null;
}
/** Hit and grab sample actual displayed translations without re-reading native layout. */
function displayedGeometry(
  view: EditorView,
  scene: ReorderMotionScene,
  frame: ReorderMotionFrame | null,
) {
  return readReorderMotionOrigin(
    view,
    { current: () => true, rects: scene.geometry },
    scene,
    frame,
  );
}
/** Resolve a painted chip only; the caller retains original Session and resource authority. */
function hitDisplayedReorderPaint(
  view: EditorView,
  scene: ReorderMotionScene,
  frame: ReorderMotionFrame | null,
  held: number,
  x: number,
  y: number,
): number | null {
  const geometry = displayedGeometry(view, scene, frame);
  const ids = scene.whole ? null : new Set(scene.chips.map((chip) => chip.id));
  return (
    geometry.find(
      (rect) =>
        rect.kind === 'chip' &&
        (!scene.whole || rect.index !== held) &&
        (!ids || ids.has(rect.index)) &&
        x >= rect.left &&
        x <= rect.right &&
        y >= rect.top &&
        y <= rect.bottom,
    )?.index ?? null
  );
}
/** Preserve the real press hotspot from complete displayed fragments, including unanimated whole-value peers. */
function grabDisplayedReorderPaint(
  view: EditorView,
  scene: ReorderMotionScene,
  frame: ReorderMotionFrame | null,
  held: number,
  press: {
    readonly x: number;
    readonly y: number;
  },
) {
  if (!scene.whole && !scene.chips.some((chip) => chip.id === held)) return null;
  return reorderGrabGeometry(
    displayedGeometry(view, scene, frame).filter(
      (rect) => rect.kind === 'chip' && rect.index === held,
    ),
    held,
    press,
  );
}
/** Presentation alone stores and consumes this exact paused native display descriptor. */
export interface ReorderPausedDisplayGrab {
  readonly held: number;
  readonly geometry: ReorderGrabGeometry;
  readonly scene: ReorderMotionScene;
}
/** A paused whole native snapshot may explicitly retain native materialization when copying is unavailable. */
export type ReorderCapturedDisplayGrab =
  | {
      readonly kind: 'paint';
      readonly paint: ReorderGrabPaint;
    }
  | {
      readonly kind: 'native';
    };
/** Adapt the consumed exact Scene through the native snapshot owner; this function retains no grab state or clock. */
export function captureDisplayedReorderGrab(
  view: EditorView,
  ports: ReorderDisplayedPaintPorts,
  held: number,
  grab: ReorderPausedDisplayGrab | null,
): ReorderCapturedDisplayGrab | null {
  if (!grab || grab.held !== held || ports.scene() !== grab.scene) return null;
  const paint = captureReorderMotionGrab(view, grab.scene, grab.geometry);
  return paint ? { kind: 'paint', paint } : grab.scene.whole ? { kind: 'native' } : null;
}
