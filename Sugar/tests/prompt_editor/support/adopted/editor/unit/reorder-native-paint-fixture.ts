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
/** Actual native snapshot ownership is composed with literal DOM transport, without a scene or rendering oracle. */
import {
  admitNativeReorderPaint,
  copyNativeReorderPaint,
} from '../../../../../../frontend/sugarprompt/editor/src/reorder-native-paint.ts';
import { prepareNativeReorderPaint } from '../../../../../../frontend/sugarprompt/editor/src/reorder-paint-preparation.ts';
import { withReorderPaintMaterialization } from '../../../../../../frontend/sugarprompt/editor/src/reorder-paint-materialization.ts';
import type { motionPaintFixture } from './reorder-motion-paint-fixture.ts';
/** Capture one complete actual admitted source line; request transfer and rollback remain production-owned. */
export function captureNativePaint(f: ReturnType<typeof motionPaintFixture>): HTMLElement | null {
  const roots = [f.line as unknown as HTMLElement],
    admitted = admitNativeReorderPaint(roots);
  if (!admitted) return null;
  const prepared = prepareNativeReorderPaint(f.view, roots, admitted, 'copy');
  if (!prepared) return null;
  return withReorderPaintMaterialization(
    f.view.dom.ownerDocument,
    (materialization) =>
      copyNativeReorderPaint(f.view, roots[0]!, prepared, materialization) as HTMLElement | null,
    (node) => node?.remove(),
  );
}
