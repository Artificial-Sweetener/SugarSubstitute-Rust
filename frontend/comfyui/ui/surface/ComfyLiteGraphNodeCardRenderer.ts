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
/** Render exact Nodes 1.0 cards through Comfy's active LiteGraph canvas renderer. */

import type { ComfyNode } from '../types/graph.js';
import type { NativeNodeCardMount, NativeNodeCardRenderer } from './NativeNodeCardRenderer.js';
import type {
  ComfyLiteGraphWidgetInteraction,
  LiteGraphWidgetInteractionMount,
} from './ComfyLiteGraphWidgetInteraction.js';
import { measureCubeFaceNodeBodyHeight } from './CubeFaceNodeMeasurement.js';
import { withCubeFaceNodePresentation } from './CubeFaceNodePresentationPolicy.js';
import type { CubeNodeColorTheme } from './CubeNodeColorTheme.js';

interface LiteGraphNodeRenderer {
  ds?: {
    scale?: number;
  };
  drawNode(node: ComfyNode, context: CanvasRenderingContext2D): void;
}

interface LegacyPresentationNode extends ComfyNode {
  arrange?(): void;
  drawSlots: ((...args: unknown[]) => void) | undefined;
  drawCollapsedSlots: ((...args: unknown[]) => void) | undefined;
  drawTitleBox?: (...args: unknown[]) => void;
  onDrawTitleBox?: (...args: unknown[]) => void;
  onDrawBackground: ((...args: unknown[]) => void) | undefined;
  title_buttons: unknown[] | undefined;
  skip_subgraph_button?: boolean;
  updateArea?(context?: CanvasRenderingContext2D): void;
  imgs?: unknown;
  animatedImages?: unknown;
  imageIndex?: unknown;
}

type PreviewField = 'imgs' | 'animatedImages' | 'imageIndex';

interface PreviewFieldPresentation {
  field: PreviewField;
  owned: boolean;
  value: unknown;
}

interface ColorFieldPresentation {
  field: 'color' | 'bgcolor';
  owned: boolean;
  value: unknown;
}

export interface ComfyLiteGraphNodeCardRendererOptions {
  document: Document;
  canvasRenderer: LiteGraphNodeRenderer;
  titleHeight: number;
  devicePixelRatio?(): number;
  widgetInteraction?: ComfyLiteGraphWidgetInteraction;
}

/** Own exact legacy canvas draws and the narrow Cube-face presentation boundary. */
export class ComfyLiteGraphNodeCardRenderer implements NativeNodeCardRenderer {
  readonly #document: Document;
  readonly #canvasRenderer: LiteGraphNodeRenderer;
  readonly #titleHeight: number;
  readonly #devicePixelRatio: () => number;
  readonly #widgetInteraction: ComfyLiteGraphWidgetInteraction | null;
  readonly #mounts = new Set<NativeNodeCardMount>();

  /** Bind the currently active Comfy LiteGraph renderer. */
  constructor(options: ComfyLiteGraphNodeCardRendererOptions) {
    this.#document = options.document;
    this.#canvasRenderer = options.canvasRenderer;
    this.#titleHeight = options.titleHeight;
    this.#devicePixelRatio = options.devicePixelRatio ?? (() => globalThis.devicePixelRatio || 1);
    this.#widgetInteraction = options.widgetInteraction ?? null;
  }

  /** Draw one exact internal graph node with Comfy's Nodes 1.0 renderer. */
  mount(target: HTMLElement, node: ComfyNode): NativeNodeCardMount {
    const canvas = this.#document.createElement('canvas');
    canvas.dataset.cubeFaceNative = 'nodes-1';
    canvas.className = 'sugarcubes-native-node-card sugarcubes-native-node-card--legacy';
    canvas.style.display = 'block';
    canvas.style.width = '100%';
    target.replaceChildren(canvas);
    let disposed = false;
    let interactionMount: LiteGraphWidgetInteractionMount | null = null;
    let logicalWidth = 1;
    let logicalHeight = 1;

    const draw = (): void => {
      if (disposed) return;
      const width = Math.max(1, Number(node.size?.[0]) || 200);
      const height = measureCubeFaceNodeBodyHeight(node);
      const titleHeight = this.#titleHeight;
      logicalWidth = width;
      logicalHeight = height + titleHeight;
      const ratio = Math.max(1, this.#devicePixelRatio());
      canvas.width = Math.ceil(width * ratio);
      canvas.height = Math.ceil((height + titleHeight) * ratio);
      canvas.style.aspectRatio = `${width} / ${height + titleHeight}`;
      const context = canvas.getContext('2d');
      if (!context) {
        throw new Error('Canvas 2D context is unavailable for Nodes 1.0 Cube cards.');
      }
      context.setTransform(ratio, 0, 0, ratio, 0, 0);
      context.clearRect(0, 0, width, height + titleHeight);
      context.save();
      context.translate(0, titleHeight);
      drawNativeLiteGraphCubeCard(this.#canvasRenderer, node, context, {
        normalizeRendererScale: true,
        presentationHeight: height,
      });
      context.restore();
    };
    draw();
    interactionMount =
      this.#widgetInteraction?.attach(canvas, node, () => ({
        width: logicalWidth,
        height: logicalHeight,
      })) ?? null;

    const mount: NativeNodeCardMount = {
      refresh: draw,
      unmount: () => {
        if (disposed) return;
        disposed = true;
        interactionMount?.dispose();
        canvas.remove();
        this.#mounts.delete(mount);
      },
    };
    this.#mounts.add(mount);
    return mount;
  }

  /** Remove every legacy card owned by this renderer. */
  dispose(): void {
    for (const mount of [...this.#mounts]) mount.unmount();
  }
}

/** Invoke Comfy's exact draw while masking only Cube-face-excluded chrome. */
export function drawNativeLiteGraphCubeCard(
  canvasRenderer: LiteGraphNodeRenderer,
  node: ComfyNode,
  context: CanvasRenderingContext2D,
  options: {
    normalizeRendererScale?: boolean;
    presentationWidth?: number;
    presentationHeight?: number;
    theme?: CubeNodeColorTheme;
  } = {},
): void {
  const presentationNode = node as LegacyPresentationNode;
  const drawSlots = presentationNode.drawSlots;
  const drawCollapsedSlots = presentationNode.drawCollapsedSlots;
  const titleBox = maskSubgraphTitleBox(presentationNode);
  const titleButtons = presentationNode.title_buttons;
  const previews = maskPreviewMedia(presentationNode);
  const originalSize = applyPresentationSize(
    presentationNode,
    options.presentationWidth,
    options.presentationHeight,
  );
  const colors = applyPresentationTheme(presentationNode, options.theme);
  const activeGraphScale = canvasRenderer.ds?.scale;
  try {
    if (canvasRenderer.ds && options.normalizeRendererScale === true) {
      canvasRenderer.ds.scale = 1;
    }
    presentationNode.drawSlots = () => {};
    presentationNode.drawCollapsedSlots = () => {};
    presentationNode.title_buttons = [];
    withCubeFaceNodePresentation(presentationNode, () => {
      context.textBaseline = 'alphabetic';
      presentationNode.arrange?.();
      presentationNode.updateArea?.(context);
      canvasRenderer.drawNode(node, context);
    });
  } finally {
    if (
      canvasRenderer.ds &&
      activeGraphScale !== undefined &&
      options.normalizeRendererScale === true
    ) {
      canvasRenderer.ds.scale = activeGraphScale;
    }
    presentationNode.drawSlots = drawSlots;
    presentationNode.drawCollapsedSlots = drawCollapsedSlots;
    restoreSubgraphTitleBox(presentationNode, titleBox);
    presentationNode.title_buttons = titleButtons;
    restorePreviewMedia(presentationNode, previews);
    restorePresentationTheme(presentationNode, colors);
    restorePresentationSize(presentationNode, originalSize, context);
  }
}

interface TitleBoxPresentation {
  drawTitleBoxOwned: boolean;
  drawTitleBox: LegacyPresentationNode['drawTitleBox'];
  onDrawTitleBoxOwned: boolean;
  onDrawTitleBox: LegacyPresentationNode['onDrawTitleBox'];
  skipSubgraphButtonOwned: boolean;
  skipSubgraphButton: LegacyPresentationNode['skip_subgraph_button'];
}

const SUBGRAPH_TYPE_ID =
  /^[0-9a-f]{8}-[0-9a-f]{4}-[1-5][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/i;

/** Replace the subgraph workflow glyph with the standard Cube-face title decoration. */
function maskSubgraphTitleBox(node: LegacyPresentationNode): TitleBoxPresentation | null {
  if (!hasSubgraphTitleBox(node)) return null;
  const presentation = {
    drawTitleBoxOwned: Object.prototype.hasOwnProperty.call(node, 'drawTitleBox'),
    drawTitleBox: node.drawTitleBox,
    onDrawTitleBoxOwned: Object.prototype.hasOwnProperty.call(node, 'onDrawTitleBox'),
    onDrawTitleBox: node.onDrawTitleBox,
    skipSubgraphButtonOwned: Object.prototype.hasOwnProperty.call(node, 'skip_subgraph_button'),
    skipSubgraphButton: node.skip_subgraph_button,
  };
  node.drawTitleBox = drawCubeFaceTitleBox;
  node.skip_subgraph_button = true;
  return presentation;
}

/** Draw the unobtrusive three-line title decoration used by Cube-face node cards. */
function drawCubeFaceTitleBox(...args: unknown[]): void {
  const context = args[0];
  if (!isTitleBoxContext(context)) return;
  const titleHeight = readTitleBoxHeight(args[1]);
  context.save();
  context.strokeStyle = 'rgba(255, 255, 255, 0.75)';
  context.lineWidth = 1.5;
  context.lineCap = 'round';
  for (const y of [-17, -13, -9]) {
    context.beginPath();
    context.moveTo(10, y + titleHeight - 24);
    context.lineTo(20, y + titleHeight - 24);
    context.stroke();
  }
  context.restore();
}

/** Validate the narrow Canvas API surface used by the face-card title decoration. */
function isTitleBoxContext(
  value: unknown,
): value is Pick<
  CanvasRenderingContext2D,
  | 'save'
  | 'restore'
  | 'beginPath'
  | 'moveTo'
  | 'lineTo'
  | 'stroke'
  | 'strokeStyle'
  | 'lineWidth'
  | 'lineCap'
> {
  if (typeof value !== 'object' || value === null) return false;
  return (
    'save' in value &&
    typeof value.save === 'function' &&
    'restore' in value &&
    typeof value.restore === 'function' &&
    'beginPath' in value &&
    typeof value.beginPath === 'function' &&
    'moveTo' in value &&
    typeof value.moveTo === 'function' &&
    'lineTo' in value &&
    typeof value.lineTo === 'function' &&
    'stroke' in value &&
    typeof value.stroke === 'function'
  );
}

/** Read LiteGraph's title height without coupling to its private draw-options type. */
function readTitleBoxHeight(options: unknown): number {
  if (typeof options !== 'object' || options === null || !('title_height' in options)) return 24;
  const titleHeight = Number(options.title_height);
  return Number.isFinite(titleHeight) && titleHeight > 0 ? titleHeight : 24;
}

/** Identify Comfy subgraph nodes across host versions before suppressing their workflow glyph. */
function hasSubgraphTitleBox(node: LegacyPresentationNode): boolean {
  if (node.isSubgraphNode?.()) return true;
  if ('subgraph' in node) return true;
  if (
    node.title_buttons?.some(
      (button) =>
        typeof button === 'object' &&
        button !== null &&
        'name' in button &&
        button.name === 'enter_subgraph',
    ) === true
  ) {
    return true;
  }
  return typeof node.type === 'string' && SUBGRAPH_TYPE_ID.test(node.type);
}

/** Restore the exact title-box method ownership after one projected card draw. */
function restoreSubgraphTitleBox(
  node: LegacyPresentationNode,
  presentation: TitleBoxPresentation | null,
): void {
  if (!presentation) return;
  Reflect.set(node, 'drawTitleBox', presentation.drawTitleBox);
  if (!presentation.drawTitleBoxOwned) Reflect.deleteProperty(node, 'drawTitleBox');
  Reflect.set(node, 'onDrawTitleBox', presentation.onDrawTitleBox);
  if (!presentation.onDrawTitleBoxOwned) Reflect.deleteProperty(node, 'onDrawTitleBox');
  Reflect.set(node, 'skip_subgraph_button', presentation.skipSubgraphButton);
  if (!presentation.skipSubgraphButtonOwned) {
    Reflect.deleteProperty(node, 'skip_subgraph_button');
  }
}

/** Apply the parent Cube colors only while Comfy draws one projected card. */
function applyPresentationTheme(
  node: LegacyPresentationNode,
  theme: CubeNodeColorTheme | undefined,
): ColorFieldPresentation[] {
  if (!theme) return [];
  const presentations = (['color', 'bgcolor'] as const).map((field) => ({
    field,
    owned: Object.prototype.hasOwnProperty.call(node, field),
    value: node[field],
  }));
  node.color = theme.header;
  node.bgcolor = theme.body;
  return presentations;
}

/** Restore exact internal-node color values and property ownership after drawing. */
function restorePresentationTheme(
  node: LegacyPresentationNode,
  presentations: readonly ColorFieldPresentation[],
): void {
  for (const presentation of presentations) {
    if (presentation.owned) Reflect.set(node, presentation.field, presentation.value);
    else Reflect.deleteProperty(node, presentation.field);
  }
}

/** Apply one temporary body size so Comfy lays out the exact face card. */
function applyPresentationSize(
  node: LegacyPresentationNode,
  presentationWidth: number | undefined,
  presentationHeight: number | undefined,
): [number, number] | null {
  const width = Number(presentationWidth);
  const height = Number(presentationHeight);
  if (!node.size) return null;
  const hasWidth = Number.isFinite(width) && width > 0;
  const hasHeight = Number.isFinite(height) && height > 0;
  if (!hasWidth && !hasHeight) return null;
  const originalSize: [number, number] = [Number(node.size[0]), Number(node.size[1])];
  if (hasWidth) node.size[0] = width;
  if (hasHeight) node.size[1] = height;
  return originalSize;
}

/** Restore internal graph geometry after the face-only native draw. */
function restorePresentationSize(
  node: LegacyPresentationNode,
  originalSize: [number, number] | null,
  context: CanvasRenderingContext2D,
): void {
  if (originalSize !== null && node.size) {
    node.size[0] = originalSize[0];
    node.size[1] = originalSize[1];
  }
  node.arrange?.();
  node.updateArea?.(context);
}

/** Suppress only Comfy's standard local preview state during a Cube-face draw. */
function maskPreviewMedia(node: LegacyPresentationNode): PreviewFieldPresentation[] {
  const presentations: PreviewFieldPresentation[] = [];
  for (const field of ['imgs', 'animatedImages', 'imageIndex'] as const) {
    presentations.push({
      field,
      owned: Object.prototype.hasOwnProperty.call(node, field),
      value: node[field],
    });
  }
  node.imgs = [];
  node.animatedImages = [];
  node.imageIndex = null;
  return presentations;
}

/** Restore preview fields without changing their original ownership semantics. */
function restorePreviewMedia(
  node: LegacyPresentationNode,
  presentations: readonly PreviewFieldPresentation[],
): void {
  for (const presentation of presentations) {
    if (!presentation.owned) {
      Reflect.deleteProperty(node, presentation.field);
      continue;
    }
    node[presentation.field] = presentation.value;
  }
}
