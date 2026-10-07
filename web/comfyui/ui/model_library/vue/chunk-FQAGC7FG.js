import {
  useModelAction
} from "./chunk-MM7W35BE.js";
import {
  ModelContextMenuView,
  projectModelContext
} from "./chunk-WIKCJTWX.js";
import {
  h,
  onBeforeUnmount,
  ref,
  shallowRef
} from "./chunk-3W5PGMRW.js";

// frontend/comfyui/ui/model_library/ModelContextMenu.ts
function modelProviderPage(value) {
  try {
    const url = new URL(value);
    return url.protocol === "https:" && ["civitai.com", "huggingface.co", "openmodeldb.info"].includes(url.hostname) && !url.username && !url.password && (!url.port || url.port === "443") ? url.href : null;
  } catch {
    return null;
  }
}
function canBrowseModelVersions(item, hasUpdate = false) {
  if (item.versionCount != null && item.versionCount < 2) return false;
  return ["checkpoints", "diffusion_models", "loras"].includes(item.kind) && /^[a-f0-9]{64}$/.test(item.id) && (hasUpdate || item.provider === "civitai" && /^[1-9][0-9]{0,15}$/.test(item.providerModelId) && /^[1-9][0-9]{0,15}$/.test(item.providerVersionId) && !!item.baseModel.trim());
}
function modelContextEntries(item, canManage, hasUpdate, hasOutput) {
  const entries = [];
  const detector = item.kind === "ultralytics";
  if (detector && canManage)
    entries.push({
      label: "Choose detector thumbnail",
      enabled: true,
      command: "detector-thumbnail",
      url: null
    });
  const links = item.providerLinks.length ? item.providerLinks : [
    {
      providerName: item.provider === "civitai" ? "CivitAI" : item.provider === "openmodeldb" ? "OpenModelDB" : "Hugging Face",
      modelPageUrl: item.providerUrl
    }
  ];
  for (const link of links) {
    const page = modelProviderPage(link.modelPageUrl);
    if (page)
      entries.push({
        label: `Go to ${link.providerName} page`,
        enabled: true,
        command: null,
        url: page
      });
  }
  const canEditMetadata = canManage && !detector && !!item.value && !!item.kind;
  if (canEditMetadata)
    entries.push({
      label: "Refresh CivitAI metadata",
      enabled: true,
      command: "refresh",
      url: null
    });
  if (canManage && canBrowseModelVersions(item, hasUpdate))
    entries.push({ label: "See other versions", enabled: true, command: "versions", url: null });
  if (canEditMetadata)
    entries.push({
      label: "Set thumbnail from canvas",
      enabled: hasOutput,
      command: "output-thumbnail",
      url: null
    });
  return entries;
}

// frontend/comfyui/ui/model_library/vue/ModelContextState.ts
import { ModelDetectorDialog } from "../ModelDetectorDialog.js";

// frontend/comfyui/ui/model_library/ModelOutputThumbnailMenu.ts
function compareSceneText(left, right) {
  const leftPoints = Array.from(left, (character) => character.codePointAt(0) ?? 0);
  const rightPoints = Array.from(right, (character) => character.codePointAt(0) ?? 0);
  for (let index = 0; index < Math.max(leftPoints.length, rightPoints.length); index++) {
    const difference = (leftPoints[index] ?? -1) - (rightPoints[index] ?? -1);
    if (difference) return difference;
  }
  return 0;
}
function group(label, choices, children) {
  const only = choices.length === 1 ? choices[0] : void 0;
  return only ? { label, enabled: true, command: "output-thumbnail", url: null, imageId: only.id } : { label, enabled: true, command: null, url: null, children: children() };
}
function sources(choices) {
  return [...new Set(choices.map((choice) => choice.sourceKey))].map((key) => {
    const selected = choices.filter((choice) => choice.sourceKey === key);
    return group(
      selected[0]?.sourceLabel || "Output",
      selected,
      () => selected.map((choice, index) => ({
        label: `Image ${String(index + 1)}`,
        enabled: true,
        command: "output-thumbnail",
        url: null,
        imageId: choice.id
      }))
    );
  });
}
function batches(choices) {
  const indices = [...new Set(choices.map((choice) => choice.batch))].sort((a, b) => a - b);
  return indices.length <= 1 ? sources(choices) : indices.map((index) => {
    const selected = choices.filter((choice) => choice.batch === index);
    return group(`Batch ${String(index)}`, selected, () => sources(selected));
  });
}
function modelOutputThumbnailMenu(choices, active) {
  const label = "Set thumbnail from canvas";
  if (!choices.length) return { label, enabled: false, command: "output-thumbnail", url: null };
  return group(label, choices, () => {
    const children = [];
    if (active && choices.some((choice) => choice.id === active.id)) {
      children.push({
        label: "Current image",
        enabled: true,
        command: "output-thumbnail",
        url: null,
        imageId: active.id
      });
      children.push({ label: "", enabled: false, command: null, url: null, separator: true });
    }
    const keys = new Set(
      choices.filter((choice) => choice.sceneKey).map((choice) => choice.sceneKey)
    );
    const meaningful = keys.size > 1 || keys.size > 0 && choices.some((choice) => choice.sceneTitle && choice.sceneTitle !== "Scene");
    if (!meaningful) return [...children, ...batches(choices)];
    const sorted = [...choices].sort(
      (a, b) => a.sceneOrder - b.sceneOrder || compareSceneText(a.sceneTitle, b.sceneTitle) || compareSceneText(a.sceneKey, b.sceneKey)
    );
    for (const key of new Set(sorted.map((choice) => choice.sceneKey))) {
      const selected = sorted.filter((choice) => choice.sceneKey === key);
      children.push(group(selected[0]?.sceneTitle || "Scene", selected, () => batches(selected)));
    }
    return children;
  });
}

// frontend/comfyui/ui/model_library/vue/ModelContextState.ts
function useModelContext(options, disabled, beforeFamily, session) {
  const menu = ref(
    null
  );
  const updates = shallowRef(options.updates?.read().proposals ?? []);
  const unsubscribe = options.updates?.subscribe((snapshot) => {
    updates.value = snapshot.proposals;
  });
  const state = useModelAction(disabled);
  let dialog = null;
  let active = true;
  const offer = (item) => updates.value.find((proposal) => proposal.modelId === item.id);
  const entries = (item, updateOnly = false) => {
    const proposal = offer(item);
    return updateOnly && proposal ? [
      { label: "Dismiss", enabled: true, command: "dismiss", url: null },
      {
        label: `Don't check for updates for ${proposal.modelName}`,
        enabled: true,
        command: "disable-updates",
        url: null
      }
    ] : modelContextEntries(
      item,
      item.kind === "ultralytics" ? !!options.detectors : !!options.actions && /^[a-f0-9]{64}$/.test(item.id),
      !!proposal,
      false
    ).map(
      (entry) => entry.command === "output-thumbnail" && options.outputs ? modelOutputThumbnailMenu(options.outputs.choices(), options.outputs.activeChoice()) : entry
    );
  };
  const show = (item, event, updateOnly = false) => {
    event.preventDefault();
    event.stopPropagation();
    if (!entries(item, updateOnly).length) return;
    menu.value = { item, x: event.clientX, y: event.clientY, updateOnly };
  };
  const family = (item) => {
    if (!options.actions || !canBrowseModelVersions(item, !!offer(item))) return;
    beforeFamily();
    menu.value = null;
    dialog?.close();
    void session.versions(item, options.actions, (severity, message) => options.feedback?.(severity, message)).finally(() => {
      void options.updates?.load().catch((error) => {
        options.feedback?.(
          "error",
          error instanceof Error ? error.message : "Model updates are unavailable."
        );
      });
    });
  };
  const command = (item, entry) => {
    if (entry.command === "detector-thumbnail" && options.detectors) {
      beforeFamily();
      menu.value = null;
      dialog?.close();
      const current = new ModelDetectorDialog(document, item, options.detectors);
      dialog = current;
      void current.open().finally(() => {
        if (dialog === current) dialog = null;
      });
      return;
    }
    if (entry.command === "versions") {
      family(item);
      return;
    }
    void state.run(async (signal) => {
      if (entry.command === "refresh")
        await options.actions?.refreshMetadata(
          item.id,
          (message) => {
            state.status.value = message;
          },
          signal
        );
      else if (entry.command === "output-thumbnail" && entry.imageId && options.outputs && options.actions) {
        const image = await options.outputs.image(entry.imageId, signal);
        if (signal.aborted) return;
        await options.actions.replaceThumbnail(item.id, image, signal);
        if (!signal.aborted) state.status.value = "Model thumbnail updated.";
      } else if (entry.command === "dismiss") await options.updates?.dismiss(item.id);
      else if (entry.command === "disable-updates") await options.updates?.disableModel(item.id);
    }).then(() => {
      if (!active) return;
      if (state.error.value) options.feedback?.("error", state.error.value);
      else if (state.status.value) options.feedback?.("info", state.status.value);
    });
  };
  onBeforeUnmount(() => {
    active = false;
    menu.value = null;
    unsubscribe?.();
    dialog?.close();
  });
  const render = () => {
    const current = menu.value;
    return current ? h(ModelContextMenuView, {
      entries: entries(current.item, current.updateOnly),
      x: current.x,
      y: current.y,
      onClose: () => {
        menu.value = null;
      },
      onAction: (entry) => command(current.item, entry)
    }) : null;
  };
  return {
    menu,
    show,
    family,
    offer,
    render,
    proposals: updates,
    /** Capture the selected model without creating another metadata or command owner. */
    project: (item) => projectModelContext(entries(item), (entry) => command(item, entry))
  };
}

export {
  modelProviderPage,
  useModelContext
};
