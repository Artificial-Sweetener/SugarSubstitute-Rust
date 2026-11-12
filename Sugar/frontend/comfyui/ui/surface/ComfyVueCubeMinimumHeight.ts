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
/** Translate face content height through Comfy's Nodes 2.0 shell geometry. */

export interface ComfyVueCubeMinimumHeightOptions {
  faceMinimumHeight: number;
  headerFlowHeight: number;
  footerFlowHeight: number;
  nodeHeight: number;
  nodeRoot: HTMLElement;
}

/** Include native header and body overflow without leaking those details into face layout. */
export function resolveComfyVueCubeMinimumHeight(
  options: ComfyVueCubeMinimumHeightOptions,
): number {
  const faceMinimumHeight = finiteNonNegative(options.faceMinimumHeight);
  const nodeHeight = finiteNonNegative(options.nodeHeight);
  const renderedRootHeight = finiteNonNegative(options.nodeRoot.offsetHeight);
  const graphIndependentShellHeight = Math.max(0, renderedRootHeight - nodeHeight);
  const requiredRootHeight =
    finiteNonNegative(options.headerFlowHeight) +
    faceMinimumHeight +
    finiteNonNegative(options.footerFlowHeight);
  return Math.ceil(Math.max(faceMinimumHeight, requiredRootHeight - graphIndependentShellHeight));
}

/** Keep browser geometry finite before it reaches graph-node sizing. */
function finiteNonNegative(value: number): number {
  return Number.isFinite(value) ? Math.max(0, value) : 0;
}
