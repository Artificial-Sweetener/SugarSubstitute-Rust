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
/** Rewire hardwired Cubes for a new chain order, or say why the order cannot be wired. */

/** One boundary port of a Cube. */
export interface RewirePort {
  readonly slot: number;
  readonly name: string;
  readonly type: unknown;
}

/** One Cube as the rewire planner reads it. */
export interface RewireCube {
  readonly instanceId: string;
  readonly title: string;
  readonly inputs: readonly RewirePort[];
  readonly outputs: readonly RewirePort[];
}

/** One real link from a Cube output to a Cube input. */
export interface RewireLink {
  readonly sourceInstance: string;
  readonly sourceSlot: number;
  readonly targetInstance: string;
  readonly targetSlot: number;
}

export type RewirePlan =
  | {
      readonly ok: true;
      readonly remove: readonly RewireLink[];
      readonly add: readonly RewireLink[];
    }
  | { readonly ok: false; readonly reason: string };

/**
 * Plan the links a chain order needs.
 *
 * A link from a Cube to the Cube right after it is a chain link: it follows the chain. When the
 * Cube before a hardwired Cube changes, its chain inputs are wired from its new neighbour,
 * choosing the output that fed it before by name, then by type, then the first that fits. Any
 * other link between Cubes stays as it is and must still run forwards. An order that leaves a
 * hardwired input with nothing that fits, or runs a link backwards, is refused.
 */
export function planRewire(
  cubes: readonly RewireCube[],
  links: readonly RewireLink[],
  current: readonly string[],
  requested: readonly string[],
  accepts: (outputType: unknown, inputType: unknown) => boolean,
): RewirePlan {
  const byId = new Map(cubes.map((cube) => [cube.instanceId, cube]));
  const before = predecessors(current);
  const after = predecessors(requested);
  const index = new Map(requested.map((id, position) => [id, position]));
  const chain = links.filter((link) => before.get(link.targetInstance) === link.sourceInstance);
  for (const link of links) {
    if (chain.includes(link)) continue;
    if ((index.get(link.sourceInstance) ?? -1) >= (index.get(link.targetInstance) ?? -1))
      return refuse(byId, link.targetInstance, link.sourceInstance, 'runs before');
  }
  const remove: RewireLink[] = [];
  const add: RewireLink[] = [];
  const rewired = new Set(
    chain
      .filter((link) => after.get(link.targetInstance) !== link.sourceInstance)
      .map((link) => link.targetInstance),
  );
  for (const target of rewired) {
    const inputs = chain.filter((link) => link.targetInstance === target);
    remove.push(...inputs);
    const source = after.get(target);
    if (source === undefined) return refuse(byId, target, null, 'first');
    const from = byId.get(source);
    const to = byId.get(target);
    if (!from || !to) return { ok: false, reason: 'A Cube in the stack is no longer available.' };
    for (const link of inputs) {
      const input = to.inputs.find((port) => port.slot === link.targetSlot);
      const previous = byId
        .get(link.sourceInstance)
        ?.outputs.find((port) => port.slot === link.sourceSlot);
      const output = input ? bestOutput(from.outputs, input, previous, accepts) : undefined;
      if (!input || !output) return refuse(byId, target, source, 'no fit');
      add.push({
        sourceInstance: source,
        sourceSlot: output.slot,
        targetInstance: target,
        targetSlot: input.slot,
      });
    }
  }
  return { ok: true, remove, add };
}

/** Map each Cube to the Cube right before it. */
function predecessors(order: readonly string[]): Map<string, string> {
  return new Map(order.slice(1).map((id, position) => [id, order[position] ?? '']));
}

/** Pick the output that fed this input before by name, then by type, then the first that fits. */
function bestOutput(
  outputs: readonly RewirePort[],
  input: RewirePort,
  previous: RewirePort | undefined,
  accepts: (outputType: unknown, inputType: unknown) => boolean,
): RewirePort | undefined {
  const fitting = outputs.filter((output) => accepts(output.type, input.type));
  return (
    fitting.find((output) => previous !== undefined && output.name === previous.name) ??
    fitting.find((output) => String(output.type) === String(input.type)) ??
    fitting[0]
  );
}

/** Explain a refused order in the maintainer's terms: which Cube and why. */
function refuse(
  cubes: ReadonlyMap<string, RewireCube>,
  target: string,
  source: string | null,
  kind: 'runs before' | 'first' | 'no fit',
): RewirePlan {
  const name = (id: string | null) => (id === null ? '' : (cubes.get(id)?.title ?? id));
  const reason =
    kind === 'first'
      ? `${name(target)} is wired to the Cube before it, so it cannot be first.`
      : kind === 'runs before'
        ? `${name(target)} is wired to ${name(source)}, which would then run after it.`
        : `${name(source)} has no output that fits ${name(target)}'s wired input.`;
  return { ok: false, reason };
}
