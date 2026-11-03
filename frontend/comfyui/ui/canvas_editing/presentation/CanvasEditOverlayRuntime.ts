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
/** Mount the editing chrome over any canvas surface whose host advertises editing. */
import { createApp, h, shallowRef } from 'vue';
import type { CanvasEditHost } from '../../../../../integrations/contracts/CanvasEditing.js';
import {
  CanvasEditController,
  type CanvasEditSnapshot,
} from '../application/CanvasEditController.js';
import { CanvasEditContextMenu } from './CanvasEditContextMenu.js';
import { CanvasEditKeyboard } from './CanvasEditKeyboard.js';
import { CanvasEditOverlay } from './CanvasEditOverlay.js';
import type { CanvasEditViewport } from './CanvasContextualToolbar.js';
import { ensureCanvasEditStyles } from './CanvasEditStyles.js';

/** The measured canvas allocation: client CSS-pixel origin, size and display density. */
export interface CanvasEditSurfaceGeometry {
  readonly origin: { readonly x: number; readonly y: number };
  readonly viewport: { readonly width: number; readonly height: number };
  readonly physicalPixelsPerCssPixel: number;
}

export interface CanvasEditOverlayOptions {
  /** Positioned layer, covering the canvas attachment, that holds the chrome. */
  readonly root: HTMLElement;
  /** The canvas attachment; keyboard shortcuts apply while focus rests inside it. */
  readonly scope: HTMLElement;
  /** The canvas region whose secondary-button presses open the canvas menu. */
  readonly region: HTMLElement;
  geometry(): CanvasEditSurfaceGeometry | null;
  /** Name the CSS colour of a mask layer; absent or null shows the theme accent. */
  maskColor?(layerId: string): string | null;
  report(error: unknown): void;
}

export interface MountedCanvasEditOverlay {
  /** Bind the canvas's current editing host; null or a changed host retires the previous one. */
  attach(host: CanvasEditHost | null): void;
  /** Remeasure after the canvas allocation changed. */
  geometryChanged(): void;
  /** Re-render after appearance such as mask colours changed. */
  refresh(): void;
  dispose(): void;
}

/**
 * Render from one controller per editing host.
 *
 * The chrome stays empty until the host reports an editable document, so the same mount serves
 * every route of a canvas. Every interactive element is native-occlusion chrome.
 */
export function mountCanvasEditOverlay(
  options: CanvasEditOverlayOptions,
): MountedCanvasEditOverlay {
  const { root } = options;
  const document = root.ownerDocument;
  ensureCanvasEditStyles(document);
  root.classList.add('sc-canvas-edit-root');
  const controller = shallowRef<CanvasEditController | null>(null);
  const snapshot = shallowRef<CanvasEditSnapshot | null>(null);
  const viewport = shallowRef<CanvasEditViewport | null>(null);
  const appearance = shallowRef(0);
  let stopSnapshot: (() => void) | null = null;
  let disposed = false;
  const menu = new CanvasEditContextMenu(options.region, () => controller.value);
  const keyboard = new CanvasEditKeyboard(document, options.scope, root, () => controller.value);
  /** Keep the attachment's output menu off the editing chrome. */
  const contain = (event: MouseEvent): void => {
    event.preventDefault();
    event.stopPropagation();
  };
  root.addEventListener('contextmenu', contain);
  const app = createApp({
    render: () => {
      void appearance.value;
      const edit = controller.value;
      const current = snapshot.value;
      if (!edit) return null;
      const mask = current?.state.activeMask;
      return h(CanvasEditOverlay, {
        snapshot: current,
        controller: edit,
        viewport: viewport.value,
        maskColor: mask ? (options.maskColor?.(mask) ?? null) : null,
      });
    },
  });
  app.config.errorHandler = (error) => options.report(error);
  app.mount(root);

  function measure(): void {
    const geometry = options.geometry();
    if (!geometry) {
      viewport.value = null;
      return;
    }
    const bounds = root.getBoundingClientRect();
    const next = {
      left: geometry.origin.x - bounds.left,
      top: geometry.origin.y - bounds.top,
      width: geometry.viewport.width,
      height: geometry.viewport.height,
      density: geometry.physicalPixelsPerCssPixel,
    };
    const previous = viewport.value;
    if (!previous || JSON.stringify(previous) !== JSON.stringify(next)) viewport.value = next;
  }

  function retire(): void {
    stopSnapshot?.();
    stopSnapshot = null;
    controller.value?.dispose();
    controller.value = null;
    snapshot.value = null;
    menu.close();
  }

  return {
    attach(host) {
      if (disposed || controller.value?.host === host) return;
      retire();
      if (!host) return;
      const next = new CanvasEditController({
        host,
        report: (error) => options.report(error),
      });
      stopSnapshot = next.subscribe(() => {
        snapshot.value = next.read();
        if (!snapshot.value) menu.close();
      });
      controller.value = next;
      measure();
      next.start();
    },
    geometryChanged() {
      if (!disposed) measure();
    },
    refresh() {
      if (!disposed) appearance.value++;
    },
    dispose() {
      if (disposed) return;
      disposed = true;
      retire();
      keyboard.dispose();
      menu.dispose();
      root.removeEventListener('contextmenu', contain);
      app.unmount();
      root.classList.remove('sc-canvas-edit-root');
    },
  };
}
