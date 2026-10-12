import {
  canvasNavigationSnapshot
} from "./chunk-HDLA7PWP.js";
import {
  CanvasSourceTabs
} from "./chunk-MS5JIV6F.js";
import {
  CanvasWipeSeparator
} from "./chunk-OVL65VLS.js";
import {
  computed,
  defineComponent,
  h
} from "./chunk-MNPWJMUZ.js";

// frontend/comfyui/ui/output_canvas/presentation/vue/CanvasOverlay.ts
var CanvasOverlay = defineComponent({
  name: "CanvasOverlay",
  props: {
    snapshot: { type: Object, required: true },
    capabilities: { type: Object, required: true },
    send: { type: Function, required: true },
    announcement: { type: String, default: "" }
  },
  /** Compose reported surface status with capability-gated control groups. */
  setup(props) {
    const navigation = computed(
      (previous) => canvasNavigationSnapshot(props.snapshot, previous)
    );
    return () => {
      const snapshot = props.snapshot;
      return h("div", { class: "sc-output-overlay", "data-canvas-view": snapshot.address.viewId }, [
        snapshot.view.mode === "image" && snapshot.view.comparison && props.capabilities.commands.includes("wipe") ? h(CanvasWipeSeparator, { snapshot, send: props.send }) : null,
        !snapshot.connected || snapshot.resource.state !== "ready" && Boolean(snapshot.resource.message) || snapshot.resource.canRetry || snapshot.resource.canAdmitLargeImage || snapshot.secondaryResource?.state !== "ready" && Boolean(snapshot.secondaryResource?.message) || snapshot.secondaryResource?.canRetry || snapshot.secondaryResource?.canAdmitLargeImage ? h("div", { class: "sc-output-status" }, [
          snapshot.connected ? null : h("span", "Disconnected"),
          h("span", snapshot.resource.state !== "ready" ? snapshot.resource.message : ""),
          h(
            "span",
            snapshot.secondaryResource?.state !== "ready" ? snapshot.secondaryResource?.message : ""
          ),
          (snapshot.resource.canRetry || snapshot.secondaryResource?.canRetry) && props.capabilities.commands.includes("retry") ? h(
            "button",
            { type: "button", onClick: () => props.send({ type: "retry" }) },
            "Retry image"
          ) : null,
          (snapshot.resource.canAdmitLargeImage || snapshot.secondaryResource?.canAdmitLargeImage) && props.capabilities.commands.includes("admit-large-image") ? h(
            "button",
            { type: "button", onClick: () => props.send({ type: "admit-large-image" }) },
            "Load large image"
          ) : null,
          snapshot.selected?.mediaKind !== void 0 && snapshot.selected.mediaKind !== "image" && props.capabilities.commands.includes("open-media") ? h(
            "button",
            {
              type: "button",
              onClick: () => props.send({ type: "open-media", target: "primary" })
            },
            "Open media"
          ) : null
        ]) : null,
        h(
          "span",
          {
            class: "sc-output-announcement",
            role: "status",
            "aria-live": "polite",
            "aria-atomic": "true"
          },
          props.announcement
        ),
        props.capabilities.commands.includes("select-source") && snapshot.sources.length > 0 ? h(CanvasSourceTabs, {
          snapshot: navigation.value,
          capabilities: props.capabilities,
          send: props.send
        }) : null
      ]);
    };
  }
});

export {
  CanvasOverlay
};
