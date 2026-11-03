import {
  parsePrompt
} from "../chunk-S7BQTISC.js";

// frontend/comfyui/ui/input_canvas/infrastructure/ComfyRegionalPrompts.ts
import { regionalPromptDemand } from "../domain/RegionalPromptDemand.js";
var PROMPT_HOPS = 3;
var PROMPT_NODE = "PrimitiveStringMultiline";
function readRegionalPrompts(topology, mask, node) {
  const incoming = /* @__PURE__ */ new Map();
  for (const edge of topology.edges) {
    const sources = incoming.get(edge.target) ?? [];
    sources.push(edge.source);
    incoming.set(edge.target, sources);
  }
  const consumers = topology.edges.filter((edge) => edge.source === mask && edge.type === "MASK").map((edge) => edge.target);
  const prompts = [], seen = /* @__PURE__ */ new Set([mask]);
  let frontier = [...new Set(consumers)];
  for (let hop = 0; hop < PROMPT_HOPS && frontier.length; hop++) {
    const next = [];
    for (const target of frontier)
      for (const source of incoming.get(target) ?? []) {
        if (seen.has(source)) continue;
        seen.add(source);
        const live = node(source);
        if ((live?.type ?? live?.class_type) === PROMPT_NODE) {
          if (live) prompts.push({ path: source, node: live });
        } else next.push(source);
      }
    frontier = next;
  }
  return {
    demand: regionalPromptDemand(
      prompts.map(({ path, node: node2 }) => ({
        path,
        separators: parsePrompt(promptField(node2)?.value ?? "").regions.separators.map(
          (separator) => separator.name
        )
      }))
    ),
    nodes: prompts.map(({ node: node2 }) => node2)
  };
}
function promptField(node) {
  for (const name of ["value", "text"]) {
    const widget = node.widgets?.find((candidate) => candidate.name === name);
    if (widget && typeof widget.value === "string") return { widget, value: widget.value };
  }
  return null;
}
export {
  promptField,
  readRegionalPrompts
};
