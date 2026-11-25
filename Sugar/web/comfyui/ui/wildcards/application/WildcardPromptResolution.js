import {
  prepareWildcardGeneration
} from "../chunk-AO5UJLCO.js";
import "../chunk-BJZB62WU.js";

// frontend/comfyui/ui/wildcards/application/WildcardPromptResolution.ts
var ROUNDS = 25;
async function resolveWildcardTexts(texts, seed, entropy, library) {
  const sources = [...new Set(texts)].filter((text) => text.includes("{"));
  if (!sources.length) return /* @__PURE__ */ new Map();
  await library.refresh();
  const loaded = /* @__PURE__ */ new Map();
  const asked = /* @__PURE__ */ new Set();
  for (let round = 0; ; round++) {
    const preparation = prepareWildcardGeneration({
      revision: library.revision,
      sources: [...loaded.values()],
      seed,
      ...seed === null ? { entropySeed: entropy } : {}
    });
    if (preparation.status !== "ready") throw new Error("Wildcard candidates could not be read.");
    const resolved = /* @__PURE__ */ new Map();
    const wanted = /* @__PURE__ */ new Map();
    try {
      for (const source of sources) {
        const outcome = preparation.pass.resolve(source);
        if (outcome.status === "rejected" || outcome.status === "disabled") continue;
        if (outcome.resolvedText !== source) resolved.set(source, outcome.resolvedText);
        for (const issue of outcome.issues) {
          const name = issue.form + ":" + issue.identifier;
          if (issue.reason === "source-missing" && !asked.has(name))
            wanted.set(name, { form: issue.form, identifier: issue.identifier });
        }
      }
    } finally {
      preparation.pass.dispose();
    }
    let added = false;
    if (round < ROUNDS)
      for (const [name, { form, identifier }] of wanted) {
        asked.add(name);
        const candidates = await library.candidates(form, identifier);
        const held = candidates ? form + ":" + candidates.identifier : null;
        if (!candidates || held === null || loaded.has(held)) continue;
        loaded.set(held, candidates);
        added = true;
      }
    if (!added) return resolved;
  }
}
export {
  resolveWildcardTexts
};
