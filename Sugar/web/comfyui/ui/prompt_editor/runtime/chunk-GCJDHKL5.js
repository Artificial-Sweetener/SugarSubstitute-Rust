import {
  SpellcheckSession
} from "./chunk-HOER6HVA.js";

// frontend/comfyui/ui/prompt_editor/runtime/PromptSpellcheckSessions.ts
var sessions = /* @__PURE__ */ new WeakMap();
function spellcheckSessionFor(gateway) {
  if (!gateway) return null;
  let session = sessions.get(gateway);
  if (!session) {
    session = new SpellcheckSession(gateway);
    sessions.set(gateway, session);
  }
  return session;
}

export {
  spellcheckSessionFor
};
