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
/** Decide whether a prompt's text reaches an encoder that loads the LoRAs written in it. */
import { buildLinkIndex } from '../graph/GraphQuery.js';
import type { ComfyNode } from '../types/graph.js';

/** Prompt Control encoders read `<lora:…>` from their text and load those models themselves. */
const ENCODER_CLASS_PREFIXES = ['PCLazyTextEncode', 'PCTextEncode'] as const;
const ENCODER_CLASSES = new Set(['SimpleSyrup.ScheduleAndEncodePromptsWithPromptControl']);

/**
 * Report whether text leaving this node reaches a Prompt Control encoder in the node's own graph.
 *
 * LoRA names are offered only where writing one has an effect. Text may pass through other nodes
 * on the way, so every node downstream of the prompt is followed.
 */
export function feedsPromptControl(node: ComfyNode): boolean {
  const graph = node.graph;
  if (!graph) return false;
  const nodes = new Map(
    (graph._nodes ?? graph.nodes ?? []).map((member) => [String(member.id), member]),
  );
  const { outgoing } = buildLinkIndex(graph);
  const pending = [String(node.id)];
  const visited = new Set(pending);
  for (let current = pending.pop(); current !== undefined; current = pending.pop())
    for (const link of outgoing.get(current) ?? []) {
      const targetId = link.target_id ?? link.target;
      if (targetId == null) continue;
      const key = String(targetId);
      if (visited.has(key)) continue;
      visited.add(key);
      const target = nodes.get(key);
      if (!target) continue;
      if (isPromptControlEncoder(target)) return true;
      pending.push(key);
    }
  return false;
}

/** Match the encoder by its registered class, never by a title a person can rename. */
function isPromptControlEncoder(node: ComfyNode): boolean {
  const type = (node.class_type ?? node.type ?? '').trim();
  return (
    ENCODER_CLASSES.has(type) || ENCODER_CLASS_PREFIXES.some((prefix) => type.startsWith(prefix))
  );
}
