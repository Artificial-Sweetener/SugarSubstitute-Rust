import {
  chooseModelThumbnail
} from "./chunk-HBDMWEN5.js";
import {
  ModelRecommendationPreview
} from "./chunk-MRDNZD2T.js";
import {
  ModelKeyRequiredBadge
} from "./chunk-YSDVLBQQ.js";
import {
  ModelBusyRing
} from "./chunk-3UMXK5CY.js";
import {
  defineComponent,
  h,
  nextTick,
  ref,
  watch
} from "./chunk-57LGDI7T.js";

// frontend/comfyui/ui/model_library/ModelArtifactPresentation.ts
function modelArtifactSize(bytes) {
  const units = ["B", "KiB", "MiB", "GiB", "TiB", "PiB"];
  const index = bytes > 0 ? Math.min(units.length - 1, Math.floor(Math.log2(bytes) / 10)) : 0;
  return `${(bytes / 1024 ** index).toLocaleString(void 0, { maximumFractionDigits: 2 })} ${units[index] ?? "B"}`;
}

// frontend/comfyui/ui/surface/ScrollWheelDistance.ts
function scrollWheelDistance(event, axis, page) {
  const factor = event.deltaMode === 1 ? 16 : event.deltaMode === 2 ? page : 1;
  const delta = axis === "vertical" ? event.deltaY : event.deltaX || event.deltaY;
  return delta * factor;
}

// frontend/comfyui/ui/model_library/vue/ModelVersionFamily.ts
var ModelVersionFamily = defineComponent({
  name: "SugarCubesModelVersionFamily",
  props: {
    item: { type: Object, required: true },
    versions: { type: Array, required: true },
    selected: { type: String, required: true },
    busy: { type: Boolean, required: true }
  },
  emits: { select: (_version) => true },
  setup(props, { emit }) {
    const timeline = ref(null);
    const scroll = (event) => {
      if (event.ctrlKey) return;
      event.stopPropagation();
      const host = timeline.value;
      if (!host || host.scrollWidth <= host.clientWidth) return;
      const distance = scrollWheelDistance(event, "horizontal", host.clientWidth);
      if (!distance) return;
      event.preventDefault();
      host.scrollLeft = Math.min(
        host.scrollWidth - host.clientWidth,
        Math.max(0, host.scrollLeft + distance)
      );
    };
    watch(
      () => props.versions,
      () => {
        void nextTick(() => {
          const host = timeline.value;
          const current = host?.querySelector("[data-in-use=true]");
          if (host && current)
            host.scrollLeft = Math.max(
              0,
              current.offsetLeft - host.clientWidth / 2 + current.offsetWidth / 2
            );
        });
      },
      { immediate: true }
    );
    return () => h("section", { class: "sc-model-version-family", "aria-label": "Model version family" }, [
      h(
        "div",
        {
          ref: timeline,
          class: "sc-model-timeline",
          "aria-label": "Model versions",
          onWheelCapture: scroll
        },
        props.versions.length ? props.versions.map((version) => {
          const inUse = version.inUse ?? version.versionId === props.item.providerVersionId;
          const onDisk = version.onDisk;
          const selected = props.selected === version.versionId;
          const access = version.files[0]?.access;
          const availability = onDisk ? "On disk" : access === "api_key_required" ? "API key required" : access === "unknown" ? "Access unavailable" : "Download available";
          const image = inUse ? chooseModelThumbnail(props.item, "standard", 512)?.url ?? version.previewUrl : version.previewUrl;
          return h(
            "button",
            {
              type: "button",
              class: ["sc-model-version-card", { "sc-model-version-selected": selected }],
              "aria-label": `${version.name}, ${inUse ? "In use" : availability}`,
              "aria-pressed": selected,
              "data-in-use": inUse,
              disabled: props.busy || (onDisk ? !version.installedFileName : !version.files.length || version.files[0]?.access === "unknown"),
              onClick: () => emit("select", version)
            },
            [
              h("span", { class: "sc-model-version-portrait" }, [
                h(ModelRecommendationPreview, { url: image }),
                !onDisk && access === "api_key_required" ? h(ModelKeyRequiredBadge, { providerName: "CivitAI" }) : null,
                h("span", { class: "sc-model-version-name" }, version.name),
                h("span", { class: "sc-model-version-check", "aria-hidden": true }, [
                  selected ? h(
                    "svg",
                    { viewBox: "0 0 16 16", "aria-hidden": true, focusable: "false" },
                    [
                      h("path", {
                        d: "M3 8 6.5 11.5 13 4.5",
                        fill: "none",
                        stroke: "currentColor",
                        "stroke-width": 2,
                        "stroke-linecap": "round",
                        "stroke-linejoin": "round"
                      })
                    ]
                  ) : null
                ])
              ]),
              h(
                "span",
                { class: "sc-model-version-state" },
                onDisk ? "On disk" : modelArtifactSize(version.files[0]?.sizeBytes ?? 0)
              )
            ]
          );
        }) : [
          h(
            "div",
            { class: "sc-model-version-loading", role: "status" },
            props.busy ? [h(ModelBusyRing, { size: 22 }), "Loading model versions..."] : "No model versions available."
          )
        ]
      )
    ]);
  }
});

export {
  ModelVersionFamily
};
