import {
  applyRegionNames,
  changedRegionNames,
  firstRegionNames,
  parsePrompt,
  strip,
  validRegionName
} from "../chunk-S7BQTISC.js";

// frontend/sugarprompt/services/src/wildcards/catalog.ts
var noColumns = Object.freeze([]);
var noMatches = Object.freeze({
  status: "ready",
  suggestions: Object.freeze([])
});

// frontend/sugarprompt/services/src/completion/catalog.ts
var empty = Object.freeze([]);

// frontend/sugarprompt/services/src/completion/scene-catalog.ts
var empty2 = Object.freeze([]);

// frontend/sugarprompt/services/src/features/registry.ts
function definition(id, label, aliases, syntax = null, dependencies = [], legacyBase = false) {
  return Object.freeze({
    id,
    label,
    aliases: Object.freeze(aliases),
    syntax,
    dependencies: Object.freeze(dependencies),
    conflicts: Object.freeze([]),
    legacyBase
  });
}
var featureDefinitions = Object.freeze([
  definition("emphasis", "Emphasis weights", ["emphasis"], "emphasis"),
  definition("danbooru_url_import", "Danbooru URL import", ["danbooru_url_import"], null, [], true),
  definition("wildcard_syntax", "Wildcard syntax", ["wildcard", "wildcard_syntax"], "wildcard"),
  definition(
    "wildcard_autocomplete",
    "Wildcard autocomplete",
    ["wildcard", "wildcard_autocomplete"],
    null,
    ["wildcard_syntax"]
  ),
  definition("autocomplete_ghost_text", "Autocomplete ghost text", [], null, [], true),
  definition("lora_syntax", "LoRA syntax", ["lora", "lora_syntax"], "lora"),
  definition("lora_autocomplete", "LoRA autocomplete", ["lora", "lora_autocomplete"], null, [
    "lora_syntax"
  ]),
  definition("lora_picker", "LoRA picker", ["lora", "lora_picker"], null, ["lora_syntax"]),
  definition("lora_trigger_words", "LoRA trigger words", ["lora", "lora_trigger_words"]),
  definition("segment_reorder", "Segment reorder", ["segment_reorder"], null, [], true),
  definition("spellcheck", "Spellcheck", ["spellcheck"], null, [], true),
  definition(
    "duplicate_segment_diagnostics",
    "Duplicate segment warnings",
    ["duplicate_segment_diagnostics"],
    null,
    [],
    true
  )
]);

// frontend/sugarprompt/services/src/lora/metadata-validation.ts
var loraItemKeys = Object.freeze([
  "model",
  "title",
  "subtitle",
  "triggerWords",
  "thumbnail",
  "modelPageUrl"
]);

// frontend/sugarprompt/services/src/features/preferences.ts
var defaultPreferences = Object.freeze({
  allowed: Object.freeze(
    Object.fromEntries(featureDefinitions.map(({ id }) => [id, true]))
  ),
  wheelMode: "hover_dwell",
  resolveOnGeneration: true
});

// frontend/sugarprompt/services/src/regions/context.ts
var unresolvedConditioning = Object.freeze({
  key: null,
  mode: "unresolved"
});

// frontend/sugarprompt/services/src/regions/names.ts
function captureRegionNames(value) {
  if (!Array.isArray(value)) throw new TypeError("Invalid shared region names");
  const seen = /* @__PURE__ */ new Set();
  return Object.freeze(
    value.map((item) => {
      if (!item || typeof item !== "object" || Object.keys(item).sort().join() !== "name,ordinal" || !("ordinal" in item) || typeof item.ordinal !== "number" || !Number.isSafeInteger(item.ordinal) || item.ordinal < 0 || seen.has(item.ordinal) || !("name" in item) || typeof item.name !== "string" || !validRegionName(item.name))
        throw new TypeError("Invalid shared region identity");
      seen.add(item.ordinal);
      return Object.freeze({ ordinal: item.ordinal, name: strip(item.name) });
    })
  );
}
function resolveSharedRegionNames(documents, changes = []) {
  const names = new Map(firstRegionNames(documents).map((value) => [value.ordinal, value]));
  for (const value of captureRegionNames(changes)) names.set(value.ordinal, value);
  return Object.freeze(Array.from(names.values()));
}

// frontend/comfyui/ui/input_canvas/domain/RegionalPromptNames.ts
function synchronizeRegionalPromptNames(prompts) {
  const documents = prompts.map((prompt) => parsePrompt(prompt.text));
  const changes = /* @__PURE__ */ new Map();
  for (const [index, prompt] of prompts.entries()) {
    const document = documents[index];
    if (prompt.previous === null || !document) continue;
    for (const change of changedRegionNames(parsePrompt(prompt.previous), document))
      changes.set(change.ordinal, change);
  }
  const names = resolveSharedRegionNames(documents, [...changes.values()]);
  return documents.map((document) => {
    const application = applyRegionNames(document, names);
    return application.replacements.length ? application.source : null;
  });
}
export {
  synchronizeRegionalPromptNames
};
