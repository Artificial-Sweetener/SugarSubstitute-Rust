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
/** Reserve Cube boundary gutters and lay out canonical port anchors. */

import { CUBE_PREVIEW_EDGE_INSET } from './CubePreviewRailGeometry.js';
import { resolveCubePreviewTitleAnchors, type CubePreviewRect } from './CubePreviewSections.js';
import {
  resolveCubeCanvasInputSocketCenterX,
  resolveCubeCanvasOutputSocketCenterX,
} from './CubeCanvasBoundaryGeometry.js';

/** Reuse the shared finite rectangle contract for Cube port gutters. */
export type CubePortRect = CubePreviewRect;

export interface CubePortLayout {
  index: number;
  name: string;
  type: string;
  x: number;
  y: number;
  defaultY: number;
  minY: number;
  maxY: number;
  labelY: number;
  slot: unknown;
}

export interface CubePortGutterGeometry {
  content: CubePortRect;
  inputGutter: CubePortRect;
  outputGutter: CubePortRect;
}

export interface CubePortPresence {
  hasInputs: boolean;
  hasOutputs: boolean;
}

/** Reserve enough width for common input type labels and their socket. */
export const CUBE_INPUT_GUTTER_WIDTH = 84;
/** Reserve the right-side rail that contains the output socket and leader elbow. */
export const CUBE_OUTPUT_GUTTER_WIDTH = CUBE_PREVIEW_EDGE_INSET;
const PORT_VERTICAL_INSET = 12;
const PORT_ROW_HEIGHT = 20;

/** Remove only the input label gutter from face content; outputs overlay the preview rail. */
export function resolveCubePortGutters(
  frame: CubePortRect,
  content: CubePortRect,
  presence: CubePortPresence,
): CubePortGutterGeometry {
  const contentRight = content.x + content.width;
  const inputRight = presence.hasInputs
    ? Math.min(contentRight, Math.max(content.x, frame.x + CUBE_INPUT_GUTTER_WIDTH))
    : content.x;
  const outputLeft = presence.hasOutputs
    ? Math.max(inputRight, frame.x + frame.width - CUBE_OUTPUT_GUTTER_WIDTH)
    : contentRight;
  return {
    content: rect(inputRight, content.y, Math.max(1, contentRight - inputRight), content.height),
    inputGutter: presence.hasInputs
      ? rect(frame.x, content.y, Math.max(0, inputRight - frame.x), content.height)
      : emptyRect(content.x, content.y, content.height),
    outputGutter: presence.hasOutputs
      ? rect(outputLeft, content.y, Math.max(0, frame.x + frame.width - outputLeft), content.height)
      : emptyRect(contentRight, content.y, content.height),
  };
}

/** Pack canonical inputs from the native top slot row through their label gutter. */
export function layoutCubeInputPorts(
  slots: readonly unknown[],
  frame: CubePortRect,
  headerHeight: number,
  slotIndexes: readonly number[] = slots.map((_, index) => index),
): CubePortLayout[] {
  const [minY, maxY] = portRange(frame, headerHeight);
  return slots.map((slot, position) => {
    const index = slotIndexes[position] ?? position;
    const record = isPortRecord(slot) ? slot : {};
    const y = clamp(minY + index * PORT_ROW_HEIGHT, minY, maxY);
    return {
      index,
      name: readString(record.name) || `input ${String(index + 1)}`,
      type: readString(record.type) || '*',
      x: resolveCubeCanvasInputSocketCenterX(frame.x),
      y,
      defaultY: y,
      minY,
      maxY,
      labelY: y,
      slot,
    };
  });
}

/** Align canonical outputs to preview labels inside the movable leader gutter. */
export function layoutCubeOutputPorts(
  slots: readonly unknown[],
  frame: CubePortRect,
  preview: CubePortRect | null,
  headerHeight: number,
  slotIndexes: readonly number[] = slots.map((_, index) => index),
): CubePortLayout[] {
  const [minY, maxY] = portRange(frame, headerHeight);
  const anchors = preview
    ? resolveCubePreviewTitleAnchors(preview, slots.length)
    : slots.map((_, position) => distribute(minY, maxY, position, slots.length));
  return slots.map((slot, position) => {
    const index = slotIndexes[position] ?? position;
    const record = isPortRecord(slot) ? slot : {};
    const defaultY = clamp(anchors[position] ?? minY, minY, maxY);
    const labelY = preview ? defaultY : minY;
    return {
      index,
      name: readString(record.name) || `output ${String(index + 1)}`,
      type: readString(record.type) || '*',
      x: resolveCubeCanvasOutputSocketCenterX(frame.x + frame.width),
      y: defaultY,
      defaultY,
      minY,
      maxY,
      labelY,
      slot,
    };
  });
}

/** Return inclusive vertical travel shared by one direction's visual lanes. */
function portRange(frame: CubePortRect, headerHeight: number): [number, number] {
  const minY = frame.y + headerHeight + PORT_VERTICAL_INSET;
  return [minY, Math.max(minY, frame.y + frame.height - PORT_VERTICAL_INSET)];
}

/** Place one canonical lane between inclusive travel bounds. */
function distribute(minY: number, maxY: number, index: number, count: number): number {
  if (count <= 0) return minY;
  return minY + ((index + 1) * (maxY - minY)) / (count + 1);
}

/** Build one finite non-negative rectangle. */
function rect(x: number, y: number, width: number, height: number): CubePortRect {
  return {
    x: finite(x, 0),
    y: finite(y, 0),
    width: Math.max(1, finite(width, 1)),
    height: Math.max(1, finite(height, 1)),
  };
}

/** Build an absent gutter without inventing a visible one-pixel affordance. */
function emptyRect(x: number, y: number, height: number): CubePortRect {
  return {
    x: finite(x, 0),
    y: finite(y, 0),
    width: 0,
    height: Math.max(1, finite(height, 1)),
  };
}

/** Narrow one native slot enough to read its visible metadata. */
function isPortRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}

/** Read one trimmed native label or type. */
function readString(value: unknown): string {
  return typeof value === 'string' ? value.trim() : '';
}

/** Read one finite number without propagating invalid host geometry. */
function finite(value: unknown, fallback: number): number {
  const number = Number(value);
  return Number.isFinite(number) ? number : fallback;
}

/** Clamp one number to an inclusive range. */
function clamp(value: number, minimum: number, maximum: number): number {
  return Math.min(Math.max(value, minimum), maximum);
}
