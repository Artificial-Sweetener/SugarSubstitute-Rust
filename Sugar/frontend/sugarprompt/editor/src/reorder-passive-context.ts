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
/** Native READ captures one bounded passive context and editor appearance independently of proposal paint lifetime. */
import type { EditorView } from '@codemirror/view';
import { isReorderDerivedRootAttribute } from './reorder-derived-root-attribute.js';
import type { PreparedPassiveMotion } from './reorder-motion-contracts.js';
import { reorderMotionAttachmentReady } from './reorder-motion-layer.js';
import {
  admitReorderLayout,
  admitReorderLineLayout,
  admitReorderTargetLayout,
} from './reorder-layout-admission.js';
import {
  borrowNativeReorderLayoutLease,
  currentNativeReorderLineOpacity,
} from './reorder-native-projection.js';
import { currentReorderProjectionTargets } from './reorder-paint-projection.js';
import { preparedReorderRowPaintAdmission } from './reorder-widget-plan.js';
import { reorderPreview } from './reorder-preview.js';
import { readReorderBaseContext } from './reorder-base-context.js';
import { prepareReorderWrapperPaintStyle } from './reorder-paint-style.js';

/** Compare the bounded live native attribute sequence without materializing another prepared snapshot. */
export function matchesMotionViewAppearance(
  node: HTMLElement,
  prepared: readonly (readonly [string, string])[],
): boolean {
  const attributes = node.attributes,
    length = attributes.length;
  if (length > 64) return false;
  let matched = 0;
  for (let index = 0; index < length; index++) {
    const attribute = attributes[index];
    if (!attribute) return false;
    const name = attribute.name;
    if (isReorderDerivedRootAttribute(name)) continue;
    const expected = prepared[matched++];
    if (!expected || name !== expected[0] || attribute.value !== expected[1]) return false;
  }
  return attributes.length === length && matched === prepared.length;
}
/** Bound the real native editor appearance stamp; issued mask attributes are derived display output, not appearance inputs. */
export function motionViewAppearance(
  node: HTMLElement,
): readonly (readonly [string, string])[] | null {
  if (node.attributes.length > 64) return null;
  return Object.freeze(
    Array.from(node.attributes)
      .filter((attribute) => !isReorderDerivedRootAttribute(attribute.name))

      .map((attribute) => Object.freeze([attribute.name, attribute.value] as const)),
  );
}

/** Current bounded source output and native wrapper context are captured only in the existing CM read phase. */
export function preparePassiveMotion(
  view: EditorView,
  held: number | null,
): PreparedPassiveMotion | null {
  const currentPublication = view.state.field(reorderPreview, false),
    preview = currentPublication?.value;
  // A new hold can request whole Stage paint over sticky canonical rows; partial masks keep their current native Layer resources.
  if (
    currentPublication?.paint &&
    !currentPublication.native &&
    currentPublication.pointer?.mode === 'stage' &&
    preparedReorderRowPaintAdmission(currentPublication.paint) !== 'absent'
  )
    return null;
  if (!preview || !reorderMotionAttachmentReady(view)) return null;
  const targets = currentReorderProjectionTargets(view);
  if (!targets) return null;
  /** Discovery retains its original target-first failure order and independent line admission. */
  const readLines = (): readonly HTMLElement[] | null => {
    const lines = view.contentDOM.querySelectorAll<HTMLElement>('.cm-line');
    return !lines.length || lines.length > 16 || view.dom.attributes.length > 64
      ? null
      : Array.from(lines);
  };
  const nativeCurrent = borrowNativeReorderLayoutLease(view, targets);
  if (nativeCurrent) {
    if (!admitReorderLayout(targets, readLines, nativeCurrent)) return null;
  } else {
    if (!admitReorderTargetLayout(targets)) return null;
    const lines = readLines();
    if (!lines || !admitReorderLineLayout(lines)) return null;
  }
  const appearance = motionViewAppearance(view.dom);
  if (!appearance) return null;
  const context = readReorderBaseContext(view, preview),
    window = view.dom.ownerDocument.defaultView;
  if (!context || !window) return null;
  const range = context.prefix,
    position = view.domAtPos(range.from),
    element =
      position.node.nodeType === 1 ? (position.node as HTMLElement) : position.node.parentElement;
  const line = element?.closest<HTMLElement>('.cm-line');
  if (!line) return null;
  const publication = view.state.field(reorderPreview),
    nativeWhole = publication.native && publication.pointer?.mode === 'stage',
    opacity = nativeWhole ? currentNativeReorderLineOpacity(view, line) : undefined;
  if (nativeWhole && opacity === null) return null;
  const scrollDOM = view.scrollDOM,
    scrollLeft = scrollDOM.scrollLeft,
    scrollTop = scrollDOM.scrollTop,
    windowX = window.scrollX,
    windowY = window.scrollY,
    windowWidth = window.innerWidth,
    windowHeight = window.innerHeight,
    deviceScale = window.devicePixelRatio;
  return Object.freeze({
    preview,
    ...(view.state.field(reorderPreview).pointer?.mode === 'stage' ? { whole: true as const } : {}),
    context,
    held,
    readViewportCurrent: () =>
      view.scrollDOM === scrollDOM &&
      scrollDOM.scrollLeft === scrollLeft &&
      scrollDOM.scrollTop === scrollTop &&
      window.scrollX === windowX &&
      window.scrollY === windowY &&
      window.innerWidth === windowWidth &&
      window.innerHeight === windowHeight &&
      window.devicePixelRatio === deviceScale,
    contentClasses: view.contentDOM.className,
    appearance,
    contentCss: prepareReorderWrapperPaintStyle(
      window.getComputedStyle(view.contentDOM),
      view.contentDOM.style,
    ).cssText,
    lineCss: prepareReorderWrapperPaintStyle(
      window.getComputedStyle(line),
      line.style,
      opacity ?? undefined,
    ).cssText,
    scrollerCss: prepareReorderWrapperPaintStyle(
      window.getComputedStyle(view.scrollDOM),
      view.scrollDOM.style,
    ).cssText,
    scrollerClasses: view.scrollDOM.className,
  });
}
