import {
  prepareTagCompletionCatalog
} from "./chunk-HOER6HVA.js";

// frontend/comfyui/ui/prompt_editor/runtime/PromptTagCatalog.ts
function bundledPromptTags(source) {
  const bundled = [];
  for (const [index, line] of source.split(/\r?\n/u).entries()) {
    if (!line.trim()) continue;
    const row = /^([^,]+),(\d+)(?:\s|$)/u.exec(line);
    const tag = row?.[1];
    const popularity = Number(row?.[2]);
    if (!tag || !Number.isSafeInteger(popularity) || popularity < 0)
      throw new TypeError(`Invalid bundled prompt tag at line ${String(index + 1)}.`);
    bundled.push({ tag, popularity });
  }
  if (!bundled.length) throw new TypeError("The bundled prompt tag catalog is empty.");
  return Object.freeze(bundled);
}
function preparePromptTagCatalog(bundled, revision, customTags, censoredTags) {
  return prepareTagCompletionCatalog({
    revision,
    bundled: [...bundled],
    customTags: [...customTags],
    censoredTags: [...censoredTags]
  });
}

export {
  bundledPromptTags,
  preparePromptTagCatalog
};
