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
/**
 * Own the LiteGraph group draw-hook lifecycle for Cube title projection.
 */

import { getGroupSugarcubes } from '../graph/GroupMetadata.js';
import { CubeChromePainter } from './CubeChromePainter.js';
import type { ComfyGroup } from '../types/graph.js';
import type { ChromeAdapter, ChromeCanvas } from './CubeChromeContracts.js';

type GroupDraw = (this: ComfyGroup, ...args: unknown[]) => unknown;
interface GroupDrawPrototype extends ComfyGroup {
  draw?: GroupDraw;
  __sugarcubes_title_icon_renderer?: CubeChromeGroupTitleHook;
  __sugarcubes_title_icon_patched?: boolean;
  __sugarcubes_title_icon_original_draw?: GroupDraw;
}

/** Install and release the managed-group title drawing hook. */
export class CubeChromeGroupTitleHook {
  constructor(
    private readonly adapter: ChromeAdapter | null,
    private readonly painter: CubeChromePainter,
  ) {}

  installGroupTitleIconRenderer(): void {
    const liteGraph =
      this.adapter?.getLiteGraph?.() ||
      (typeof globalThis !== 'undefined' ? globalThis.LiteGraph : null);
    const GroupRef = liteGraph?.LGraphGroup;
    if (!GroupRef?.prototype) {
      return;
    }
    const prototype = GroupRef.prototype as unknown as GroupDrawPrototype;
    if (typeof prototype.draw !== 'function') {
      return;
    }
    prototype.__sugarcubes_title_icon_renderer = this;
    if (prototype.__sugarcubes_title_icon_patched) {
      return;
    }
    const originalDraw = prototype.draw;
    prototype.__sugarcubes_title_icon_original_draw = originalDraw;
    prototype.__sugarcubes_title_icon_patched = true;
    prototype.draw = function drawSugarCubesGroupTitleIcon(this: ComfyGroup, ...args: unknown[]) {
      const renderer = prototype.__sugarcubes_title_icon_renderer;
      if (renderer?.shouldDrawGroupTitleIcon?.(this)) {
        return renderer.drawManagedGroupWithTitleIcon(this, originalDraw, args);
      }
      return originalDraw.apply(this, args);
    };
  }

  releaseGroupTitleIconRenderer(): void {
    const liteGraph =
      this.adapter?.getLiteGraph?.() ||
      (typeof globalThis !== 'undefined' ? globalThis.LiteGraph : null);
    const GroupRef = liteGraph?.LGraphGroup;
    const prototype = GroupRef?.prototype as unknown as GroupDrawPrototype | undefined;
    if (prototype?.__sugarcubes_title_icon_renderer === this) {
      delete prototype.__sugarcubes_title_icon_renderer;
    }
  }

  shouldDrawGroupTitleIcon(group: ComfyGroup): boolean {
    const metadata = getGroupSugarcubes(group);
    return Boolean(metadata?.managed && metadata.instance_id);
  }

  drawManagedGroupWithTitleIcon(
    group: ComfyGroup,
    originalDraw: GroupDraw,
    args: unknown[],
  ): unknown {
    const titleDescriptor = Object.getOwnPropertyDescriptor(group, 'title');
    const hadTitleDescriptor = Boolean(titleDescriptor);
    try {
      Object.defineProperty(group, 'title', {
        value: '',
        writable: true,
        enumerable: titleDescriptor?.enumerable ?? true,
        configurable: true,
      });
      originalDraw.apply(group, args);
    } finally {
      if (hadTitleDescriptor && titleDescriptor) {
        Object.defineProperty(group, 'title', titleDescriptor);
      } else {
        delete group.title;
      }
    }

    const ctx = args[1];
    const canvasInstance = args[0];
    if (
      !ctx ||
      typeof ctx !== 'object' ||
      typeof (ctx as CanvasRenderingContext2D).save !== 'function' ||
      !canvasInstance ||
      typeof canvasInstance !== 'object'
    ) {
      return undefined;
    }
    this.painter.drawGroupTitleIcon(
      ctx as CanvasRenderingContext2D,
      group,
      canvasInstance as ChromeCanvas,
    );
    return undefined;
  }
}
