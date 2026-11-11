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
/** Complete native event routing composes the sole reorder session and its prepared preview. */
import { Prec, type Extension } from '@codemirror/state';
import { ViewPlugin } from '@codemirror/view';
import { reorderMotionAttachmentExtension } from './reorder-motion-layer.js';
import { NativeReorderSession } from './reorder-session.js';
import { nativeReorderProjectionExtension } from './reorder-native-projection.js';
import { reorderPreviewExtension } from './reorder-preview.js';
import { reorderGrabCursorExtension } from './reorder-grab-cursor.js';

/** High precedence affects only active reorder; dormant ordinary native owners remain installed. */
export const reorderInteractions: Extension = [
  reorderPreviewExtension,
  reorderMotionAttachmentExtension,
  nativeReorderProjectionExtension,
  reorderGrabCursorExtension,
  Prec.highest(
    ViewPlugin.fromClass(NativeReorderSession, {
      eventHandlers: {
        /** Native wheel retains its default viewport action; only competing edge-scroll work pauses. */ wheel() {
          return this.wheel();
        },
        /** Native viewport and ancestor scroll retain ordinary scrolling while refreshing current held feedback. */ scroll() {
          this.refreshGeometry();
          return false;
        },
        /** Route entry/active key intent. */ keydown(event) {
          return this.keydown(event);
        },
        /** Commit finite Alt lifetime. */ keyup(event) {
          return this.keyup(event);
        },
        /** Capture primary native drag intent. */ pointerdown(event) {
          return this.pointerdown(event);
        },
        /** Coalesce current coordinates. */ pointermove(event) {
          return this.pointermove(event);
        },
        /** Settle final native drop. */ pointerup(event) {
          return this.pointerup(event);
        },
        /** Lost native pointer authority cancels. */ pointercancel() {
          this.close();
          return false;
        },
        /** Release never leaves a stale proxy. */ lostpointercapture(event) {
          return this.pointerLost(event);
        },
        /** Composition takes back ordinary native input. */ compositionstart() {
          this.close();
          return false;
        },
        /** Focus loss retires source authority. */ blur() {
          this.close();
          return false;
        },
        /** Block source input during finite preview. */ beforeinput() {
          return this.blocksInput();
        },
        /** Block paste during finite preview. */ paste() {
          return this.blocksInput();
        },
        /** Reorder uses native pointer capture, never the browser text/image MIME drag pipeline. */ dragstart() {
          return this.blocksInput();
        },
        /** Block external source drops during finite preview. */ drop() {
          return this.blocksInput();
        },
      },
    }),
  ),
];
