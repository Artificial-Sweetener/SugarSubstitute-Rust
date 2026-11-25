import {
  useModelSelectionUpdates
} from "./chunk-5NNW6U22.js";
import {
  ModelTransferView
} from "./chunk-UMQMPOKY.js";
import {
  modelPickerPlacement
} from "./chunk-JK626FTQ.js";
import {
  ModelPickerPopupContent
} from "./chunk-OMRMSSWC.js";
import {
  useModelContext
} from "./chunk-2WMWUCGK.js";
import {
  ModelUpdateButton
} from "./chunk-P7J5YKTZ.js";
import {
  useModelBrowsing
} from "./chunk-TGMBNLE7.js";
import {
  MODEL_BANNER_SIZE,
  chooseModelThumbnail
} from "./chunk-XU3535ZK.js";
import {
  ModelCatalogImage
} from "./chunk-OLJZ2VLG.js";
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
} from "./chunk-OJTDXCRK.js";

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

// frontend/comfyui/ui/menus/TextFieldEditMenu.ts
import { ContextMenu } from "../../menus/ContextMenu.js";

// integrations/contracts/HostDescription.ts
var HOST_PROTOCOLS = Object.freeze({
  panel: Object.freeze({ name: "sugarcubes.panel", version: 3 }),
  canvas: Object.freeze({ name: "sugarcubes.canvas", version: 3 }),
  canvasMasks: Object.freeze({ name: "sugarcubes.canvas.masks", version: 2 }),
  memory: Object.freeze({ name: "sugarcubes.memory", version: 1 }),
  updates: Object.freeze({ name: "sugarcubes.updates", version: 1 }),
  releaseNotes: Object.freeze({ name: "sugarcubes.release-notes", version: 1 }),
  sessionNotice: Object.freeze({ name: "sugarcubes.session-notice", version: 1 }),
  fileDrag: Object.freeze({ name: "sugarcubes.file-drag", version: 1 }),
  backend: Object.freeze({ name: "sugarcubes.backend", version: 1 }),
  updateReport: Object.freeze({ name: "sugarcubes.update-report", version: 1 }),
  fileChoice: Object.freeze({ name: "sugarcubes.file-choice", version: 1 })
});
var HOST_CAPABILITIES = Object.freeze({
  /** The host's configured presentation replaces the user's saved appearance while embedded. */
  hostStyles: "presentation.host-styles",
  /** The host's configured accent is the system accent, offered as a choice beside a custom one. */
  systemAccent: "presentation.system-accent",
  /** The host presents output images itself; the page never falls back to browser images. */
  hostOutputSurface: "output.host-surface",
  /** Besides sugarcubes.updates version 1, the host answers version 2 on the same channel, whose states name the product they describe. */
  namedUpdates: "updates.product-name",
  /** Besides sugarcubes.session-notice version 1, the host answers version 2 on the same channel, whose notices include lines the host words itself. */
  noticeLines: "session-notice.lines",
  /** The host takes up to 64 acrylic regions in one canvas geometry message, where a host without this takes two. */
  backdropRegions: "canvas.backdrop-regions",
  /** In this host's browser, SugarCubes owns the context menu of every text field, including the host application's own fields; without it, SugarCubes owns only its own fields. */
  textMenus: "editing.text-menus",
  /** Besides desktop-session checkpoint version 1, the host takes and keeps checkpoint version 2, whose entries also name where each workflow's masonry is scrolled; it gives a checkpoint back as version 1 when no entry names a scroll. */
  checkpointScroll: "session.checkpoint-scroll"
});

// integrations/shared/HostDescription.ts
var DeclaredHost = class _DeclaredHost {
  #protocols;
  #capabilities;
  constructor(content) {
    this.#protocols = new Set(content.protocols.map(protocolKey));
    this.#capabilities = new Set(content.capabilities.map((capability) => capability.name));
  }
  /**
   * Adopt a schema-validated description, refusing one that repeats a protocol or capability.
   *
   * A repeated name is ambiguous about what the host meant, so the whole description fails
   * closed rather than guessing which entry to honour.
   */
  static read(content) {
    const protocols = new Set(content.protocols.map((protocol) => protocol.name));
    const capabilities = new Set(content.capabilities.map((capability) => capability.name));
    if (protocols.size !== content.protocols.length || capabilities.size !== content.capabilities.length)
      return null;
    return new _DeclaredHost(content);
  }
  /** Serve the page outside any host: no host protocol and no host capability. */
  static standalone = new _DeclaredHost({ protocols: [], capabilities: [] });
  /** Describe a host that configured its panel without first describing itself. */
  static undeclared = new _DeclaredHost({
    protocols: [HOST_PROTOCOLS.panel],
    capabilities: []
  });
  /** Whether the host serves this protocol at exactly this envelope version. */
  serves(protocol) {
    return this.#protocols.has(protocolKey(protocol));
  }
  /** Whether the host declares this capability. */
  declares(capability) {
    return this.#capabilities.has(capability);
  }
  /** Compare declarations regardless of the order a host listed them in. */
  same(other) {
    return sameSet(this.#protocols, other.#protocols) && sameSet(this.#capabilities, other.#capabilities);
  }
};
function protocolKey(protocol) {
  return `${protocol.name}@${String(protocol.version)}`;
}
function sameSet(left, right) {
  return left.size === right.size && [...left].every((name) => right.has(name));
}

// frontend/comfyui/ui/menus/TextFieldEditMenu.ts
var FIELD_MENU_OWNER = "data-sc-field-menu";

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
    detectorDialog: {
      type: Function,
      default: null
    },
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
    const current = ref(props.binding.readValue());
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
    const browsing = useModelBrowsing(() => compatible.value);
    const { query, folder, active, wallWidth, visible, navigable, completion } = browsing;
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
      browsing.restart(current.value);
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
      if (event.key === "Tab" && completion.value) {
        event.preventDefault();
        browsing.complete();
      } else if (event.key === "Enter") {
        event.preventDefault();
        const item = visible.value[active.value];
        if (item) select(item.value);
      } else if (browsing.move(event.key)) event.preventDefault();
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
      const banner = item ? chooseModelThumbnail(item, "banner", MODEL_BANNER_SIZE) : null;
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
          [FIELD_MENU_OWNER]: "",
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
              if (event.target instanceof HTMLInputElement) browsing.search(event.target.value);
            }
          }),
          open.value && completion.value ? h("span", { class: "sc-model-completion", "aria-hidden": true }, [
            h("span", { class: "sc-model-completion-prefix" }, query.value),
            completion.value.suffix
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
                    browsing.open("");
                    input.value?.focus();
                  },
                  visible: visible.value,
                  folder: folder.value,
                  selected: current.value,
                  active: active.value,
                  optionPrefix: popupId,
                  onNavigate: (path) => {
                    browsing.open(path);
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
