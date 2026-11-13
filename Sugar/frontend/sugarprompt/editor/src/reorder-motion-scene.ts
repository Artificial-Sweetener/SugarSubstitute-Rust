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
/** Passive scene contracts and paused native snapshots retain finite readonly paint ownership. */
import { withReorderPaintMaterialization } from './reorder-paint-materialization.js';
import { prepareNativeReorderPaint } from './reorder-paint-preparation.js';
import type { ReorderGrabGeometry } from './reorder-grab-geometry.js';
import type { ReorderGrabPaint } from './reorder-grab-contracts.js';
import type { EditorView } from '@codemirror/view';
import type { ReorderRect, ReorderMeasuredGeometry } from './reorder-geometry.js';
import type { ReorderMeasureInput } from './reorder-measurement-contracts.js';
import type { ReorderMotionFrame, ReorderMotionTarget } from './reorder-motion-clock.js';
import {
  admitNativeReorderPaint,
  copyNativeReorderPaint,
  nativeReorderLineSize,
} from './reorder-native-paint.js';
/** One admitted semantic chip retains every native fragment and its externally owned readonly roots. */
interface SceneChip {
  readonly id: number;
  readonly rects: readonly ReorderRect[];
  readonly left: number;
  readonly top: number;
  readonly roots: readonly {
    readonly node: HTMLElement;
    readonly scaleX: number;
    readonly scaleY: number;
  }[];
}
/** A READ-only presentation decision prepares actual translated peer endpoints from complete viewport-CSS-pixel geometry, or refuses the entire animation. */
export type ReorderMotionSelection = (
  geometry: readonly ReorderRect[],
) => readonly ReorderMotionTarget[] | null;
/** Exact READ operands carry no DOM or contour construction across the native measurement boundary. */
export interface ReorderMotionEndpoint {
  readonly order: ReorderMeasureInput['order'];
  readonly held: number;
  readonly hold: object;
  readonly viewport: ReorderMeasuredGeometry['viewport'];
  readonly color: string;
}
/** Passive paint owns no source, gesture, document, selection or history authority. */
export interface ReorderMotionScene {
  /** Only whole Scenes carry their independently measured contour context. */
  readonly endpoint: ReorderMotionEndpoint | null;
  /** Complete native line coverage retains a settled pointer endpoint independently of clock activity. */
  readonly whole?: true;
  /** Whole endpoints retain the canonical mounted owner's actual carrier/partition/blank marker geometry. */
  readonly layout?: readonly ReorderRect[];
  /** Whole paint remains hidden until the exact whole-value suppression gate is admitted. */
  readonly reveal?: () => void;
  /** Whole Resource admission publishes its exact endpoint receipt under the same physical Slot write reservation. */
  readonly publishEndpoint?: (issue: string) => void;
  readonly node: HTMLElement;
  /** Complete eligible native peer fragments in viewport CSS pixels, including unanimated peers. */
  readonly geometry: readonly ReorderRect[];
  /** The exact immutable selection endpoints are consumed directly by the existing clock. */
  readonly targets: readonly ReorderMotionTarget[];
  /** Only actually translated peers are revealed, suppressed and animated; at most 64 groups. */
  readonly chips: readonly SceneChip[];
  /** Writes only to externally owned readonly paint, preserving native inline glyph shaping. */
  paint(frame: ReorderMotionFrame): void;
  destroy(): void;
}
/** One paused native viewport snapshot supplies complete press paint without another line/source layout. */
export function captureReorderMotionGrab(
  view: EditorView,
  scene: Pick<ReorderMotionScene, 'node'>,
  geometry: ReorderGrabGeometry,
): ReorderGrabPaint | null {
  const size = nativeReorderLineSize(view, scene.node),
    box = scene.node.getBoundingClientRect();
  if (!size) return null;
  const scaleX = box.width / size.width,
    scaleY = box.height / size.height;
  if (![box.left, box.top, scaleX, scaleY].every(Number.isFinite) || scaleX <= 0 || scaleY <= 0)
    return null;
  const nodes = admitNativeReorderPaint([scene.node]);
  if (!nodes) return null;
  const prepared = prepareNativeReorderPaint(view, [scene.node], nodes, 'copy');
  if (!prepared) return null;
  return withReorderPaintMaterialization(
    view.dom.ownerDocument,
    (materialization) => {
      const copy = copyNativeReorderPaint(
        view,
        scene.node,
        prepared,
        materialization,
      ) as HTMLElement | null;
      if (!copy) return null;
      let proxy: HTMLElement | null = null;
      try {
        // Resolved snapshot declarations own detached paint; structural editor selectors remain exclusive to the native view.
        for (const node of [
          copy,
          ...copy.querySelectorAll<HTMLElement>('.cm-content, .cm-scroller'),
        ]) {
          node.setAttribute(
            'class',
            (node.getAttribute('class') ?? '')
              .split(/\s+/)
              .filter((name) => name !== 'cm-content' && name !== 'cm-scroller')
              .join(' '),
          );
        }
        proxy = view.dom.ownerDocument.createElement('span');
        proxy.className = 'sp-reorder-proxy';
        proxy.inert = true;
        proxy.setAttribute('aria-hidden', 'true');
        Object.assign(proxy.style, {
          position: 'fixed',
          width: `${geometry.width}px`,
          height: `${geometry.height}px`,
          padding: '0',
          border: '0',
          margin: '0',
          background: 'transparent',
          pointerEvents: 'none',
        });
        proxy.style.clipPath = `path("${geometry.fragments.map((rect) => `M${rect.left - geometry.left - 4} ${rect.top - geometry.top - 2}h${rect.right - rect.left + 8}v${rect.bottom - rect.top + 4}h${rect.left - rect.right - 8}Z`).join(' ')}")`;
        Object.assign(copy.style, {
          position: 'absolute',
          left: `${box.left - geometry.left}px`,
          top: `${box.top - geometry.top}px`,
          transformOrigin: '0 0',
          transform: `scale(${scaleX},${scaleY})`,
          clipPath: 'none',
          clip: 'auto',
        });
        proxy.append(copy);
        return { node: proxy, geometry };
      } catch (primary) {
        const causes: unknown[] = [primary];
        for (const release of [() => copy.remove(), () => proxy?.remove()]) {
          try {
            release();
          } catch (cause) {
            causes.push(cause);
          }
        }
        if (causes.length === 1) throw primary;
        throw new AggregateError(causes, 'reorder-motion-grab-retirement-failed');
      }
    },
    (grab) => grab?.node.remove(),
  );
}
