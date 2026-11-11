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
/** Route embedded Nodes 1.0 pointers through Comfy's native widget handlers. */

import type { ComfyNode } from '../types/graph.js';
import type { NumericVector } from '../types/graph.js';
import { withCubeFaceNodePresentation } from './CubeFaceNodePresentationPolicy.js';

export interface NativeWidgetPointer {
  eDown?: Event;
  eUp?: Event;
  onClick?: (event: Event) => void;
  onDrag?: (event: Event) => void;
  finally?: () => void;
}

export interface InteractiveLiteGraphNode extends ComfyNode {
  pos: NumericVector;
  size: NumericVector;
  getWidgetOnPos(x: number, y: number): unknown;
}

interface ComfyWidgetInteractionHost {
  graphMouse: NumericVector;
  processWidgetClick(
    event: Event,
    node: InteractiveLiteGraphNode,
    widget: unknown,
    pointer: NativeWidgetPointer,
  ): void;
}

export interface ComfyLiteGraphWidgetInteractionOptions extends ComfyWidgetInteractionHost {
  titleHeight?: number;
}

export interface LiteGraphWidgetInteractionMount {
  dispose(): void;
}

export interface LiteGraphCardGeometry {
  width: number;
  height: number;
}

interface PointerSession {
  originX: number;
  originY: number;
  pointer: NativeWidgetPointer;
}

export interface LiteGraphWidgetPointerSession {
  move(event: PointerEvent, graphX: number, graphY: number): void;
  finish(event: PointerEvent, graphX: number, graphY: number, cancelled: boolean): void;
}

/** Own only coordinate and pointer-lifecycle adaptation for native widgets. */
export class ComfyLiteGraphWidgetInteraction {
  readonly #host: ComfyWidgetInteractionHost;
  readonly #titleHeight: number;

  /** Bind Comfy's active canvas widget boundary. */
  constructor(options: ComfyLiteGraphWidgetInteractionOptions) {
    this.#host = options;
    this.#titleHeight = options.titleHeight ?? 30;
  }

  /** Attach native widget routing to one exact node-card canvas. */
  attach(
    canvas: HTMLCanvasElement,
    node: ComfyNode,
    getGeometry?: () => LiteGraphCardGeometry,
  ): LiteGraphWidgetInteractionMount {
    let session: LiteGraphWidgetPointerSession | null = null;
    canvas.style.touchAction = 'none';

    const onPointerDown = (event: PointerEvent): void => {
      if (!isInteractiveNode(node)) return;
      const geometry = getGeometry?.();
      const position = this.#mapPosition(canvas, node, event, geometry);
      session = this.begin(node, event, position.graphX, position.graphY, geometry);
      if (!session) return;
      canvas.setPointerCapture?.(event.pointerId);
    };
    const onPointerMove = (event: PointerEvent): void => {
      if (!session || !isInteractiveNode(node)) return;
      event.preventDefault();
      event.stopPropagation();
      const position = this.#mapPosition(canvas, node, event, getGeometry?.());
      session.move(event, position.graphX, position.graphY);
    };
    const finish = (event: PointerEvent, cancelled: boolean): void => {
      if (!session || !isInteractiveNode(node)) return;
      event.preventDefault();
      event.stopPropagation();
      const position = this.#mapPosition(canvas, node, event, getGeometry?.());
      session.finish(event, position.graphX, position.graphY, cancelled);
      canvas.releasePointerCapture?.(event.pointerId);
      session = null;
    };
    const onPointerUp = (event: PointerEvent): void => finish(event, false);
    const onPointerCancel = (event: PointerEvent): void => finish(event, true);

    canvas.addEventListener('pointerdown', onPointerDown);
    canvas.addEventListener('pointermove', onPointerMove);
    canvas.addEventListener('pointerup', onPointerUp);
    canvas.addEventListener('pointercancel', onPointerCancel);
    return {
      dispose: () => {
        canvas.removeEventListener('pointerdown', onPointerDown);
        canvas.removeEventListener('pointermove', onPointerMove);
        canvas.removeEventListener('pointerup', onPointerUp);
        canvas.removeEventListener('pointercancel', onPointerCancel);
      },
    };
  }

  /** Begin one native widget interaction from any Cube-owned pointer surface. */
  begin(
    node: ComfyNode,
    event: PointerEvent,
    graphX: number,
    graphY: number,
    geometry?: LiteGraphCardGeometry,
  ): LiteGraphWidgetPointerSession | null {
    if (!isInteractiveNode(node)) return null;
    const session: PointerSession = {
      originX: event.clientX,
      originY: event.clientY,
      pointer: { eDown: event },
    };
    const widget = withFacePresentation(node, geometry, this.#titleHeight, () =>
      node.getWidgetOnPos(graphX, graphY),
    );
    if (!widget) return null;
    event.preventDefault();
    event.stopPropagation();
    writeCanvasCoordinates(event, graphX, graphY);
    this.#writeGraphMouse(graphX, graphY);
    withFacePresentation(node, geometry, this.#titleHeight, () => {
      this.#host.processWidgetClick(event, node, widget, session.pointer);
    });
    return {
      move: (moveEvent, nextGraphX, nextGraphY) => {
        writeCanvasCoordinates(moveEvent, nextGraphX, nextGraphY);
        this.#writeGraphMouse(nextGraphX, nextGraphY);
        withFacePresentation(node, geometry, this.#titleHeight, () => {
          session.pointer.onDrag?.(moveEvent);
        });
      },
      finish: (upEvent, nextGraphX, nextGraphY, cancelled) => {
        writeCanvasCoordinates(upEvent, nextGraphX, nextGraphY);
        this.#writeGraphMouse(nextGraphX, nextGraphY);
        const moved =
          Math.hypot(upEvent.clientX - session.originX, upEvent.clientY - session.originY) > 3;
        session.pointer.eUp = upEvent;
        withFacePresentation(node, geometry, this.#titleHeight, () => {
          if (!cancelled && !moved) session.pointer.onClick?.(upEvent);
          session.pointer.finally?.();
        });
      },
    };
  }

  /** Convert CSS pixels into the exact node-local coordinate system Comfy draws. */
  #mapPosition(
    canvas: HTMLCanvasElement,
    node: InteractiveLiteGraphNode,
    event: PointerEvent,
    geometry?: LiteGraphCardGeometry,
  ): { graphX: number; graphY: number } {
    const bounds = canvas.getBoundingClientRect();
    const candidateWidth = geometry?.width ?? Number(node.size[0]);
    const candidateHeight = geometry?.height ?? Number(node.size[1]) + this.#titleHeight;
    const width = Number.isFinite(candidateWidth) ? Math.max(1, candidateWidth) : 1;
    const height = Number.isFinite(candidateHeight) ? Math.max(1, candidateHeight) : 1;
    const localX = ((event.clientX - bounds.left) / Math.max(1, bounds.width)) * width;
    const localY =
      ((event.clientY - bounds.top) / Math.max(1, bounds.height)) * height - this.#titleHeight;
    return {
      graphX: Number(node.pos[0]) + localX,
      graphY: Number(node.pos[1]) + localY,
    };
  }

  /** Keep Comfy's active canvas coordinate state aligned for native handlers. */
  #writeGraphMouse(x: number, y: number): void {
    this.#host.graphMouse[0] = x;
    this.#host.graphMouse[1] = y;
  }
}

/** Present the rendered card size only while Comfy handles one widget event. */
function withPresentationSize<Result>(
  node: InteractiveLiteGraphNode,
  geometry: LiteGraphCardGeometry | undefined,
  titleHeight: number,
  callback: () => Result,
): Result {
  if (!geometry) return callback();
  const width = Number(geometry.width);
  const bodyHeight = Number(geometry.height) - titleHeight;
  const originalSize: [number, number] = [Number(node.size[0]), Number(node.size[1])];
  if (Number.isFinite(width) && width > 0) node.size[0] = width;
  if (Number.isFinite(bodyHeight) && bodyHeight > 0) node.size[1] = bodyHeight;
  try {
    return callback();
  } finally {
    node.size[0] = originalSize[0];
    node.size[1] = originalSize[1];
  }
}

/** Apply every transient Nodes 1.0 Cube-face geometry and state override together. */
function withFacePresentation<Result>(
  node: InteractiveLiteGraphNode,
  geometry: LiteGraphCardGeometry | undefined,
  titleHeight: number,
  callback: () => Result,
): Result {
  return withCubeFaceNodePresentation(node, () =>
    withPresentationSize(node, geometry, titleHeight, callback),
  );
}

/** Validate the narrow native node interaction surface. */
function isInteractiveNode(node: ComfyNode): node is InteractiveLiteGraphNode {
  return (
    typeof node.getWidgetOnPos === 'function' && node.pos !== undefined && node.size !== undefined
  );
}

/** Add the coordinates Comfy's CanvasPointerEvent consumers expect. */
function writeCanvasCoordinates(event: Event, x: number, y: number): void {
  Object.defineProperties(event, {
    canvasX: { configurable: true, value: x },
    canvasY: { configurable: true, value: y },
  });
}
