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
/** Qualify actual painted output pixels against the current lightweight display plan. */
import type { WorkflowOutputItem } from '../../execution/domain/WorkflowOutput.js';
import type { CanvasDisplayPlan } from '../application/CanvasDisplayPlan.js';
import type { BrowserCanvasImages } from '../presentation/browser/BrowserCanvasImages.js';
import type { BrowserCanvasOverview } from '../presentation/browser/BrowserCanvasOverview.js';
import type { BrowserCanvasDragSubject } from '../presentation/browser/BrowserCanvasDrag.js';

/** Resolve display identity without graph discovery, image admission, or transfer policy. */
export class BrowserCanvasDragSubjects {
  /** Bind presentation owners and an explicit current-plan reader for one viewport. */
  constructor(
    private readonly viewport: HTMLElement,
    private readonly images: BrowserCanvasImages,
    private readonly overview: BrowserCanvasOverview,
    private readonly plan: () => CanvasDisplayPlan,
  ) {}

  /** Find exact batch files already represented in the current display plan. */
  item(key: string): WorkflowOutputItem | null {
    const plan = this.plan();
    const items =
      plan.view.mode === 'overview'
        ? plan.grid.flatMap((entry) => entry.items)
        : [plan.primary?.item, plan.secondary?.item];
    return items.find((item) => item?.id === key && item.mediaKind === 'image') ?? null;
  }

  /** Refuse latent previews, outgoing decoded replacements, non-image media, and enlarged cameras. */
  resolve(target: EventTarget | null, event: MouseEvent | null): BrowserCanvasDragSubject | null {
    if (!event) return null;
    let element: HTMLImageElement | null = null;
    let key: string | null = null;
    if (this.plan().view.mode === 'overview') {
      const frame = this.overview.draggable(target);
      key = frame?.key ?? null;
      element = frame?.handle ?? null;
    } else {
      const bounds = this.viewport.getBoundingClientRect();
      const frame = this.images.draggable(target, {
        x: event.clientX - bounds.left,
        y: event.clientY - bounds.top,
      });
      key = frame?.key ?? null;
      element = frame?.handle ?? null;
    }
    return key && element && this.item(key) ? { key, element } : null;
  }
}
