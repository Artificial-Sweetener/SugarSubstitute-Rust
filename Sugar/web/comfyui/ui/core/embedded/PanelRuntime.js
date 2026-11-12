// integrations/shared/PanelValidation.generated.ts
function isRecord(value) {
  return !!value && typeof value === "object" && !Array.isArray(value);
}
function isPanelPhotoProPalette(value) {
  return value === "black" || value === "dark-gray" || value === "medium-gray" || value === "light-gray";
}
function isPanelPresentation(value) {
  return isRecord(value) && Object.keys(value).length === 4 + (Object.hasOwn(value, "photoProPalette") ? 1 : 0) && Object.hasOwn(value, "pluginMode") && typeof value["pluginMode"] === "boolean" && Object.hasOwn(value, "visualStyle") && (value["visualStyle"] === "comfy" || value["visualStyle"] === "fluent" || value["visualStyle"] === "spectrum" || value["visualStyle"] === "photopro" || value["visualStyle"] === "gimp") && (!Object.hasOwn(value, "photoProPalette") || isPanelPhotoProPalette(value["photoProPalette"])) && Object.hasOwn(value, "accentColor") && (typeof value["accentColor"] === "string" && value["accentColor"].length <= 7 && new RegExp("^#[a-fA-F0-9]{6}$").test(value["accentColor"]) || value["accentColor"] === null) && Object.hasOwn(value, "resizeCornerSize") && typeof value["resizeCornerSize"] === "number" && Number.isSafeInteger(value["resizeCornerSize"]) && value["resizeCornerSize"] >= 0 && value["resizeCornerSize"] <= 64;
}
function isPanelReady(value) {
  return isRecord(value) && Object.keys(value).length === 4 && Object.hasOwn(value, "protocol") && value["protocol"] === "sugarcubes.panel" && Object.hasOwn(value, "version") && value["version"] === 3 && Object.hasOwn(value, "session") && typeof value["session"] === "string" && value["session"].length <= 80 && new RegExp("^[a-zA-Z0-9-]{16,80}$").test(value["session"]) && Object.hasOwn(value, "type") && value["type"] === "ready";
}
function isPanelConfigure(value) {
  return isRecord(value) && Object.keys(value).length === 5 && Object.hasOwn(value, "protocol") && value["protocol"] === "sugarcubes.panel" && Object.hasOwn(value, "version") && value["version"] === 3 && Object.hasOwn(value, "session") && typeof value["session"] === "string" && value["session"].length <= 80 && new RegExp("^[a-zA-Z0-9-]{16,80}$").test(value["session"]) && Object.hasOwn(value, "type") && value["type"] === "configure" && Object.hasOwn(value, "presentation") && isPanelPresentation(value["presentation"]);
}
function isPanelApplied(value) {
  return isRecord(value) && Object.keys(value).length === 4 && Object.hasOwn(value, "protocol") && value["protocol"] === "sugarcubes.panel" && Object.hasOwn(value, "version") && value["version"] === 3 && Object.hasOwn(value, "session") && typeof value["session"] === "string" && value["session"].length <= 80 && new RegExp("^[a-zA-Z0-9-]{16,80}$").test(value["session"]) && Object.hasOwn(value, "type") && value["type"] === "applied";
}
function isPanelPing(value) {
  return isRecord(value) && Object.keys(value).length === 4 && Object.hasOwn(value, "protocol") && value["protocol"] === "sugarcubes.panel" && Object.hasOwn(value, "version") && value["version"] === 3 && Object.hasOwn(value, "session") && typeof value["session"] === "string" && value["session"].length <= 80 && new RegExp("^[a-zA-Z0-9-]{16,80}$").test(value["session"]) && Object.hasOwn(value, "type") && value["type"] === "ping";
}
function isPanelPong(value) {
  return isRecord(value) && Object.keys(value).length === 5 && Object.hasOwn(value, "protocol") && value["protocol"] === "sugarcubes.panel" && Object.hasOwn(value, "version") && value["version"] === 3 && Object.hasOwn(value, "session") && typeof value["session"] === "string" && value["session"].length <= 80 && new RegExp("^[a-zA-Z0-9-]{16,80}$").test(value["session"]) && Object.hasOwn(value, "type") && value["type"] === "pong" && Object.hasOwn(value, "connected") && typeof value["connected"] === "boolean";
}
function isPanelRejected(value) {
  return isRecord(value) && Object.keys(value).length === 5 && Object.hasOwn(value, "protocol") && value["protocol"] === "sugarcubes.panel" && Object.hasOwn(value, "version") && value["version"] === 3 && Object.hasOwn(value, "session") && typeof value["session"] === "string" && value["session"].length <= 80 && new RegExp("^[a-zA-Z0-9-]{16,80}$").test(value["session"]) && Object.hasOwn(value, "type") && value["type"] === "rejected" && Object.hasOwn(value, "reason") && (value["reason"] === "unsupported-presentation" || value["reason"] === "presentation-failed");
}
function isPanelMessage(value) {
  return isPanelReady(value) || isPanelConfigure(value) || isPanelApplied(value) || isPanelPing(value) || isPanelPong(value) || isPanelRejected(value);
}

// integrations/shared/PanelPresentation.ts
function samePanelPresentation(left, right) {
  if (!left || !right) return left === right;
  return left.pluginMode === right.pluginMode && left.visualStyle === right.visualStyle && left.photoProPalette === right.photoProPalette && left.accentColor === right.accentColor && left.resizeCornerSize === right.resizeCornerSize;
}
function freezePanelPresentation(value) {
  return Object.freeze({ ...value });
}

// integrations/contracts/HostDescription.ts
var HOST_PROTOCOLS = Object.freeze({
  panel: Object.freeze({ name: "sugarcubes.panel", version: 3 }),
  canvas: Object.freeze({ name: "sugarcubes.canvas", version: 3 }),
  canvasMasks: Object.freeze({ name: "sugarcubes.canvas.masks", version: 2 }),
  memory: Object.freeze({ name: "sugarcubes.memory", version: 1 }),
  updates: Object.freeze({ name: "sugarcubes.updates", version: 1 })
});
var HOST_CAPABILITIES = Object.freeze({
  /** The host's configured presentation replaces the user's saved appearance while embedded. */
  hostStyles: "presentation.host-styles",
  /** The host's configured accent is the system accent, offered as a choice beside a custom one. */
  systemAccent: "presentation.system-accent",
  /** The host presents output images itself; the page never falls back to browser images. */
  hostOutputSurface: "output.host-surface"
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

// integrations/shared/PanelSession.ts
var PanelClientSession = class {
  /** Bind one application-neutral client to an externally verified transport session. */
  constructor(session, port) {
    this.session = session;
    this.port = port;
  }
  #applied = false;
  /** Announce readiness until the native panel confirms presentation configuration. */
  ready() {
    if (!this.#applied) this.#send({ type: "ready" });
  }
  /** Apply validated configuration or answer a heartbeat after presentation is accepted. */
  receive(value) {
    if (!isPanelMessage(value) || value.session !== this.session) return false;
    if (value.type === "configure") {
      try {
        if (!this.port.apply(value.presentation)) {
          this.#send({ type: "rejected", reason: "unsupported-presentation" });
          return true;
        }
        this.#applied = true;
        this.#send({ type: "applied" });
      } catch (error) {
        this.port.report(error);
        this.#send({ type: "rejected", reason: "presentation-failed" });
      }
    } else if (value.type === "ping" && this.#applied)
      this.#send({ type: "pong", connected: this.port.connected() });
    else return false;
    return true;
  }
  /** Attach authoritative client metadata to schema-checked response kinds. */
  #send(message) {
    this.port.send({ protocol: "sugarcubes.panel", version: 3, session: this.session, ...message });
  }
};

// integrations/shared/HostValidation.generated.ts
function isRecord2(value) {
  return !!value && typeof value === "object" && !Array.isArray(value);
}
function isHostCapability(value) {
  return isRecord2(value) && Object.keys(value).length === 1 && Object.hasOwn(value, "name") && typeof value["name"] === "string" && value["name"].length <= 64 && new RegExp("^[a-z0-9.-]{1,64}$").test(value["name"]);
}
function isHostProtocol(value) {
  return isRecord2(value) && Object.keys(value).length === 2 && Object.hasOwn(value, "name") && typeof value["name"] === "string" && value["name"].length <= 64 && new RegExp("^[a-z0-9.-]{1,64}$").test(value["name"]) && Object.hasOwn(value, "version") && typeof value["version"] === "number" && Number.isSafeInteger(value["version"]) && value["version"] >= 1 && value["version"] <= 65535;
}
function isHostDescriptionMessage(value) {
  return isRecord2(value) && Object.keys(value).length === 6 && Object.hasOwn(value, "protocol") && value["protocol"] === "sugarcubes.host" && Object.hasOwn(value, "version") && value["version"] === 1 && Object.hasOwn(value, "session") && typeof value["session"] === "string" && value["session"].length <= 80 && new RegExp("^[a-zA-Z0-9-]{16,80}$").test(value["session"]) && Object.hasOwn(value, "type") && value["type"] === "description" && Object.hasOwn(value, "protocols") && Array.isArray(value["protocols"]) && value["protocols"].length <= 32 && value["protocols"].every((item) => isHostProtocol(item)) && Object.hasOwn(value, "capabilities") && Array.isArray(value["capabilities"]) && value["capabilities"].length <= 64 && value["capabilities"].every((item) => isHostCapability(item));
}
export {
  DeclaredHost,
  HOST_CAPABILITIES,
  HOST_PROTOCOLS,
  PanelClientSession,
  freezePanelPresentation,
  isHostDescriptionMessage,
  samePanelPresentation
};
