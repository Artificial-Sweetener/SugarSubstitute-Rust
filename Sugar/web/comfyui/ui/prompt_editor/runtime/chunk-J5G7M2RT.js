import {
  prepareLoraSchedulingContext
} from "./chunk-HVJO4BPJ.js";

// frontend/comfyui/ui/prompt_editor/runtime/PromptLoraScheduling.ts
var PromptLoraScheduling = class {
  /** Bind the editor, whether its prompt loads written LoRAs, and the host's upstream lookup. */
  constructor(api, supported, upstream) {
    this.api = api;
    this.supported = supported;
    this.upstream = upstream;
  }
  #key = "";
  #revision = 0;
  /** Forget what was published, so the next catalog is always published. */
  reset() {
    this.#key = "";
  }
  /** Publish for this catalog when it or the upstream models changed. */
  publish(adapter) {
    const upstream = this.upstream();
    const key = [adapter.revision, ...upstream.map((item) => item.model.backendValue)].join("\n");
    if (key === this.#key) return;
    this.#key = key;
    this.api.setLoraSchedulingHost({
      context: prepareLoraSchedulingContext({
        revision: `${adapter.revision}#${String(++this.#revision)}`,
        catalogRevision: adapter.revision,
        mode: "workflow",
        supported: this.supported,
        upstream,
        fields: []
      }),
      catalogRevision: adapter.revision
    });
  }
};

export {
  PromptLoraScheduling
};
