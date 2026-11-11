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
/** Read masonry model lineage through native graph connections and portable Cube requirements. */
import { isRecord } from '../types/common.js';
import type { UnknownRecord } from '../types/common.js';
import type { ModelCatalogItem, ModelCatalogSnapshot } from './ModelCatalog.js';
import type { ModelPickerBinding } from './ModelPickerBinding.js';
import type { ModelCompatibilityContext } from './ModelCompatibility.js';
import { decodeModelRequirement } from '../cube/ModelRequirement.js';
import { resolveModelFamily } from './ModelRelationships.js';
import { isCubeNode } from '../cube/node/ComfyCubeNodeFactory.js';
import { readCubeModelRequirement } from '../cube/node/CubeNodeInstanceCatalog.js';
import { isScopedModelKind } from './ModelPickerCompatibilityScope.js';
import { locateModelGraph } from './ComfyModelGraph.js';
import type { GraphScope } from './ComfyModelGraph.js';

/** Restrict special filtering to masonry, including native cards projected on Cube faces. */
export function comfyModelCompatibilityContext(
  app: unknown,
  binding: ModelPickerBinding,
  snapshot: ModelCatalogSnapshot,
): ModelCompatibilityContext | null {
  if (!isScopedModelKind(binding.kind)) return null;
  if (!binding.anchor.closest('.sugarcubes-workflow-masonry, .sugarcubes-cube-face')) return null;
  const located = locateModelGraph(app, binding);
  if (!located) return null;
  const { scope, node } = located;
  const addon = binding.kind === 'loras';
  const assigned = isCubeNode(scope.owner) ? readCubeModelRequirement(scope.owner) : null;
  if (assigned && !assigned.architecture && !assigned.family) return null;
  if (addon) {
    const model = upstreamModel(scope, node, snapshot, new Set());
    if (model) {
      const architecture = model.traits?.architecture || model.architecture?.base || '';
      const family = model.traits?.conflict
        ? ''
        : model.traits?.family ||
          resolveModelFamily(snapshot.relationships, architecture)?.key ||
          '';
      if (architecture || family)
        return { architecture, family, addon, label: model.traits?.familyName || architecture };
    }
  }
  const extra = isRecord(scope.graph.extra) ? scope.graph.extra : {};
  const identity = isRecord(extra.sugarcubes_cube) ? extra.sugarcubes_cube : {};
  const document = isRecord(extra.sugarcubes_document) ? extra.sugarcubes_document : {};
  const metadata = isRecord(document.metadata) ? document.metadata : {};
  const requirement =
    assigned ?? decodeModelRequirement(identity.model_requirements ?? metadata.model_requirements);
  if (requirement)
    return requirement.architecture || requirement.family
      ? { ...requirement, addon, label: requirement.family || requirement.architecture }
      : null;
  const target = typeof identity.target_model === 'string' ? identity.target_model : '';
  const family = resolveModelFamily(snapshot.relationships, target);
  return family?.architecture
    ? {
        architecture: family.architecture,
        family: addon || family.parent ? family.key : '',
        addon,
        label: family.name,
      }
    : null;
}

/** Follow MODEL edges through reroutes and chained addons without choosing one side of a branch. */
function upstreamModel(
  scope: GraphScope,
  node: UnknownRecord,
  snapshot: ModelCatalogSnapshot,
  seen: Set<object>,
): ModelCatalogItem | null {
  if (seen.has(node) || seen.size > 2048) return null;
  seen.add(node);
  if (Array.isArray(node.widgets))
    for (const widget of node.widgets) {
      if (!isRecord(widget) || typeof widget.name !== 'string' || typeof widget.value !== 'string')
        continue;
      const value = widget.value;
      const kind = snapshot.fieldContracts.find(
        (field) => field.classType === node.type && field.inputName === widget.name,
      )?.kind;
      if (kind !== 'checkpoints' && kind !== 'diffusion_models') continue;
      const normalize = (value: string) => value.replaceAll('\\', '/').toLowerCase();
      const matches = snapshot.items.filter(
        (item) => item.kind === kind && normalize(item.value) === normalize(value),
      );
      return matches.length === 1 ? (matches[0] ?? null) : null;
    }
  const inputs = Array.isArray(node.inputs)
    ? node.inputs
        .filter(isRecord)
        .filter(
          (input) => input.type === 'MODEL' || input.name === 'model' || node.type === 'Reroute',
        )
    : [];
  if (inputs.length !== 1 || inputs[0]?.link === undefined || inputs[0].link === null) return null;
  const id = inputs[0].link;
  const links = scope.graph.links ?? scope.graph._links;
  const link: unknown =
    links instanceof Map
      ? links.get(id)
      : isRecord(links)
        ? links[String(id)]
        : Array.isArray(links)
          ? links.find((value) => isRecord(value) && value.id === id)
          : null;
  if (!isRecord(link)) return null;
  const origin = scope.nodes.find((candidate) => String(candidate.id) === String(link.origin_id));
  return origin ? upstreamModel(scope, origin, snapshot, seen) : null;
}
