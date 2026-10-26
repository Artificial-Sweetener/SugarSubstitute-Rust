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
/** Resolve conservative loader-field relationships from normalized typed graph inputs. */
import type { InputKind } from './InputLoader.js';

/** Identify a real upload field independently of native nodes, editor APIs and UI surfaces. */
export interface InputAssetEndpoint {
  readonly key: string;
  readonly kind: InputKind;
  /** Identify image fields whose file also supplies a connected native MASK output. */
  readonly maskOutput?: boolean;
}
/** Preserve every connected spatial input, including providers with no recognized loader field. */
export interface InputAssetConsumerInput {
  readonly consumer: string;
  readonly input: string;
  readonly kind: InputKind;
  readonly endpoints: readonly string[];
}
/** Supply one graph scope after native boundaries have been resolved by its adapter. */
export interface InputAssetAssociationGraph {
  readonly endpoints: readonly InputAssetEndpoint[];
  readonly inputs: readonly InputAssetConsumerInput[];
}
/** Distinguish safe relationships from absent or competing image contexts. */
export type InputAssetAssociation =
  | {
      readonly kind: 'associated';
      readonly mask: string;
      readonly image: string;
      readonly consumers: readonly string[];
    }
  | {
      readonly kind: 'missing' | 'ambiguous';
      readonly mask: string;
      readonly candidates: readonly string[];
      readonly consumers: readonly string[];
    };

/** Associate masks only when every image-bearing consumer resolves to one compatible image field. */
export function resolveInputAssetAssociations(
  graphs: readonly InputAssetAssociationGraph[],
): ReadonlyMap<string, InputAssetAssociation> {
  const endpoints = new Map(
    graphs.flatMap((graph) => graph.endpoints.map((endpoint) => [endpoint.key, endpoint] as const)),
  );
  const consumers = new Map<string, InputAssetConsumerInput[]>();
  for (const graph of graphs)
    for (const input of graph.inputs) {
      const inputs = consumers.get(input.consumer) ?? [];
      if (!inputs.some((previous) => previous.input === input.input)) inputs.push(input);
      consumers.set(input.consumer, inputs);
    }
  const masks = new Map<
    string,
    { images: Set<string>; consumers: Set<string>; unresolved: boolean; ambiguous: boolean }
  >();
  for (const endpoint of endpoints.values())
    if (endpoint.kind === 'mask')
      masks.set(endpoint.key, {
        images: new Set(),
        consumers: new Set(),
        unresolved: false,
        ambiguous: false,
      });
  for (const [consumer, inputs] of consumers) {
    const images = inputs.filter((input) => input.kind === 'image');
    if (!images.length) continue;
    const candidates = [
      ...new Set(
        images
          .flatMap((input) => input.endpoints)
          .filter((key) => endpoints.get(key)?.kind === 'image'),
      ),
    ];
    for (const input of inputs.filter((input) => input.kind === 'mask'))
      for (const key of input.endpoints) {
        const mask = masks.get(key);
        if (!mask) continue;
        mask.consumers.add(consumer);
        for (const candidate of candidates) mask.images.add(candidate);
        mask.ambiguous ||= images.length > 1 || candidates.length > 1;
        mask.unresolved ||= candidates.length !== 1;
      }
  }
  return new Map(
    [...masks].map(([mask, facts]) => {
      const candidates = [...facts.images].sort(),
        consumers = [...facts.consumers].sort();
      const image = candidates[0];
      const association: InputAssetAssociation =
        image && candidates.length === 1 && !facts.ambiguous && !facts.unresolved
          ? { kind: 'associated', mask, image, consumers }
          : {
              kind:
                facts.ambiguous ||
                candidates.length > 1 ||
                (facts.unresolved && candidates.length > 0)
                  ? 'ambiguous'
                  : 'missing',
              mask,
              candidates,
              consumers,
            };
      return [mask, Object.freeze(association)];
    }),
  );
}

/** Admit editing only when no other mask can target the same image input. */
export function exclusiveInputImage(
  associations: ReadonlyMap<string, InputAssetAssociation>,
  mask: string,
  maskSources: ReadonlySet<string>,
): string | null {
  const pair = associations.get(mask);
  if (pair?.kind !== 'associated' || maskSources.has(pair.image)) return null;
  for (const other of associations.values()) {
    if (other.mask === mask) continue;
    if (
      other.kind === 'associated'
        ? other.image === pair.image
        : other.candidates.includes(pair.image)
    )
      return null;
  }
  return pair.image;
}
