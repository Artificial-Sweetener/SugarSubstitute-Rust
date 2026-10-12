import {
  CanvasCompareControls
} from "./chunk-VMDE2JL6.js";
import {
  CanvasSourceTabs
} from "./chunk-BBIMJ2PJ.js";
import {
  CanvasToolbar
} from "./chunk-UL5SOQL2.js";
import {
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
    return () => {
      const snapshot = props.snapshot;
      const zoom = snapshot.zoom;
      const dimensions = zoom.dimensions;
      const label = zoom.mode === "fit" ? "Fit" : zoom.mode === "native" ? "1:1" : `${Math.round(zoom.physicalScale * 100)}%`;
      return h("div", { class: "sc-output-overlay", "data-canvas-view": snapshot.address.viewId }, [
        h("div", { class: "sc-output-navigation" }, [
          h(CanvasToolbar, { snapshot, capabilities: props.capabilities, send: props.send }),
          snapshot.view.comparison ? h(CanvasCompareControls, {
            snapshot,
            capabilities: props.capabilities,
            send: props.send
          }) : null
        ]),
        h("div", { class: "sc-output-status" }, [
          h("span", { "aria-label": "Zoom" }, label),
          dimensions ? h("span", `${dimensions.width} \xD7 ${dimensions.height}`) : null,
          snapshot.preview ? h("span", "Live preview") : null,
          snapshot.connected ? null : h("span", "Disconnected"),
          h("span", snapshot.resource.message),
          snapshot.resource.canRetry && props.capabilities.commands.includes("retry") ? h(
            "button",
            { type: "button", onClick: () => props.send({ type: "retry" }) },
            "Retry image"
          ) : null,
          snapshot.resource.canAdmitLargeImage && props.capabilities.commands.includes("admit-large-image") ? h(
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
        ]),
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
        props.capabilities.commands.includes("select-source") ? h(CanvasSourceTabs, { snapshot, send: props.send }) : null
      ]);
    };
  }
});

export {
  CanvasOverlay
};
