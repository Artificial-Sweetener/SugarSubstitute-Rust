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

// frontend/comfyui/ui/model_library/ModelMenu.ts
var ModelMenu = class {
  /** Bind the owners, the session that applies a chosen version, and the caller's hooks. */
  constructor(options, session, disabled = () => false, beforeFamily = () => void 0) {
    this.options = options;
    this.session = session;
    this.disabled = disabled;
    this.beforeFamily = beforeFamily;
    this.#proposals = options.updates?.read().proposals ?? [];
    this.#unsubscribe = options.updates?.subscribe((snapshot) => {
      this.#proposals = snapshot.proposals;
      for (const listener of this.#listeners) listener();
    });
  }
  #proposals;
  #unsubscribe;
  #listeners = /* @__PURE__ */ new Set();
  #controller = null;
  #dialog = null;
  #disposed = false;
  /** The current update offers, in catalog order. */
  get proposals() {
    return this.#proposals;
  }
  /** Observe changed update offers. */
  subscribe(listener) {
    this.#listeners.add(listener);
    return () => this.#listeners.delete(listener);
  }
  /** Match only authoritative catalog identities rather than loosely matching display labels. */
  offer(item) {
    return this.#proposals.find((proposal) => proposal.modelId === item.id);
  }
  /** The rows about `item`; the update badge's own menu holds only its two update decisions. */
  entries(item, updateOnly = false) {
    const proposal = this.offer(item);
    const { actions, detectors, outputs } = this.options;
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
      item.kind === "ultralytics" ? !!detectors && !!this.options.detectorDialog : !!actions && /^[a-f0-9]{64}$/.test(item.id),
      !!proposal,
      false
    ).map(
      (entry) => entry.command === "output-thumbnail" && outputs ? modelOutputThumbnailMenu(outputs.choices(), outputs.activeChoice()) : entry
    );
  }
  /** The rows about `item` as shared menu entries, for menus that are not the picker's own. */
  project(item, updateOnly = false) {
    return projectModelContext(
      this.entries(item, updateOnly),
      (entry) => this.command(item, entry)
    );
  }
  /** Browse a verified version family independently of update notification state. */
  family(item) {
    const { actions, feedback, updates } = this.options;
    if (!actions || !this.session || !canBrowseModelVersions(item, !!this.offer(item))) return;
    this.beforeFamily();
    this.#dialog?.close();
    void this.session.versions(item, actions, (severity, message) => feedback?.(severity, message)).finally(() => {
      void updates?.load().catch((error) => {
        feedback?.(
          "error",
          error instanceof Error ? error.message : "Model updates are unavailable."
        );
      });
    });
  }
  /** Execute one row through its authoritative owner and report how it ended. */
  command(item, entry) {
    const { actions, outputs, updates, feedback, detectorDialog } = this.options;
    if (entry.command === "detector-thumbnail" && detectorDialog) {
      this.beforeFamily();
      this.#dialog?.close();
      const current = detectorDialog(item);
      this.#dialog = current;
      void current.open().finally(() => {
        if (this.#dialog === current) this.#dialog = null;
      });
      return;
    }
    if (entry.command === "versions") {
      this.family(item);
      return;
    }
    let status = "";
    void this.#run(async (signal) => {
      if (entry.command === "refresh")
        await actions?.refreshMetadata(
          item.id,
          (message) => {
            status = message;
          },
          signal
        );
      else if (entry.command === "output-thumbnail" && entry.imageId && outputs && actions) {
        const image = await outputs.image(entry.imageId, signal);
        if (signal.aborted) return;
        await actions.replaceThumbnail(item.id, image, signal);
        if (!signal.aborted) status = "Model thumbnail updated.";
      } else if (entry.command === "dismiss") await updates?.dismiss(item.id);
      else if (entry.command === "disable-updates") await updates?.disableModel(item.id);
    }).then((error) => {
      if (this.#disposed) return;
      if (error) feedback?.("error", error);
      else if (status) feedback?.("info", status);
    });
  }
  /** Release the update subscription, a running command and an open detector dialog. */
  dispose() {
    this.#disposed = true;
    this.#unsubscribe?.();
    this.#listeners.clear();
    this.#controller?.abort();
    this.#dialog?.close();
  }
  /** Run one command at a time; resolve with its failure message, or null. */
  async #run(operation) {
    if (this.#controller || this.disabled()) return null;
    const current = new AbortController();
    this.#controller = current;
    try {
      await operation(current.signal);
      return null;
    } catch (failure) {
      if (current.signal.aborted) return null;
      console.error("SugarCubes model action failed.", failure);
      return failure instanceof Error ? failure.message : "The model action failed.";
    } finally {
      this.#controller = null;
    }
  }
};

// frontend/comfyui/ui/model_library/vue/ModelContextState.ts
function useModelContext(options, disabled, beforeFamily, session) {
  const menu = ref(
    null
  );
  const owner = new ModelMenu(options, session, disabled, () => {
    menu.value = null;
    beforeFamily();
  });
  const proposals = shallowRef(owner.proposals);
  const unsubscribe = owner.subscribe(() => {
    proposals.value = owner.proposals;
  });
  const show = (item, event, updateOnly = false) => {
    event.preventDefault();
    event.stopPropagation();
    if (!owner.entries(item, updateOnly).length) return;
    menu.value = { item, x: event.clientX, y: event.clientY, updateOnly };
  };
  onBeforeUnmount(() => {
    menu.value = null;
    unsubscribe();
    owner.dispose();
  });
  const render = () => {
    const current = menu.value;
    return current ? h(ModelContextMenuView, {
      entries: owner.entries(current.item, current.updateOnly),
      x: current.x,
      y: current.y,
      onClose: () => {
        menu.value = null;
      },
      onAction: (entry) => owner.command(current.item, entry)
    }) : null;
  };
  return {
    menu,
    show,
    family: (item) => owner.family(item),
    offer: (item) => owner.offer(item),
    render,
    proposals,
    /** Capture the selected model without creating another metadata or command owner. */
    project: (item) => owner.project(item)
  };
}

export {
  useModelContext
};
