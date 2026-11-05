import {
  CanvasWipeSeparator
} from "./chunk-WIEFPHCO.js";
import {
  CanvasComparisonNavigation
} from "./chunk-NHCXFNIG.js";
import {
  CanvasLoadingIndicator
} from "./chunk-Z66IIUOP.js";
import {
  CanvasNavigationBar
} from "./chunk-HR7CZA7V.js";
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
    announcement: { type: String, default: "" },
    route: { type: Object, default: null },
    chooseRoute: {
      type: Function,
      default: () => void 0
    }
  },
  /** Compose reported surface status with capability-gated control groups. */
  setup(props) {
    const navigation = computed(
      (previous) => canvasNavigationSnapshot(props.snapshot, previous)
    );
    const chooseRoute = (value) => props.chooseRoute?.(value);
    return () => {
      const snapshot = props.snapshot;
      const zoom = snapshot.zoom?.dimensions && snapshot.zoom.physicalScale > 0 ? snapshot.zoom : null;
      const primaryMessage = resourceMessage(snapshot.resource);
      const secondaryMessage = resourceMessage(snapshot.secondaryResource);
      const input = props.route?.route === "input";
      return h(
        "div",
        {
          class: "sc-output-overlay",
          "data-canvas-view": snapshot.address.viewId,
          "data-canvas-zoom-mode": zoom?.mode,
          "data-canvas-physical-scale": zoom?.physicalScale,
          "data-canvas-image-width": zoom?.dimensions?.width,
          "data-canvas-image-height": zoom?.dimensions?.height
        },
        [
          !input && snapshot.view.mode === "image" && snapshot.view.comparison && props.capabilities.commands.includes("wipe") ? h(CanvasWipeSeparator, { snapshot, send: props.send }) : null,
          !input && (!snapshot.connected || Boolean(primaryMessage) || snapshot.resource.canRetry || snapshot.resource.canAdmitLargeImage || Boolean(secondaryMessage) || snapshot.secondaryResource?.canRetry || snapshot.secondaryResource?.canAdmitLargeImage) ? h("div", { class: "sc-output-status" }, [
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
            loading: !input && snapshot.connected && (snapshot.resource.state === "loading" || snapshot.secondaryResource?.state === "loading")
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
          !input && snapshot.view.comparison && props.capabilities.commands.includes("select-comparison") ? h(CanvasComparisonNavigation, {
            snapshot: navigation.value,
            send: props.send,
            route: props.route,
            chooseRoute
          }) : h(CanvasNavigationBar, {
            snapshot: navigation.value,
            capabilities: props.capabilities,
            send: props.send,
            route: props.route,
            chooseRoute
          })
        ]
      );
    };
  }
});

export {
  CanvasOverlay
};
