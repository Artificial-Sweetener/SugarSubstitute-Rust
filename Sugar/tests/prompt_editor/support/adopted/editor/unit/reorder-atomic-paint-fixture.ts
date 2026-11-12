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
import assert from 'node:assert/strict';
import { admitNativeReorderPaint } from '../../../../../../frontend/sugarprompt/editor/src/reorder-native-paint.ts';
import { admitReorderLineLayout } from '../../../../../../frontend/sugarprompt/editor/src/reorder-layout-admission.ts';
import { nativeReorderFootprint } from '../../../../../../frontend/sugarprompt/editor/src/reorder-native-footprint.ts';
import type { ReorderMotionScene } from '../../../../../../frontend/sugarprompt/editor/src/reorder-motion-scene.ts';
/** Actual canonical LoRA/media constructors share one bounded literal native transport for complete grab and Scene-copy owner controls. */
import type { EditorView } from '@codemirror/view';
import { LoraWidget } from '../../../../../../frontend/sugarprompt/editor/src/lora-widget.ts';
import { SourceToken } from '../../../../../../frontend/sugarprompt/editor/src/projection-widgets.ts';
import { projectionState } from '../transaction/projection-state.ts';
import { motionPaintFixture } from './reorder-motion-paint-fixture.ts';
/** Actual native source canvases are distinguished from fresh destination buffers; no decoded resource is acquired by capture. */
export function atomicPaintFixture(position: 'static' | 'relative' = 'relative') {
  const f = motionPaintFixture(),
    previous = Object.getOwnPropertyDescriptor(globalThis, 'document'),
    widgets: { widget: LoraWidget; dom: HTMLElement }[] = [],
    canvases: HTMLCanvasElement[] = [],
    draws: { source: unknown; destination: unknown }[] = [];
  const causes: unknown[] = [];
  /** Every owner retires independently even when a raw construction/assertion/cleanup fault occurs. */
  const destroy = () => {
    for (const { widget, dom } of widgets) {
      try {
        widget.destroy(dom);
      } catch (cause) {
        causes.push(cause);
      }
    }
    try {
      f.destroyLayer();
    } catch (cause) {
      causes.push(cause);
    }
    try {
      if (previous) Object.defineProperty(globalThis, 'document', previous);
      else Reflect.deleteProperty(globalThis, 'document');
    } catch (cause) {
      causes.push(cause);
    }
    if (causes.length === 1) throw causes[0];
    if (causes.length) throw new AggregateError(causes, 'Atomic grab fixture retirement failed');
  };
  try {
    Object.defineProperty(globalThis, 'document', {
      configurable: true,
      value: f.document,
    });
    const create = f.document.createElement.bind(f.document);
    f.document.createElement = (tag) => {
      const node = create(tag);
      Object.assign(node.box, { left: 20, top: 20, width: 40, height: 20 });
      if (tag === 'canvas')
        Object.assign(node, {
          getContext: () => ({
            drawImage(source: unknown) {
              draws.push({ source, destination: node });
            },
          }),
        });
      return node;
    };
    f.line.replaceChildren();
    const roots = Array.from({ length: 45 }, (_, index) => {
      const root = f.document.createElement('span');
      root.className = 'sp-reorder-chip';
      root.dataset.reorderChip = String(index);
      const widget = new LoraWidget(
        new SourceToken(),
        {
          name: 'owned',
          resolution: 'found',
          model: {
            promptName: 'owned',
            backendValue: 'owned.safetensors',
            basename: 'owned',
          },
          modelPageUrl: null,
          title: 'Owned',
          subtitle: '',
          triggerWords: [],
          thumbnail: { status: 'unavailable' },
        },
        '0.70',
        'none',
        {
          status: 'ready',
          raster: {
            width: 256,
            height: 256,
            paint(canvas) {
              canvases.push(canvas);
            },
            dispose() {
              throw new Error('borrowed-raster-disposed');
            },
          },
        },
      );
      const dom = widget.toDOM(f.view);
      widgets.push({ widget, dom });
      root.append(dom as unknown as typeof root);
      f.line.append(root);
      return root;
    });
    const read = f.document.defaultView.getComputedStyle;
    /** Literal native CSS transport mirrors the canonical inline outer wrapper, LoRA flex and nested weight grid. */
    f.document.defaultView.getComputedStyle = (node, pseudo?: string) => {
      const original = read(node);
      const properties: Record<string, string> = {
        display:
          node.className === 'sp-lora-chip'
            ? 'inline-flex'
            : node.className === 'sp-lora-weight'
              ? 'inline-grid'
              : 'inline',
        position: roots.includes(node) ? position : 'static',
        inset: '0px',
        transform: 'none',
        content: 'none',
        'background-color': 'rgba(0, 0, 0, 0.055)',
        outline: '1px solid rgb(0, 0, 0)',
        'border-radius': '9px',
      };
      return {
        ...original,
        ...properties,
        content: pseudo ? 'none' : 'normal',
        outlineStyle: 'solid',
        outlineWidth: '1px',
        outlineOffset: '0px',
        getPropertyValue: (name: string) => properties[name] ?? original.getPropertyValue(name),
      };
    };
    const state = projectionState('<lora:owned:0.70>, '.repeat(45)),
      view = {
        // eslint-disable-next-line @typescript-eslint/no-misused-spread -- Native boundary fixtures intentionally project own fields without a browser class prototype.
        ...f.view,
        state,
        contentDOM: {
          contains: (node: unknown) => node === f.line,
          querySelectorAll: () => [roots[0]!],
        },
      } as unknown as EditorView;
    return { f, view, roots, widgets, canvases, draws, causes, destroy };
  } catch (primary) {
    causes.push(primary);
    destroy();
    throw primary;
  }
}

/** Run one finite fixture action and independently release every acquired native owner, retaining raw primary order. */
export function usingAtomicPaintFixture(
  action: (f: ReturnType<typeof atomicPaintFixture>) => void,
): void {
  const f = atomicPaintFixture();
  try {
    action(f);
  } catch (cause) {
    f.causes.push(cause);
  }
  f.destroy();
}

/** The original observed complete footprint, not a guessed atomic rectangle, supplies the actual Scene input. */
export function atomicScenePaintSource(
  f: ReturnType<typeof atomicPaintFixture>,
  id: number,
): ReorderMotionScene['chips'][number] {
  const root = f.roots[id]! as unknown as HTMLElement;
  assert.ok(admitNativeReorderPaint([root]));
  const nodes = admitReorderLineLayout([root]);
  assert.ok(nodes);
  const rects = nativeReorderFootprint(root, nodes, id);
  assert.ok(rects);
  return {
    id,
    roots: [{ node: root, scaleX: 1, scaleY: 1 }],
    rects,
    left: 20,
    top: 20,
  };
}
