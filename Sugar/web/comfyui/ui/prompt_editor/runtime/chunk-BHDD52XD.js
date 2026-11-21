// frontend/comfyui/ui/prompt_editor/runtime/ClipboardRefusal.ts
function clipboardRefusalObserver(refused, report) {
  return {
    observe: (outcome) => {
      if (outcome.status === "denied" || outcome.status === "unavailable") refused();
    },
    error: report
  };
}

export {
  clipboardRefusalObserver
};
