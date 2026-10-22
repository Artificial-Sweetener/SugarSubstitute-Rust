import {
  CanvasSourceTabs
} from "./chunk-T26L5BHO.js";
import {
  CanvasWipeSeparator
} from "./chunk-WIEFPHCO.js";
import {
  CanvasComparisonNavigation
} from "./chunk-CIRP6VLK.js";
import {
  CanvasLoadingIndicator
} from "./chunk-Z66IIUOP.js";
import {
  canvasNavigationSnapshot
} from "./chunk-HDLA7PWP.js";
import {
  computed,
  defineComponent,
  h
} from "./chunk-6PAJ5Y37.js";

// frontend/comfyui/ui/output_canvas/presentation/vue/CanvasOverlay.ts
function resourceMessage(resource) {
  return resource && resource.state !== "ready" && resource.state !== "loading" ? resource.message : "";
}
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
      const primaryMessage = resourceMessage(snapshot.resource);
      const secondaryMessage = resourceMessage(snapshot.secondaryResource);
      return h("div", { class: "sc-output-overlay", "data-canvas-view": snapshot.address.viewId }, [
        snapshot.view.mode === "image" && snapshot.view.comparison && props.capabilities.commands.includes("wipe") ? h(CanvasWipeSeparator, { snapshot, send: props.send }) : null,
        !snapshot.connected || Boolean(primaryMessage) || snapshot.resource.canRetry || snapshot.resource.canAdmitLargeImage || Boolean(secondaryMessage) || snapshot.secondaryResource?.canRetry || snapshot.secondaryResource?.canAdmitLargeImage ? h("div", { class: "sc-output-status" }, [
          snapshot.connected ? null : h("span", "Disconnected"),
          h("span", primaryMessage),
          h("span", secondaryMessage),
          (snapshot.resource.canRetry || snapshot.secondaryResource?.canRetry) && props.capabilities.commands.includes("retry") ? h(
            "button",
            { type: "button", onClick: () => props.send({ type: "retry" }) },
            snapshot.selected && snapshot.selected.mediaKind !== "image" ? "Retry output" : "Retry image"
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
        h(CanvasLoadingIndicator, {
          loading: snapshot.connected && (snapshot.resource.state === "loading" || snapshot.secondaryResource?.state === "loading")
        }),
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
        snapshot.view.comparison && props.capabilities.commands.includes("select-comparison") ? h(CanvasComparisonNavigation, { snapshot: navigation.value, send: props.send }) : props.capabilities.commands.includes("select-source") && (snapshot.sources.length > 1 || (snapshot.sources[0]?.batchCount ?? 0) > 1) ? h(CanvasSourceTabs, {
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
