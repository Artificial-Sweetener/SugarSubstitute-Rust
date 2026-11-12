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
/** Determine which authored native-subgraph boundaries are public Cube ports. */

interface NativeBoundarySlot {
  linkIds?: readonly unknown[];
  getLinks?(): unknown;
}

interface NativeBoundaryGraph {
  inputNode?: unknown;
  outputNode?: unknown;
  extra?: {
    sugarcubes_kind?: unknown;
    sugarcubes_cube?: { kind?: unknown };
  };
}

/** Narrow the host's dynamic native subgraph shape before reading boundaries. */
export interface CubeExternalInterfaceNode {
  inputs?: readonly unknown[];
  outputs?: readonly unknown[];
  properties?: {
    sugarcubes_kind?: unknown;
    sugarcubes_cube?: { kind?: unknown };
  };
  subgraph: NativeBoundaryGraph;
  resolveSubgraphInputLinks?(slot: number): readonly unknown[];
  resolveSubgraphOutputLink?(slot: number): unknown;
}

/** Describe the ports that are meaningful on a Cube's closed face. */
export interface CubeExternalInterface {
  inputSlots: readonly number[];
  outputSlots: readonly number[];
}

/**
 * Surface only boundaries that are connected to authored content.
 *
 * Empty draft boundaries remain in Comfy's subgraph editor, but they are not
 * part of the reusable Cube's public interface until an internal link reaches
 * them. Older host shapes without inspectable boundary slots retain all ports.
 */
export function resolveCubeExternalInterface(
  node: CubeExternalInterfaceNode,
): CubeExternalInterface {
  const graph = node.subgraph;
  const draft =
    node.properties?.sugarcubes_kind === 'cube_draft' ||
    node.properties?.sugarcubes_cube?.kind === 'draft' ||
    graph.extra?.sugarcubes_kind === 'cube_draft' ||
    graph.extra?.sugarcubes_cube?.kind === 'draft';
  return {
    inputSlots: resolveConnectedSlots(
      node.inputs ?? [],
      readBoundarySlots(graph.inputNode),
      draft,
      (slot) => node.resolveSubgraphInputLinks?.(slot)?.length ?? null,
    ),
    outputSlots: resolveConnectedSlots(
      node.outputs ?? [],
      readBoundarySlots(graph.outputNode),
      draft,
      (slot) =>
        typeof node.resolveSubgraphOutputLink === 'function'
          ? node.resolveSubgraphOutputLink(slot)
            ? 1
            : 0
          : null,
    ),
  };
}

/** Narrow a dynamic Comfy boundary node before reading its slot links. */
function readBoundarySlots(value: unknown): readonly NativeBoundarySlot[] | undefined {
  if (typeof value !== 'object' || value === null || Array.isArray(value)) return undefined;
  const slots = Reflect.get(value, 'slots');
  return Array.isArray(slots) ? (slots as NativeBoundarySlot[]) : undefined;
}

/** Preserve public ports when the host does not expose native boundary state. */
function resolveConnectedSlots(
  ports: readonly unknown[],
  boundaries: readonly NativeBoundarySlot[] | undefined,
  hideUninspectableDraftPorts: boolean,
  readNativeConnectionCount: (slot: number) => number | null,
): number[] {
  const nativeCounts = ports.map((_, index) => readNativeConnectionCount(index));
  if (nativeCounts.some((count) => count !== null)) {
    return nativeCounts.flatMap((count, index) => (count && count > 0 ? [index] : []));
  }
  if (!boundaries) return hideUninspectableDraftPorts ? [] : ports.map((_, index) => index);
  return ports.flatMap((_, index) => (hasConnections(boundaries[index]) ? [index] : []));
}

/** Read a boundary's authoritative live links without assuming Comfy's class. */
function hasConnections(boundary: NativeBoundarySlot | undefined): boolean {
  if (!boundary) return false;
  const links = boundary.getLinks?.();
  if (Array.isArray(links)) return links.length > 0;
  return (boundary.linkIds?.length ?? 0) > 0;
}
