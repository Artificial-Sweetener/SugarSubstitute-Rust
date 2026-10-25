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
/** Project independently bound upload fields into detached normal and Cube execution candidates. */
import { isRecord, type UnknownRecord } from '../../types/common.js';
import type { ComfyInputLoaders } from './ComfyInputLoaders.js';
import { MASK_BINDING_KEY, IMAGE_BINDING_KEY, type InputBinding } from '../domain/InputBinding.js';
import type { InputLoader, InputPreviewSettings } from '../domain/InputLoader.js';
import { inputLoaderSettings } from '../domain/InputLoaderSettings.js';
import { readInputBinding, withInputBinding } from './InputBindingMetadata.js';
/** Identify a field independently from its node's compiled prompt path. */
export interface InputWorkflowTarget {
  readonly key: string;
  readonly nodePath: string;
  readonly settings: InputPreviewSettings;
  readonly unresolvedSettings: readonly string[];
  readonly maskOutput: boolean;
  readonly binding: InputBinding;
  readonly loader: InputLoader;
  apply(files: readonly string[]): void;
}
/** Freeze authored sources before asynchronous capture without changing graph topology or native loader settings. */
export async function inputWorkflowProjection(
  payload: unknown,
  loaders: ComfyInputLoaders,
  resolve: (
    properties: unknown,
    loader: InputLoader,
    files: readonly string[],
  ) => Promise<InputBinding | null>,
): Promise<{ payload: unknown; targets: readonly InputWorkflowTarget[] }> {
  if (!isRecord(payload) || !isRecord(payload.workflow)) return { payload, targets: [] };
  const copy: unknown = JSON.parse(JSON.stringify(payload));
  if (!isRecord(copy) || !isRecord(copy.workflow)) return { payload, targets: [] };
  const workflow = copy.workflow,
    output = isRecord(copy.output) ? copy.output : null;
  const definitions =
    isRecord(workflow.definitions) && Array.isArray(workflow.definitions.subgraphs)
      ? workflow.definitions.subgraphs
      : [];
  const byId = new Map(definitions.filter(isRecord).map((entry) => [entry.id, entry]));
  const targets: InputWorkflowTarget[] = [];
  const assetPorts = loaders.assetPorts();
  /** Follow exact authored instances and native subgraph identities without global numeric-ID matching. */
  async function visit(
    graph: UnknownRecord,
    prefix: readonly string[],
    ancestors: ReadonlySet<UnknownRecord>,
  ): Promise<void> {
    if (!Array.isArray(graph.nodes) || prefix.length > 32 || ancestors.has(graph)) return;
    for (const node of graph.nodes.filter(isRecord)) {
      if (typeof node.id !== 'string' && typeof node.id !== 'number') continue;
      const path = [...prefix, String(node.id)],
        key = path.join(':');
      const nested = isRecord(node.subgraph) ? node.subgraph : byId.get(node.type);
      if (nested) await visit(nested, path, new Set([...ancestors, graph]));
      const fields = loaders.all(typeof node.type === 'string' ? node.type : '');
      const properties = isRecord(node.properties) ? node.properties : {};
      if (!fields.length) {
        if (
          Object.hasOwn(properties, MASK_BINDING_KEY) ||
          Object.hasOwn(properties, IMAGE_BINDING_KEY)
        )
          throw new Error('The bound source loader definition changed.');
        continue;
      }
      const savedImages = properties[IMAGE_BINDING_KEY];
      if (
        isRecord(savedImages) &&
        isRecord(savedImages.fields) &&
        Object.keys(savedImages.fields).some(
          (field) => !fields.some((loader) => loader.kind === 'image' && loader.field === field),
        )
      )
        throw new Error('The bound image source field changed.');
      const compiled = output?.[key];
      if (output && !isRecord(compiled)) continue;
      for (const loader of fields) {
        if (isRecord(compiled) && compiled.class_type !== loader.classType)
          throw new Error('The compiled source loader identity changed.');
        const inputs = isRecord(compiled) && isRecord(compiled.inputs) ? compiled.inputs : null;
        const existing = inputs?.[loader.field];
        if (
          Array.isArray(existing) &&
          existing.length === 2 &&
          typeof existing[0] === 'string' &&
          typeof existing[1] === 'number'
        ) {
          if (
            readInputBinding(properties, loader.kind, loader.field) ||
            (loader.kind === 'mask' && Object.hasOwn(properties, MASK_BINDING_KEY))
          )
            throw new Error(
              'A connected source file field cannot be replaced by a canvas binding.',
            );
          continue;
        }
        const values = isRecord(node.sugarcubes_widget_values) ? node.sugarcubes_widget_values : {};
        const authored = values[loader.field];
        const files =
          typeof authored === 'string'
            ? authored
              ? [authored]
              : []
            : Array.isArray(authored)
              ? authored
              : null;
        const binding = await resolve(
          node.properties,
          loader,
          files?.filter((file): file is string => typeof file === 'string') ?? [],
        );
        if (!binding) continue;
        if (loader.field !== binding.field)
          throw new Error('The bound source loader definition changed.');
        if (files !== null && JSON.stringify(files) !== JSON.stringify(binding.files)) continue;
        targets.push({
          key: JSON.stringify([key, loader.field]),
          nodePath: key,
          settings: inputLoaderSettings(loader, { ...values, ...(inputs ?? {}) }),
          maskOutput:
            loader.kind === 'image' &&
            (assetPorts.get(loader.classType)?.outputs ?? []).some((kind, slot) => {
              if (kind !== 'mask') return false;
              if (output)
                return Object.values(output).some(
                  (value) =>
                    isRecord(value) &&
                    isRecord(value.inputs) &&
                    Object.values(value.inputs).some(
                      (input) =>
                        Array.isArray(input) &&
                        input.length === 2 &&
                        input[0] === key &&
                        input[1] === slot,
                    ),
                );
              const socket = Array.isArray(node.outputs) ? node.outputs[slot] : null;
              return isRecord(socket) && Array.isArray(socket.links) && socket.links.length > 0;
            }),
          unresolvedSettings: Object.freeze(
            inputs
              ? Object.entries(inputs)
                  .filter(
                    ([name, value]) =>
                      name !== loader.field &&
                      Array.isArray(value) &&
                      value.length === 2 &&
                      typeof value[0] === 'string' &&
                      typeof value[1] === 'number',
                  )
                  .map(([name]) => name)
              : Array.isArray(node.inputs)
                ? node.inputs
                    .filter(isRecord)
                    .filter(
                      (input) =>
                        input.name !== loader.field &&
                        input.link !== null &&
                        input.link !== undefined,
                    )
                    .map((input) => String(input.name))
                : [],
          ),
          binding,
          loader,
          apply: (files) => {
            if (!loader.ordered && files.length !== 1)
              throw new Error('A scalar source loader requires exactly one input.');
            const value = loader.ordered ? [...files] : files[0];
            const current = isRecord(node.sugarcubes_widget_values)
              ? node.sugarcubes_widget_values
              : {};
            node.sugarcubes_widget_values = { ...current, [loader.field]: value };
            node.properties = withInputBinding(node.properties, loader.kind, binding);
            // Comfy interprets bare lists as graph links, including native multiselect filenames.
            if (inputs) inputs[loader.field] = loader.ordered ? { __value__: value } : value;
          },
        });
      }
    }
  }
  await visit(workflow, [], new Set());
  return { payload: copy, targets };
}
