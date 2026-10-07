import {
  ModelDiscoveryPortrait
} from "./chunk-Y7OLUA3V.js";
import {
  ModelKeyRequiredBadge
} from "./chunk-KJ2VRCYH.js";
import {
  CivitaiCredentialLayer
} from "./chunk-GFEHLP3Q.js";
import {
  ModelBusyRing
} from "./chunk-SYMYA4CE.js";
import {
  ModelContextMenuView
} from "./chunk-WIKCJTWX.js";
import {
  ModelProviderIcon
} from "./chunk-NC5FFMF2.js";
import {
  defineComponent,
  h,
  shallowRef
} from "./chunk-3W5PGMRW.js";

// frontend/comfyui/ui/model_library/vue/ModelDiscoveryGallery.ts
var ModelDiscoveryGallery = defineComponent({
  name: "SugarCubesModelDiscoveryGallery",
  props: {
    state: { type: Object, required: true },
    actions: { type: Object, required: true }
  },
  setup(props) {
    const menu = shallowRef(null);
    const showMenu = (item, event) => {
      event.preventDefault();
      event.stopPropagation();
      if (props.state.downloading || props.state.credential) return;
      const sources = /* @__PURE__ */ new Map();
      const entries = item.offers.map((offer) => ({
        label: `View ${item.modelName} on ${offer.providerName}`,
        enabled: true,
        command: null,
        url: offer.modelPageUrl,
        providerIcon: offer.providerId
      }));
      if (item.offers.length > 1) {
        entries.push({ label: "", enabled: false, command: null, url: null, separator: true });
        for (const offer of item.offers) {
          const entry = {
            label: `Download from ${offer.providerName}`,
            checked: (props.state.providers[item.identity] ?? item.offers[0]?.providerId) === offer.providerId,
            providerIcon: offer.providerId,
            enabled: true,
            command: null,
            url: null
          };
          sources.set(entry, offer.providerId);
          entries.push(entry);
        }
      }
      menu.value = {
        x: event.clientX,
        y: event.clientY,
        entries,
        sources,
        identity: item.identity
      };
    };
    return () => {
      const state = props.state;
      const currentMenu = menu.value;
      return h(
        "div",
        {
          class: "sc-model-discovery-view",
          onPointerdown: (event) => event.stopPropagation(),
          onWheel: (event) => event.stopPropagation()
        },
        [
          h(
            "p",
            { class: "sc-model-discovery-purpose" },
            state.artifactKind === "upscale_models" ? "Upscalers enlarge existing images and refine details." : "Image models create new images from your prompts."
          ),
          state.loading ? h("div", { class: "sc-model-discovery-loading", role: "status" }, [
            h(ModelBusyRing),
            state.status
          ]) : null,
          !state.loading ? h(
            "div",
            {
              class: "sc-model-discovery-grid",
              "aria-label": "Compatible model suggestions",
              inert: state.credential
            },
            state.plan?.suggestions.map((item) => {
              const offer = item.offers.find(
                (value) => value.providerId === state.providers[item.identity]
              ) ?? item.offers[0];
              if (!offer) return null;
              const selected = state.selected === item.identity;
              return h(
                "div",
                {
                  class: "sc-model-discovery-card",
                  key: item.identity,
                  onContextmenu: (event) => showMenu(item, event)
                },
                [
                  h(ModelDiscoveryPortrait, {
                    previewUrl: offer.previewUrl,
                    title: item.modelName,
                    selected,
                    disabled: state.downloading,
                    onSelect: () => {
                      menu.value = null;
                      props.actions.select(selected ? "" : item.identity);
                    }
                  }),
                  h(
                    "a",
                    {
                      class: "sc-model-discovery-provider",
                      "aria-label": `View ${item.modelName} on ${offer.providerName}`,
                      href: state.downloading ? void 0 : offer.modelPageUrl,
                      "aria-disabled": state.downloading,
                      tabindex: state.downloading ? -1 : 0,
                      target: "_blank",
                      rel: "noopener noreferrer"
                    },
                    [h(ModelProviderIcon, { provider: offer.providerId })]
                  ),
                  offer.access === "api_key_required" ? h(ModelKeyRequiredBadge, { providerName: offer.providerName }) : null
                ]
              );
            })
          ) : null,
          !state.loading && state.status ? h("div", { class: "sc-model-discovery-status", role: "status" }, [
            state.downloading ? h(ModelBusyRing) : null,
            state.status
          ]) : null,
          menu.value && !state.credential ? h(ModelContextMenuView, {
            entries: menu.value.entries,
            x: menu.value.x,
            y: menu.value.y,
            label: "Model providers",
            onClose: () => {
              menu.value = null;
            },
            onAction: (entry) => {
              const current = currentMenu;
              const source = current?.sources.get(entry);
              if (current && source) props.actions.provider(current.identity, source);
              menu.value = null;
            }
          }) : null,
          state.credential ? h(CivitaiCredentialLayer, {
            busy: state.credentialBusy,
            error: state.credentialError,
            actions: props.actions
          }) : null
        ]
      );
    };
  }
});

export {
  ModelDiscoveryGallery
};
