// frontend/comfyui/ui/prompt_editor/runtime/ComfyPromptInput.ts
function markComfyPromptInput(root) {
  const content = root.querySelector(".cm-content");
  if (!content || Reflect.get(content, "type") === "textarea") return;
  Object.defineProperty(content, "type", { configurable: true, value: "textarea" });
}

export {
  markComfyPromptInput
};
