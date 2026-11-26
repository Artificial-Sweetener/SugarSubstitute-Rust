import {
  PROMPT_LIBRARY_REVISION
} from "./chunk-UGQLUC67.js";
import {
  prepareLoraSchedulingContext
} from "./chunk-HOER6HVA.js";

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
  /** Publish when the upstream models, or what the library knows of them, changed. */
  publish() {
    const upstream = this.upstream();
    const key = JSON.stringify(
      upstream.map((item) => [item.model.backendValue, item.title, item.triggerWords])
    );
    if (key === this.#key) return;
    this.#key = key;
    this.api.setLoraSchedulingHost({
      context: prepareLoraSchedulingContext({
        revision: `${PROMPT_LIBRARY_REVISION}#${String(++this.#revision)}`,
        catalogRevision: PROMPT_LIBRARY_REVISION,
        mode: "workflow",
        supported: this.supported,
        upstream,
        fields: []
      }),
      catalogRevision: PROMPT_LIBRARY_REVISION
    });
  }
};

export {
  PromptLoraScheduling
};
