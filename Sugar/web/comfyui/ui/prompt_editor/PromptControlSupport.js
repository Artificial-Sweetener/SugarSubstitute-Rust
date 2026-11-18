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
import { promptLoraItem } from './PromptLoraCatalog.js';
/** Prompt Control encoders read `<lora:…>` from their text and load those models themselves. */
const ENCODER_CLASS_PREFIXES = ['PCLazyTextEncode', 'PCTextEncode'];
const ENCODER_CLASSES = new Set(['SimpleSyrup.ScheduleAndEncodePromptsWithPromptControl']);
/**
 * Report whether text leaving this node reaches a Prompt Control encoder in the node's own graph.
 *
 * LoRA names are offered only where writing one has an effect. Text may pass through other nodes
 * on the way, so every node downstream of the prompt is followed.
 */
export function feedsPromptControl(node) {
    return promptControlEncoders(node).length > 0;
}
/**
 * The text values configured on every node that feeds the encoders this prompt reaches.
 *
 * A LoRA loaded ahead of the encoder shapes the same conditioning as one written in the prompt.
 * The caller decides which of these values name installed LoRAs; the prompt's own node and the
 * nodes between it and the encoder are not sources of models and are left out.
 */
export function promptUpstreamValues(node) {
    const graph = node.graph;
    const encoders = promptControlEncoders(node);
    if (!graph || !encoders.length)
        return [];
    const nodes = graphNodes(node);
    const { incoming } = buildLinkIndex(graph);
    const between = textPath(node);
    const pending = encoders.map((encoder) => String(encoder.id));
    const visited = new Set(pending);
    const values = [];
    for (let current = pending.pop(); current !== undefined; current = pending.pop())
        for (const link of incoming.get(current) ?? []) {
            const originId = link.origin_id ?? link.origin;
            if (originId == null)
                continue;
            const key = String(originId);
            if (visited.has(key) || between.has(key))
                continue;
            visited.add(key);
            const origin = nodes.get(key);
            if (!origin)
                continue;
            for (const widget of origin.widgets ?? [])
                if (typeof widget.value === 'string' && widget.value)
                    values.push(widget.value);
            pending.push(key);
        }
    return values;
}
/** The installed LoRAs loaded ahead of this prompt's encoder, as the catalog describes them. */
export function promptUpstreamLoras(node, catalog) {
    if (!catalog)
        return [];
    return promptUpstreamValues(node).flatMap((value) => {
        const item = promptLoraItem(catalog, value);
        return item ? [item] : [];
    });
}
/** The Prompt Control encoders reached by text leaving this node, in discovery order. */
function promptControlEncoders(node) {
    const nodes = graphNodes(node);
    const encoders = [];
    for (const key of textPath(node)) {
        const member = nodes.get(key);
        if (member && member !== node && isPromptControlEncoder(member))
            encoders.push(member);
    }
    return encoders;
}
/** The prompt's node and every node its text passes through, stopping at encoders. */
function textPath(node) {
    const visited = new Set([String(node.id)]);
    const graph = node.graph;
    if (!graph)
        return visited;
    const nodes = graphNodes(node);
    const { outgoing } = buildLinkIndex(graph);
    const pending = [String(node.id)];
    for (let current = pending.pop(); current !== undefined; current = pending.pop())
        for (const link of outgoing.get(current) ?? []) {
            const targetId = link.target_id ?? link.target;
            if (targetId == null)
                continue;
            const key = String(targetId);
            if (visited.has(key))
                continue;
            visited.add(key);
            const target = nodes.get(key);
            if (target && !isPromptControlEncoder(target))
                pending.push(key);
        }
    return visited;
}
/** Index the members of the node's own graph by identity. */
function graphNodes(node) {
    const graph = node.graph;
    return new Map((graph?._nodes ?? graph?.nodes ?? []).map((member) => [String(member.id), member]));
}
/** Match the encoder by its registered class, never by a title a person can rename. */
function isPromptControlEncoder(node) {
    const type = (node.class_type ?? node.type ?? '').trim();
    return (ENCODER_CLASSES.has(type) || ENCODER_CLASS_PREFIXES.some((prefix) => type.startsWith(prefix)));
}
