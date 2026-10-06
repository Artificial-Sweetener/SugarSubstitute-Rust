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
/** Identify native field rows without using mounted DOM as the candidate inventory. */
import type { ComfyNode } from '../../types/graph.js';
import { isRecord } from '../../types/common.js';
import { findVueComponents, findMountedNodeVNode } from '../../surface/ComfyVueTree.js';
import { CUBE_FACE_PROJECTION_SYMBOL } from '../../surface/ComfyLiteGraphCubeFaceProjectionHost.js';
import { cubeFacePresentedWidgets } from '../../surface/CubeFaceWidgetPolicy.js';
import type {
  RelationCardSurface,
  RelationCanvasField,
} from '../presentation/RelationCardSurface.js';

/** Resolve already-mounted views of one eligible face node in either native renderer. */
export function nativeRelationCards(
  document: Document,
  node: ComfyNode,
  cubeId: string | null,
): RelationCardSurface[] {
  const locator = `${String(node.graph?.id)}:${String(node.id)}`;
  const cells = [
    ...document.querySelectorAll<HTMLElement>('.sugarcubes-cube-face__node-card'),
  ].filter((cell) => cell.dataset.cubeNodeLocator === locator);
  const roots =
    cubeId !== null
      ? cells
      : [
          ...cells,
          ...[...document.querySelectorAll<HTMLElement>('.lg-node[data-node-id]')].filter(
            (root) => root.dataset.nodeId === String(node.id),
          ),
        ];
  const projection: unknown = Reflect.get(node, CUBE_FACE_PROJECTION_SYMBOL);
  if (
    isRecord(projection) &&
    projection.container instanceof HTMLElement &&
    typeof projection.projectRect === 'function'
  ) {
    const projectRect = projection.projectRect;
    const root = projection.container;
    const width = Number.parseFloat(root.style.width);
    const bounds = root.getBoundingClientRect();
    const scale = bounds.width / width;
    const fields = canvasFields(node, width, (y) => {
      const rect: unknown = projectRect.call(projection, [0, y, 1, 1]);
      return isRecord(rect) && typeof rect.top === 'number' && scale > 0
        ? (rect.top - bounds.top) / scale
        : y;
    });
    return [
      {
        root,
        header: null,
        rows: new Map(),
        canvas: null,
        canvasFields: fields,
        canvasTitle: true,
      },
    ];
  }
  return roots.map((root) => {
    const card = root.matches('.lg-node') ? root : root.querySelector<HTMLElement>('.lg-node');
    const canvas = root.querySelector<HTMLCanvasElement>('canvas[data-cube-face-native="nodes-1"]');
    const header = card?.querySelector<HTMLElement>('.lg-node-header') ?? null;
    const rows = new Map<string, HTMLElement>();
    const vnodeRoot =
      root.querySelector<HTMLElement>('.sugarcubes-cube-face__native-card-mount') ?? card;
    const localVNode: unknown = vnodeRoot ? Reflect.get(vnodeRoot, '_vnode') : null;
    const appRoot = document.getElementById('vue-app');
    const appVNode: unknown = appRoot ? Reflect.get(appRoot, '_vnode') : null;
    const vnode =
      localVNode ??
      (card ? findMountedNodeVNode(card) : null) ??
      findVueComponents(appVNode, 'LGraphNode').find((candidate) => {
        const component = isRecord(candidate.component) ? candidate.component : {};
        const subtree = isRecord(component.subTree) ? component.subTree : {};
        return subtree.el === card;
      });
    if (vnode)
      for (const container of findVueComponents(vnode, 'NodeWidgets')) {
        const component = isRecord(container.component) ? container.component : {},
          tree = isRecord(component.subTree) ? component.subTree : {};
        if (!(tree.el instanceof HTMLElement)) continue;
        for (const name of [
          'WidgetSelect',
          'WidgetInputNumber',
          'WidgetInputNumberSlider',
          'WidgetInputText',
          'WidgetTextarea',
          'WidgetToggle',
        ])
          for (const child of findVueComponents(container, name)) {
            const instance = isRecord(child.component) ? child.component : {},
              props = isRecord(instance.props) ? instance.props : {},
              widget = isRecord(props.widget) ? props.widget : {},
              subtree = isRecord(instance.subTree) ? instance.subTree : {};
            if (!(subtree.el instanceof HTMLElement) || typeof widget.name !== 'string') continue;
            let row = subtree.el;
            while (row.parentElement && row.parentElement !== tree.el) row = row.parentElement;
            if (row.parentElement === tree.el) rows.set(widget.name, row);
          }
      }
    if (card)
      for (const row of card.querySelectorAll<HTMLElement>('[data-sc-fluent-row]')) {
        if (row.closest('.lg-node') !== card) continue;
        const label = row
          .querySelector<HTMLElement>('[data-testid="widget-layout-field-label"]')
          ?.textContent?.trim();
        const control = row.querySelector<HTMLElement>(
          '[role="combobox"][aria-label], [aria-label]',
        );
        const name = control?.getAttribute('aria-label');
        if (name) rows.set(name, row);
        else if (label) rows.set(label, row);
      }
    const aspect = canvas?.style.aspectRatio.split('/').map((value) => Number.parseFloat(value));
    const width = aspect?.[0] ?? root.clientWidth;
    const height = aspect?.[1] ?? root.clientHeight;
    const ratio = canvas && width > 0 ? canvas.getBoundingClientRect().width / width : 1;
    const canvasTop = canvas
      ? canvas.getBoundingClientRect().top - root.getBoundingClientRect().top
      : 0;
    const fields = canvas
      ? canvasFields(node, width, (y) => canvasTop + (30 + y) * ratio, ratio)
      : new Map<string, RelationCanvasField>();
    return {
      root,
      header,
      rows,
      canvas,
      canvasFields: fields,
      canvasTitle: canvas !== null && height > 0,
    };
  });
}

/** Read actual arranged LiteGraph widget positions for native DOM selectors on canvas faces. */
function canvasFields(
  node: ComfyNode,
  width: number,
  top: (y: number) => number,
  scale = 1,
): ReadonlyMap<string, RelationCanvasField> {
  const fields = new Map<string, RelationCanvasField>();
  for (const widget of cubeFacePresentedWidgets(node)) {
    const y = typeof widget.y === 'number' ? widget.y : widget.last_y;
    if (typeof y !== 'number' || !Number.isFinite(y)) continue;
    fields.set(widget.name, {
      top: top(y),
      left: 12 * scale,
      width: Math.max(1, width - 24) * scale,
      height: 24 * scale,
    });
  }
  return fields;
}
