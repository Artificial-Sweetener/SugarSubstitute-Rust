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
/** Regional row ink borrows CodeMirror's supported rectangle marker lifetime and the source-prepared palette. */
import { RectangleMarker } from '@codemirror/view';

/** One opaque regional rail has no input, geometry cache or independently managed DOM lifetime. */
export class RegionRowMarker extends RectangleMarker {
  private readonly color: string;
  /** All coordinates are viewport-plane CSS pixels already prepared by the existing SourceLines READ owner. */
  constructor(left: number, top: number, width: number, height: number, color: string) {
    super('sp-region-row-paint', left, top, width, height);
    this.color = color;
  }
  /** CodeMirror owns allocation and terminal removal of the returned paint element. */
  override draw(): HTMLDivElement {
    const node = super.draw();
    node.style.backgroundColor = this.color;
    return node;
  }
  /** Geometry and palette jointly identify reusable native output. */
  override eq(previous: RectangleMarker): boolean {
    return (
      previous instanceof RegionRowMarker && previous.color === this.color && super.eq(previous)
    );
  }
  /** The native layer delegates every geometry/color update through the same original marker owner. */
  override update(node: HTMLElement, previous: RectangleMarker): boolean {
    if (!(previous instanceof RegionRowMarker) || !super.update(node, previous)) return false;
    node.style.backgroundColor = this.color;
    return true;
  }
}
