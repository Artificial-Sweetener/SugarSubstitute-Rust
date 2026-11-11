import {
  useModelSelectionUpdates
} from "./chunk-OKMKTXD6.js";
import {
  ModelTransferView
} from "./chunk-RR5YCA3U.js";
import {
  ModelPickerPopupContent,
  filterModelChoices
} from "./chunk-EYWLUWSB.js";
import {
  useModelContext
} from "./chunk-LMZEATZG.js";
import {
  modelWallNeighbor
} from "./chunk-ZMF3A2RB.js";
import {
  ModelUpdateButton
} from "./chunk-4DBFGV2O.js";
import {
  chooseModelThumbnail
} from "./chunk-HBDMWEN5.js";
import {
  ModelCatalogImage
} from "./chunk-RPKXVHLS.js";
import {
  Teleport,
  computed,
  defineComponent,
  h,
  nextTick,
  onBeforeUnmount,
  ref,
  shallowRef,
  useId,
  watch
} from "./chunk-3W5PGMRW.js";

// frontend/comfyui/ui/model_library/ModelCompatibility.ts
function modelCompatibility(item, context) {
  const traits = item.traits;
  if (traits?.conflict) return "unknown";
  const architecture = traits?.architecture || item.architecture?.base || "";
  if (context.addon && context.family) {
    const support = traits?.compatibleTargets.find(
      (target) => target.family === context.family
    )?.support;
    return support ?? (traits?.family ? "incompatible" : "unknown");
  }
  if (context.architecture && !architecture) return "unknown";
  if (context.architecture && architecture !== context.architecture) return "incompatible";
  if (context.family && !traits?.family) return "unknown";
  if (context.family && traits?.family !== context.family) return "incompatible";
  return context.addon ? "unknown" : "full";
}
function queryModelCatalog(items, query) {
  const terms = query.text?.trim().toLowerCase().split(/\s+/).filter(Boolean) ?? [];
  return items.filter(
    (item) => (!query.kind || item.kind === query.kind) && (!query.architecture || (item.traits?.architecture || item.architecture?.base) === query.architecture) && (!query.family || item.traits?.family === query.family) && (!query.nsfw || (item.traits?.nsfw ?? "unknown") === query.nsfw) && (!query.tags || query.tags.every(
      (tag) => item.tags?.some((value) => value.toLowerCase() === tag.toLowerCase())
    )) && terms.every(
      (term) => [
        item.value,
        item.title,
        item.baseModel,
        item.traits?.familyName ?? "",
        ...item.trainedWords,
        ...item.tags ?? []
      ].join(" ").toLowerCase().includes(term)
    ) && (!query.compatibleWith || ["full", "partial"].includes(modelCompatibility(item, query.compatibleWith)))
  );
}

// frontend/comfyui/ui/model_library/ModelPickerCompletion.ts
function modelChoiceCompletion(item, query) {
  if (!item) return null;
  const path = (item.relativePath.trim() || item.value.trim()).replaceAll("\\", "/");
  const stripped = path.replace(/\.(?:safetensors|ckpt|pt)$/i, "");
  const filename = stripped.split("/").at(-1) ?? stripped;
  const normalized = query.toLocaleLowerCase();
  const hasSeparator = /[\\/]/.test(query);
  if (hasSeparator && stripped.toLocaleLowerCase().startsWith(normalized.replaceAll("\\", "/"))) {
    const candidate = query.includes("\\") && !query.includes("/") ? stripped.replaceAll("/", "\\") : stripped;
    return completion(candidate.slice(query.length), query);
  }
  if (filename.toLocaleLowerCase().startsWith(normalized))
    return completion(filename.slice(query.length), query);
  if (!hasSeparator && query.trim().length > 0) {
    const start = filename.toLocaleLowerCase().indexOf(normalized);
    if (start > 0) return completion(filename.slice(start + query.length), query);
  }
  const friendly = [item.title.trim(), item.version.trim()].filter(Boolean).join(" - ");
  if (friendly.toLocaleLowerCase().startsWith(normalized))
    return completion(friendly.slice(query.length), query);
  return filename ? { suffix: ` \u2192 ${filename}`, text: filename } : null;
}
function completion(suffix, query) {
  return suffix ? { suffix, text: query + suffix } : null;
}

// frontend/comfyui/ui/model_library/ModelPickerPlacement.ts
var PREFERRED_WIDTH = 560;
var PREFERRED_HEIGHT = 630;
var MARGIN = 8;
function safeAxis(origin, length) {
  const bounded = Math.max(0, length);
  return bounded > MARGIN * 2 ? [origin + MARGIN, origin + bounded - MARGIN] : [origin, origin + bounded];
}
function modelPickerPlacement(boundary, anchor) {
  const [safeLeft, safeRight] = safeAxis(boundary.left, boundary.width);
  const [safeTop, safeBottom] = safeAxis(boundary.top, boundary.height);
  const width = Math.min(PREFERRED_WIDTH, Math.max(1, safeRight - safeLeft));
  const left = Math.max(safeLeft, Math.min(anchor.left, safeRight - width));
  const bottom = anchor.top + anchor.height;
  const below = Math.max(0, safeBottom - bottom);
  const above = Math.max(0, anchor.top - safeTop);
  const mode = below >= PREFERRED_HEIGHT ? "below" : above >= PREFERRED_HEIGHT || above > below ? "above" : "below";
  const height = Math.min(PREFERRED_HEIGHT, Math.max(1, mode === "below" ? below : above));
  return { left, top: mode === "below" ? bottom : anchor.top - height, width, height, mode };
}

// frontend/comfyui/ui/model_library/ModelPickerSession.ts
import { ModelVersionDialog } from "../ModelVersionDialog.js";
var ModelPickerSession = class {
  #document;
  #resolve;
  #listeners = /* @__PURE__ */ new Set();
  #progress = null;
  #dialog = null;
  #active = true;
  #transfer = 0;
  #controller = null;
  /** Bind a receiver that survives presentation replacement but rejects removed workflow fields. */
  constructor(document2, resolve) {
    this.#document = document2;
    this.#resolve = resolve;
  }
  /** Read the current workflow choice independently of the inspected model family. */
  current() {
    return this.#resolve()?.readValue() ?? "";
  }
  /** Expose the current transfer without allowing views to own its state. */
  read() {
    return this.#progress;
  }
  /** Follow progress until the temporary field renderer is replaced or removed. */
  subscribe(listener) {
    this.#listeners.add(listener);
    return () => this.#listeners.delete(listener);
  }
  /** Write only an exact legal host option, preserving host path separator conventions. */
  select(filename) {
    const binding = this.#active ? this.#resolve() : null;
    const legal = binding?.options.find(
      (option) => option.replaceAll("\\", "/") === filename.replaceAll("\\", "/")
    );
    if (!binding || legal === void 0 || binding.disabled())
      throw new Error("The model is installed, but its current Comfy field is unavailable.");
    binding.writeValue(legal);
  }
  /** Publish one scoped transfer and clear it on success, cancellation, or failure. */
  async download(operation, model, signal) {
    if (!this.#active || this.#progress)
      throw new Error("The model field is unavailable or already downloading.");
    const transfer = ++this.#transfer;
    const controller = new AbortController();
    this.#controller = controller;
    const cancel = () => controller.abort();
    signal?.addEventListener("abort", cancel, { once: true });
    if (signal?.aborted) cancel();
    this.#publish({ message: "Waiting to download\u2026", percentage: null, model });
    try {
      if (controller.signal.aborted) throw new DOMException("Cancelled", "AbortError");
      const filename = await operation((progress) => {
        if (this.#active && !controller.signal.aborted && transfer === this.#transfer)
          this.#publish({ ...progress, model });
      }, controller.signal);
      if (!this.#active || controller.signal.aborted)
        throw new DOMException("Cancelled", "AbortError");
      return filename;
    } finally {
      signal?.removeEventListener("abort", cancel);
      if (this.#controller === controller) this.#controller = null;
      if (transfer === this.#transfer) {
        this.#transfer += 1;
        this.#publish(null);
      }
    }
  }
  /** Keep an accepted acquisition alive until its refreshed legal choice is committed or its field disappears. */
  async useDownload(operation, model) {
    try {
      this.select(await this.download(operation, model));
    } catch (error) {
      if (this.#active) throw error;
    }
  }
  /** Browse versions without tying the modal to a temporary combobox renderer. */
  async versions(item, actions, feedback) {
    if (!this.#active || this.#progress) return;
    this.#dialog?.close();
    const dialog = new ModelVersionDialog(this.#document, item, actions, this, feedback);
    this.#dialog = dialog;
    try {
      await dialog.open();
    } finally {
      if (this.#dialog === dialog) this.#dialog = null;
    }
  }
  /** Close interactions when the actual node or supported renderer disappears. */
  dispose() {
    this.#active = false;
    this.#controller?.abort();
    this.#controller = null;
    this.#dialog?.close();
    this.#dialog = null;
    this.#transfer += 1;
    this.#publish(null);
    this.#listeners.clear();
  }
  /** Notify subscribers through the single progress state owner. */
  #publish(progress) {
    this.#progress = progress;
    for (const listener of this.#listeners) listener();
  }
};

// frontend/comfyui/ui/surface/appearance/ComboToggleController.ts
import { readDocumentVisualStyle } from "../../surface/appearance/VisualStyleBoundary.js";
var leases = /* @__PURE__ */ new WeakMap();
function acquireComboToggle(documentRef) {
  let lease = leases.get(documentRef);
  if (!lease) {
    lease = {
      users: 0,
      onClick: (event) => routeComboClick(documentRef, event),
      onPointerDown: (event) => retainCaptionToggle(documentRef, event)
    };
    leases.set(documentRef, lease);
    documentRef.addEventListener("click", lease.onClick, true);
    documentRef.addEventListener("pointerdown", lease.onPointerDown, true);
  }
  lease.users += 1;
  let released = false;
  return () => {
    if (!lease || released) return;
    released = true;
    if (--lease.users !== 0) return;
    documentRef.removeEventListener("click", lease.onClick, true);
    documentRef.removeEventListener("pointerdown", lease.onPointerDown, true);
    leases.delete(documentRef);
  };
}
function captionTrigger(document2, target) {
  if (readDocumentVisualStyle(document2) !== "gimp" || !(target instanceof Element) || target.closest('[data-sc-style-combo-frame],button,input,a,[role="button"]'))
    return null;
  const trigger = target.closest("[data-sc-style-combo-layout]")?.querySelector(
    '[data-sc-style-combo-frame]:not([data-sc-model-native]) > [role="combobox"]'
  );
  return trigger && !trigger.matches(':disabled,[aria-disabled="true"]') ? trigger : null;
}
function retainCaptionToggle(document2, event) {
  const trigger = event.button === 0 ? captionTrigger(document2, event.target) : null;
  if (!trigger) return;
  event.preventDefault();
  event.stopImmediatePropagation();
  trigger.focus({ preventScroll: true });
}
function routeComboClick(documentRef, event) {
  if (!(event.target instanceof Element)) return;
  const trigger = captionTrigger(documentRef, event.target);
  if (trigger) {
    event.preventDefault();
    event.stopImmediatePropagation();
    trigger.focus({ preventScroll: true });
    trigger.click();
    return;
  }
  const inlineSearch = event.target.closest("[data-sc-inline-search]");
  const frame = event.target.closest(
    "[data-sc-style-combo-frame], [data-capture-wheel]"
  );
  const activation = event.target.closest(
    '[role="combobox"], .sc-model-field-arrow, button[aria-hidden="true"]'
  );
  if (!inlineSearch && (!frame || !activation || !frame.contains(activation))) return;
  const combo = inlineSearch ?? frame?.querySelector(':scope > [role="combobox"]');
  if (combo?.getAttribute("aria-expanded") !== "true") return;
  event.preventDefault();
  event.stopImmediatePropagation();
  const popupId = combo.getAttribute("aria-controls");
  const popup = popupId ? documentRef.getElementById(popupId) : null;
  const target = popup?.querySelector("[data-sc-inline-search]") ?? popup ?? combo;
  target.dispatchEvent(
    new KeyboardEvent("keydown", {
      key: "Escape",
      code: "Escape",
      bubbles: true,
      cancelable: true
    })
  );
}

// frontend/comfyui/ui/model_library/vue/ModelPickerField.ts
var ModelPickerField = defineComponent({
  name: "SugarCubesModelPickerField",
  props: {
    session: { type: Object, default: null },
    binding: { type: Object, required: true },
    items: { type: Array, required: true },
    actions: { type: Object, default: null },
    feedback: { type: Function, default: null },
    updates: { type: Object, default: null },
    detectors: { type: Object, default: null },
    outputs: { type: Object, default: null },
    discoveryAction: { type: Function, default: null },
    compatibility: {
      type: Function,
      default: null
    }
  },
  emits: { contextReady: (_read) => true },
  setup(props, { emit }) {
    const releaseComboToggle = acquireComboToggle(document);
    const session = props.session ?? new ModelPickerSession(document, () => props.binding);
    const transfer = shallowRef(session.read());
    const unsubscribeTransfer = session.subscribe(() => {
      transfer.value = session.read();
    });
    const open = ref(false);
    const context = useModelContext(
      props,
      () => props.binding.disabled(),
      () => close(),
      session
    );
    const query = ref("");
    const folder = ref("");
    const current = ref(props.binding.readValue());
    const active = ref(0);
    const input = ref(null);
    const surface = ref(null);
    const popup = ref(null);
    const placement = ref({});
    const opensBelow = ref(true);
    const popupId = useId();
    const currentItem = computed(() => props.items.find((item) => item.value === current.value));
    useModelSelectionUpdates(() => currentItem.value, props.updates, props.feedback);
    emit("contextReady", () => currentItem.value ? context.project(currentItem.value) : []);
    const compatibility = ref(props.compatibility?.() ?? null);
    const unfiltered = ref(false);
    const compatible = computed(
      () => queryModelCatalog(props.items, {
        compatibleWith: unfiltered.value ? null : compatibility.value
      })
    );
    const offersDiscovery = computed(() => !!props.discoveryAction && !compatible.value.length);
    const visible = computed(() => filterModelChoices(compatible.value, query.value, folder.value));
    const navigable = computed(() => filterModelChoices(compatible.value, query.value, ""));
    const wallWidth = ref(540);
    const completion2 = computed(
      () => modelChoiceCompletion(visible.value[active.value], query.value)
    );
    const unsubscribe = props.binding.subscribe(() => {
      current.value = props.binding.readValue();
      compatibility.value = props.compatibility?.() ?? null;
    });
    let frame = null;
    const position = () => {
      if (!open.value || !surface.value) return;
      const rectangle = surface.value.getBoundingClientRect();
      const geometry = modelPickerPlacement(
        { left: 0, top: 0, width: innerWidth, height: innerHeight },
        rectangle
      );
      opensBelow.value = geometry.mode === "below";
      const nextPlacement = {
        left: `${String(geometry.left)}px`,
        top: `${String(geometry.top)}px`,
        width: `${String(geometry.width)}px`,
        height: `${String(geometry.height)}px`
      };
      if (Object.entries(nextPlacement).some(([key2, value]) => placement.value[key2] !== value))
        placement.value = nextPlacement;
      frame = requestAnimationFrame(position);
    };
    const close = () => {
      context.menu.value = null;
      open.value = false;
      query.value = "";
      if (frame !== null) cancelAnimationFrame(frame);
      frame = null;
    };
    watch(offersDiscovery, (offered) => {
      if (offered) close();
    });
    const select = (value) => {
      if (!props.binding.options.includes(value) || props.binding.disabled()) return;
      props.binding.writeValue(value);
      current.value = props.binding.readValue();
      close();
      input.value?.focus();
    };
    const show = () => {
      if (open.value || transfer.value || props.binding.disabled()) return;
      compatibility.value = props.compatibility?.() ?? null;
      unfiltered.value = false;
      open.value = true;
      query.value = "";
      folder.value = "";
      active.value = Math.max(
        0,
        visible.value.findIndex((item) => item.value === current.value)
      );
      position();
      void nextTick(() => input.value?.focus());
    };
    const outside = (event) => {
      if (context.menu.value) return;
      if (event.target instanceof Node && !surface.value?.contains(event.target) && !popup.value?.contains(event.target))
        close();
    };
    const escape = (event) => {
      if (context.menu.value) return;
      if (!open.value || event.key !== "Escape") return;
      event.preventDefault();
      event.stopPropagation();
      close();
      void nextTick(() => input.value?.focus());
    };
    document.addEventListener("pointerdown", outside, true);
    document.addEventListener("keydown", escape, true);
    onBeforeUnmount(() => {
      releaseComboToggle();
      close();
      unsubscribe();
      unsubscribeTransfer();
      if (!props.session) session.dispose();
      document.removeEventListener("pointerdown", outside, true);
      document.removeEventListener("keydown", escape, true);
    });
    const key = (event) => {
      event.stopPropagation();
      if (event.key === "Escape") {
        event.preventDefault();
        close();
        return;
      }
      if (!open.value && (event.key === "Enter" || event.key === "ArrowDown")) {
        event.preventDefault();
        show();
        return;
      }
      if (!open.value) return;
      if (event.key === "Tab" && completion2.value) {
        event.preventDefault();
        const highlighted = visible.value[active.value]?.value;
        query.value = completion2.value.text;
        active.value = Math.max(
          0,
          visible.value.findIndex((item) => item.value === highlighted)
        );
      } else if (event.key === "Enter") {
        event.preventDefault();
        const item = visible.value[active.value];
        if (item) select(item.value);
      } else if (["ArrowDown", "ArrowUp", "ArrowLeft", "ArrowRight", "Home", "End"].includes(event.key)) {
        event.preventDefault();
        active.value = modelWallNeighbor(visible.value, wallWidth.value, active.value, event.key);
      }
    };
    return () => {
      if (offersDiscovery.value && !transfer.value)
        return h(
          "button",
          {
            class: "sc-model-find",
            type: "button",
            disabled: props.binding.disabled(),
            onPointerdown: (event) => event.stopPropagation(),
            onMousedown: (event) => event.stopPropagation(),
            onClick: props.discoveryAction
          },
          [h("span", { class: "sc-model-find-icon", "aria-hidden": "true" }), "Find models"]
        );
      const item = currentItem.value;
      const banner = item ? chooseModelThumbnail(item, "banner", 768) : null;
      const label = item ? [item.title, item.version].filter(Boolean).join(" - ") : current.value;
      return h(
        "div",
        {
          ref: surface,
          class: [
            "sc-model-field",
            { "sc-model-field-open": open.value, "sc-model-field-transferring": !!transfer.value }
          ],
          "data-sc-style-combo-frame": "",
          "aria-busy": !!transfer.value,
          onPointerdown: (event) => event.stopPropagation(),
          onMousedown: (event) => event.stopPropagation(),
          onWheel: (event) => event.stopPropagation(),
          onContextmenu: (event) => {
            if (item) context.show(item, event);
          }
        },
        [
          transfer.value ? h(ModelTransferView, { transfer: transfer.value }) : null,
          banner && !open.value && !transfer.value ? h(ModelCatalogImage, { class: "sc-model-banner", image: banner }) : null,
          h("input", {
            ref: input,
            value: transfer.value ? `Downloading ${transfer.value.model.name}` : open.value ? query.value : label,
            readonly: !open.value,
            disabled: props.binding.disabled() || !!transfer.value,
            role: "combobox",
            "aria-label": props.binding.label,
            "aria-expanded": open.value,
            "aria-controls": open.value ? popupId : void 0,
            "aria-activedescendant": open.value && visible.value[active.value] ? `${popupId}-option-${String(active.value)}` : void 0,
            "aria-autocomplete": "both",
            title: transfer.value ? transfer.value.model.name : current.value,
            spellcheck: false,
            onClick: show,
            onKeydown: key,
            onInput: (event) => {
              if (event.target instanceof HTMLInputElement) {
                query.value = event.target.value;
                active.value = 0;
              }
            }
          }),
          open.value && completion2.value ? h("span", { class: "sc-model-completion", "aria-hidden": true }, [
            h("span", { class: "sc-model-completion-prefix" }, query.value),
            completion2.value.suffix
          ]) : null,
          item && !open.value && !transfer.value && context.offer(item) ? h(ModelUpdateButton, {
            onOpen: () => {
              close();
              context.family(item);
            },
            onContext: (event) => context.show(item, event, true)
          }) : null,
          h(
            "button",
            {
              type: "button",
              class: "sc-model-field-arrow",
              tabindex: -1,
              disabled: !!transfer.value,
              "aria-label": `Choose ${props.binding.label}`,
              onClick: show
            },
            h("span", { class: "sc-model-chevron" })
          ),
          open.value ? h(
            Teleport,
            { to: "body" },
            h(
              "div",
              {
                ref: popup,
                id: popupId,
                class: ["sc-model-popup", { "sc-model-popup-above": !opensBelow.value }],
                style: placement.value,
                onPointerdown: (event) => event.stopPropagation(),
                onWheel: (event) => event.stopPropagation()
              },
              [
                h(ModelPickerPopupContent, {
                  items: navigable.value,
                  compatibilityLabel: compatibility.value?.architecture || compatibility.value?.label || "",
                  unfiltered: unfiltered.value,
                  onToggleFilter: () => {
                    unfiltered.value = !unfiltered.value;
                    folder.value = "";
                    active.value = 0;
                    input.value?.focus();
                  },
                  visible: visible.value,
                  folder: folder.value,
                  selected: current.value,
                  active: active.value,
                  optionPrefix: popupId,
                  onNavigate: (path) => {
                    folder.value = path;
                    active.value = 0;
                    input.value?.focus();
                  },
                  onSelect: select,
                  onHighlight: (index) => {
                    active.value = index;
                  },
                  onGeometry: (width) => {
                    wallWidth.value = width;
                  },
                  updateOffers: context.proposals.value,
                  onFamily: (target) => {
                    close();
                    context.family(target);
                  },
                  onUpdateContext: (target, event) => context.show(target, event, true),
                  onContext: context.show
                })
              ]
            )
          ) : null,
          context.render()
        ]
      );
    };
  }
});

export {
  ModelPickerField
};
