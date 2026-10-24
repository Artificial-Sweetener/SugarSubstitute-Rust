// integrations/shared/CanvasValidation.generated.ts
function isRecord(value) {
  return !!value && typeof value === "object" && !Array.isArray(value);
}
function isCanvasCapabilities(value) {
  return isRecord(value) && Object.keys(value).length === 2 && Object.hasOwn(value, "displayOutput") && typeof value["displayOutput"] === "boolean" && Object.hasOwn(value, "insertImageLayer") && typeof value["insertImageLayer"] === "boolean";
}
function isCanvasDescriptor(value) {
  return isRecord(value) && Object.keys(value).length === 3 && Object.hasOwn(value, "id") && typeof value["id"] === "string" && value["id"].length <= 128 && Object.hasOwn(value, "label") && typeof value["label"] === "string" && value["label"].length <= 128 && Object.hasOwn(value, "capabilities") && isCanvasCapabilities(value["capabilities"]);
}
function isCanvasDocument(value) {
  return isRecord(value) && Object.keys(value).length === 4 && Object.hasOwn(value, "id") && typeof value["id"] === "string" && value["id"].length <= 128 && Object.hasOwn(value, "geometryRevision") && typeof value["geometryRevision"] === "string" && value["geometryRevision"].length <= 512 && Object.hasOwn(value, "width") && typeof value["width"] === "number" && Number.isSafeInteger(value["width"]) && value["width"] >= 1 && value["width"] <= 3e5 && Object.hasOwn(value, "height") && typeof value["height"] === "number" && Number.isSafeInteger(value["height"]) && value["height"] >= 1 && value["height"] <= 3e5;
}
function isCanvasFailure(value) {
  return isRecord(value) && Object.keys(value).length === 2 && Object.hasOwn(value, "status") && (value["status"] === "unavailable" || value["status"] === "unsupported" || value["status"] === "stale" || value["status"] === "cancelled" || value["status"] === "error") && Object.hasOwn(value, "message") && typeof value["message"] === "string" && value["message"].length <= 512;
}
function isCanvasInsertedLayer(value) {
  return isRecord(value) && Object.keys(value).length === 7 && Object.hasOwn(value, "status") && value["status"] === "inserted" && Object.hasOwn(value, "documentId") && typeof value["documentId"] === "string" && value["documentId"].length <= 128 && Object.hasOwn(value, "layerId") && typeof value["layerId"] === "string" && value["layerId"].length <= 128 && Object.hasOwn(value, "left") && typeof value["left"] === "number" && Number.isSafeInteger(value["left"]) && value["left"] >= -3e5 && value["left"] <= 3e5 && Object.hasOwn(value, "top") && typeof value["top"] === "number" && Number.isSafeInteger(value["top"]) && value["top"] >= -3e5 && value["top"] <= 3e5 && Object.hasOwn(value, "width") && typeof value["width"] === "number" && Number.isSafeInteger(value["width"]) && value["width"] >= 1 && value["width"] <= 3e5 && Object.hasOwn(value, "height") && typeof value["height"] === "number" && Number.isSafeInteger(value["height"]) && value["height"] >= 1 && value["height"] <= 3e5;
}
function isCanvasInsertResult(value) {
  return isCanvasInsertedLayer(value) || isCanvasFailure(value);
}
function isCanvasContext(value) {
  return isRecord(value) && Object.keys(value).length === 2 && Object.hasOwn(value, "canvas") && isCanvasDescriptor(value["canvas"]) && Object.hasOwn(value, "document") && (isCanvasDocument(value["document"]) || value["document"] === null);
}
function isCanvasInsertImage(value) {
  return isRecord(value) && Object.keys(value).length === 3 && Object.hasOwn(value, "document") && isCanvasDocument(value["document"]) && Object.hasOwn(value, "image") && isCanvasEncodedImage(value["image"]) && Object.hasOwn(value, "layerName") && typeof value["layerName"] === "string" && value["layerName"].length <= 128;
}
function isCanvasReadRequest(value) {
  return isRecord(value) && Object.keys(value).length === 5 && Object.hasOwn(value, "protocol") && value["protocol"] === "sugarcubes.canvas" && Object.hasOwn(value, "version") && value["version"] === 1 && Object.hasOwn(value, "session") && typeof value["session"] === "string" && value["session"].length <= 80 && new RegExp("^[a-zA-Z0-9-]{16,80}$").test(value["session"]) && Object.hasOwn(value, "requestId") && typeof value["requestId"] === "number" && Number.isSafeInteger(value["requestId"]) && value["requestId"] >= 1 && value["requestId"] <= 2147483647 && Object.hasOwn(value, "type") && value["type"] === "context.request";
}
function isCanvasReadReply(value) {
  return isRecord(value) && Object.keys(value).length === 6 && Object.hasOwn(value, "protocol") && value["protocol"] === "sugarcubes.canvas" && Object.hasOwn(value, "version") && value["version"] === 1 && Object.hasOwn(value, "session") && typeof value["session"] === "string" && value["session"].length <= 80 && new RegExp("^[a-zA-Z0-9-]{16,80}$").test(value["session"]) && Object.hasOwn(value, "requestId") && typeof value["requestId"] === "number" && Number.isSafeInteger(value["requestId"]) && value["requestId"] >= 1 && value["requestId"] <= 2147483647 && Object.hasOwn(value, "type") && value["type"] === "context.reply" && Object.hasOwn(value, "context") && isCanvasContext(value["context"]);
}
function isCanvasInsertRequest(value) {
  return isRecord(value) && Object.keys(value).length === 6 && Object.hasOwn(value, "protocol") && value["protocol"] === "sugarcubes.canvas" && Object.hasOwn(value, "version") && value["version"] === 1 && Object.hasOwn(value, "session") && typeof value["session"] === "string" && value["session"].length <= 80 && new RegExp("^[a-zA-Z0-9-]{16,80}$").test(value["session"]) && Object.hasOwn(value, "requestId") && typeof value["requestId"] === "number" && Number.isSafeInteger(value["requestId"]) && value["requestId"] >= 1 && value["requestId"] <= 2147483647 && Object.hasOwn(value, "type") && value["type"] === "image.insert" && Object.hasOwn(value, "image") && isCanvasInsertImage(value["image"]);
}
function isCanvasInsertReply(value) {
  return isRecord(value) && Object.keys(value).length === 6 && Object.hasOwn(value, "protocol") && value["protocol"] === "sugarcubes.canvas" && Object.hasOwn(value, "version") && value["version"] === 1 && Object.hasOwn(value, "session") && typeof value["session"] === "string" && value["session"].length <= 80 && new RegExp("^[a-zA-Z0-9-]{16,80}$").test(value["session"]) && Object.hasOwn(value, "requestId") && typeof value["requestId"] === "number" && Number.isSafeInteger(value["requestId"]) && value["requestId"] >= 1 && value["requestId"] <= 2147483647 && Object.hasOwn(value, "type") && value["type"] === "image.reply" && Object.hasOwn(value, "result") && isCanvasInsertResult(value["result"]);
}
function isCanvasCancelRequest(value) {
  return isRecord(value) && Object.keys(value).length === 5 && Object.hasOwn(value, "protocol") && value["protocol"] === "sugarcubes.canvas" && Object.hasOwn(value, "version") && value["version"] === 1 && Object.hasOwn(value, "session") && typeof value["session"] === "string" && value["session"].length <= 80 && new RegExp("^[a-zA-Z0-9-]{16,80}$").test(value["session"]) && Object.hasOwn(value, "requestId") && typeof value["requestId"] === "number" && Number.isSafeInteger(value["requestId"]) && value["requestId"] >= 1 && value["requestId"] <= 2147483647 && Object.hasOwn(value, "type") && value["type"] === "cancel";
}
function isCanvasErrorReply(value) {
  return isRecord(value) && Object.keys(value).length === 6 && Object.hasOwn(value, "protocol") && value["protocol"] === "sugarcubes.canvas" && Object.hasOwn(value, "version") && value["version"] === 1 && Object.hasOwn(value, "session") && typeof value["session"] === "string" && value["session"].length <= 80 && new RegExp("^[a-zA-Z0-9-]{16,80}$").test(value["session"]) && Object.hasOwn(value, "requestId") && typeof value["requestId"] === "number" && Number.isSafeInteger(value["requestId"]) && value["requestId"] >= 1 && value["requestId"] <= 2147483647 && Object.hasOwn(value, "type") && value["type"] === "error.reply" && Object.hasOwn(value, "failure") && isCanvasFailure(value["failure"]);
}
function isCanvasEncodedImage(value) {
  return isRecord(value) && Object.keys(value).length === 3 && Object.hasOwn(value, "format") && value["format"] === "png" && Object.hasOwn(value, "encoding") && value["encoding"] === "base64" && Object.hasOwn(value, "data") && typeof value["data"] === "string" && value["data"].length <= 89478488;
}
function isCanvasSourceTarget(value) {
  return isRecord(value) && Object.keys(value).length === 1 && Object.hasOwn(value, "kind") && value["kind"] === "selection" || isRecord(value) && Object.keys(value).length === 1 && Object.hasOwn(value, "kind") && value["kind"] === "current-layer" || isRecord(value) && Object.keys(value).length === 2 && Object.hasOwn(value, "kind") && value["kind"] === "layer" && Object.hasOwn(value, "layerId") && typeof value["layerId"] === "string" && value["layerId"].length <= 128 || isRecord(value) && Object.keys(value).length === 1 && Object.hasOwn(value, "kind") && value["kind"] === "composition" || isRecord(value) && Object.keys(value).length === 2 && Object.hasOwn(value, "kind") && value["kind"] === "composition-from-layer" && Object.hasOwn(value, "layerId") && typeof value["layerId"] === "string" && value["layerId"].length <= 128;
}
function isCanvasInputTarget(value) {
  return isCanvasSourceTarget(value) || isRecord(value) && Object.keys(value).length === 2 && Object.hasOwn(value, "kind") && value["kind"] === "file" && Object.hasOwn(value, "file") && (typeof value["file"] === "string" && value["file"].length <= 4096 || value["file"] === null);
}
function isCanvasPixelRaster(value) {
  return isRecord(value) && Object.keys(value).length === 4 && Object.hasOwn(value, "width") && typeof value["width"] === "number" && Number.isSafeInteger(value["width"]) && value["width"] >= 1 && value["width"] <= 65536 && Object.hasOwn(value, "height") && typeof value["height"] === "number" && Number.isSafeInteger(value["height"]) && value["height"] >= 1 && value["height"] <= 65536 && Object.hasOwn(value, "format") && (value["format"] === "rgba8" || value["format"] === "coverage8") && Object.hasOwn(value, "data") && typeof value["data"] === "string" && value["data"].length <= 89478488;
}
function isCanvasSourceLayer(value) {
  return isRecord(value) && Object.keys(value).length === 5 && Object.hasOwn(value, "id") && typeof value["id"] === "string" && value["id"].length <= 128 && Object.hasOwn(value, "name") && typeof value["name"] === "string" && value["name"].length <= 256 && Object.hasOwn(value, "parentId") && (typeof value["parentId"] === "string" && value["parentId"].length <= 128 || value["parentId"] === null) && Object.hasOwn(value, "depth") && typeof value["depth"] === "number" && Number.isSafeInteger(value["depth"]) && value["depth"] >= 0 && value["depth"] <= 128 && Object.hasOwn(value, "visible") && typeof value["visible"] === "boolean";
}
function isCanvasSourceContext(value) {
  return isRecord(value) && Object.keys(value).length === 7 && Object.hasOwn(value, "kind") && value["kind"] === "context" && Object.hasOwn(value, "supported") && typeof value["supported"] === "boolean" && Object.hasOwn(value, "document") && (isCanvasDocument(value["document"]) || value["document"] === null) && Object.hasOwn(value, "revision") && typeof value["revision"] === "string" && value["revision"].length <= 512 && Object.hasOwn(value, "targets") && Array.isArray(value["targets"]) && value["targets"].length <= 5 && value["targets"].every(
    (item) => item === "selection" || item === "current-layer" || item === "layer" || item === "composition" || item === "composition-from-layer"
  ) && Object.hasOwn(value, "layers") && Array.isArray(value["layers"]) && value["layers"].length <= 4096 && value["layers"].every((item) => isCanvasSourceLayer(item)) && Object.hasOwn(value, "currentLayerId") && (typeof value["currentLayerId"] === "string" && value["currentLayerId"].length <= 128 || value["currentLayerId"] === null);
}
function isCanvasSourceSnapshot(value) {
  return isRecord(value) && Object.keys(value).length === 4 && Object.hasOwn(value, "target") && isCanvasSourceTarget(value["target"]) && Object.hasOwn(value, "document") && isCanvasDocument(value["document"]) && Object.hasOwn(value, "revision") && typeof value["revision"] === "string" && value["revision"].length <= 512 && Object.hasOwn(value, "pixels") && isCanvasPixelRaster(value["pixels"]);
}
function isCanvasSourceCommand(value) {
  return isCanvasSourceQuery(value) || isRecord(value) && Object.keys(value).length === 2 && Object.hasOwn(value, "type") && value["type"] === "watch" && Object.hasOwn(value, "enabled") && typeof value["enabled"] === "boolean";
}
function isCanvasSourceResult(value) {
  return isCanvasSourceContext(value) || isRecord(value) && Object.keys(value).length === 2 && Object.hasOwn(value, "kind") && value["kind"] === "captured" && Object.hasOwn(value, "snapshots") && Array.isArray(value["snapshots"]) && value["snapshots"].length <= 64 && value["snapshots"].every((item) => isCanvasSourceSnapshot(item)) || isRecord(value) && Object.keys(value).length === 1 && Object.hasOwn(value, "kind") && value["kind"] === "watching" || isRecord(value) && Object.keys(value).length === 3 && Object.hasOwn(value, "kind") && value["kind"] === "failure" && Object.hasOwn(value, "status") && (value["status"] === "unavailable" || value["status"] === "unsupported" || value["status"] === "stale" || value["status"] === "cancelled" || value["status"] === "error") && Object.hasOwn(value, "message") && typeof value["message"] === "string" && value["message"].length <= 512;
}
function isCanvasSourceRequest(value) {
  return isRecord(value) && Object.keys(value).length === 6 && Object.hasOwn(value, "protocol") && value["protocol"] === "sugarcubes.canvas" && Object.hasOwn(value, "version") && value["version"] === 1 && Object.hasOwn(value, "session") && typeof value["session"] === "string" && value["session"].length <= 80 && new RegExp("^[a-zA-Z0-9-]{16,80}$").test(value["session"]) && Object.hasOwn(value, "requestId") && typeof value["requestId"] === "number" && Number.isSafeInteger(value["requestId"]) && value["requestId"] >= 1 && value["requestId"] <= 2147483647 && Object.hasOwn(value, "type") && value["type"] === "source.request" && Object.hasOwn(value, "command") && isCanvasSourceCommand(value["command"]);
}
function isCanvasSourceReply(value) {
  return isRecord(value) && Object.keys(value).length === 6 && Object.hasOwn(value, "protocol") && value["protocol"] === "sugarcubes.canvas" && Object.hasOwn(value, "version") && value["version"] === 1 && Object.hasOwn(value, "session") && typeof value["session"] === "string" && value["session"].length <= 80 && new RegExp("^[a-zA-Z0-9-]{16,80}$").test(value["session"]) && Object.hasOwn(value, "requestId") && typeof value["requestId"] === "number" && Number.isSafeInteger(value["requestId"]) && value["requestId"] >= 1 && value["requestId"] <= 2147483647 && Object.hasOwn(value, "type") && value["type"] === "source.reply" && Object.hasOwn(value, "result") && isCanvasSourceResult(value["result"]);
}
function isCanvasSourceChanged(value) {
  return isRecord(value) && Object.keys(value).length === 6 && Object.hasOwn(value, "protocol") && value["protocol"] === "sugarcubes.canvas" && Object.hasOwn(value, "version") && value["version"] === 1 && Object.hasOwn(value, "session") && typeof value["session"] === "string" && value["session"].length <= 80 && new RegExp("^[a-zA-Z0-9-]{16,80}$").test(value["session"]) && Object.hasOwn(value, "requestId") && value["requestId"] === 0 && Object.hasOwn(value, "type") && value["type"] === "source.changed" && Object.hasOwn(value, "change") && isCanvasSourceChange(value["change"]);
}
function isCanvasSourceChange(value) {
  return isRecord(value) && Object.keys(value).length === 2 && Object.hasOwn(value, "documentId") && (typeof value["documentId"] === "string" && value["documentId"].length <= 128 || value["documentId"] === null) && Object.hasOwn(value, "revision") && typeof value["revision"] === "string" && value["revision"].length <= 512;
}
function isCanvasSourceQuery(value) {
  return isRecord(value) && Object.keys(value).length === 1 && Object.hasOwn(value, "type") && value["type"] === "context" || isRecord(value) && Object.keys(value).length === 4 && Object.hasOwn(value, "type") && value["type"] === "capture" && Object.hasOwn(value, "document") && isCanvasDocument(value["document"]) && Object.hasOwn(value, "targets") && Array.isArray(value["targets"]) && value["targets"].length <= 64 && value["targets"].every((item) => isCanvasSourceTarget(item)) && Object.hasOwn(value, "maximumSize") && typeof value["maximumSize"] === "number" && Number.isSafeInteger(value["maximumSize"]) && value["maximumSize"] >= 0 && value["maximumSize"] <= 1024;
}
function isCanvasInputSource(value) {
  return isRecord(value) && Object.keys(value).length === 2 && Object.hasOwn(value, "kind") && value["kind"] === "file" && Object.hasOwn(value, "file") && (typeof value["file"] === "string" && value["file"].length <= 4096 || value["file"] === null) || isRecord(value) && Object.keys(value).length === 4 && Object.hasOwn(value, "kind") && value["kind"] === "canvas" && Object.hasOwn(value, "canvasId") && typeof value["canvasId"] === "string" && value["canvasId"].length <= 128 && Object.hasOwn(value, "documentId") && (typeof value["documentId"] === "string" && value["documentId"].length <= 128 || value["documentId"] === null) && Object.hasOwn(value, "target") && isCanvasSourceTarget(value["target"]);
}
function isCanvasMessage(value) {
  return isCanvasReadRequest(value) || isCanvasReadReply(value) || isCanvasInsertRequest(value) || isCanvasInsertReply(value) || isCanvasCancelRequest(value) || isCanvasErrorReply(value) || isCanvasSourceRequest(value) || isCanvasSourceReply(value) || isCanvasSourceChanged(value);
}

// integrations/shared/CanvasSourceConnection.ts
var CanvasSourceConnection = class {
  /** Borrow the session transport without creating a second request-ID sequence. */
  constructor(port) {
    this.port = port;
  }
  #listeners = /* @__PURE__ */ new Set();
  #stop = null;
  #disposed = false;
  #context = null;
  /** Resolve identity from the actual connected canvas rather than an editor-specific name. */
  get canvasId() {
    return this.port.identity();
  }
  /** Share simultaneous metadata reads while keeping later reads and captures current. */
  execute(command) {
    if (this.#disposed)
      return Promise.resolve({
        kind: "failure",
        status: "unavailable",
        message: "Canvas source connection closed."
      });
    if (command.type !== "context") return this.port.execute(command);
    if (this.#context) return this.#context;
    const pending = this.port.execute(command).finally(() => {
      if (this.#context === pending) this.#context = null;
    });
    this.#context = pending;
    return pending;
  }
  /** Share one native watch across all interested views and application collaborators. */
  subscribe(listener) {
    if (this.#disposed) return () => {
    };
    this.#listeners.add(listener);
    if (!this.#stop) {
      this.#stop = this.port.listen((change) => {
        this.#context = null;
        for (const notify of this.#listeners) notify(change);
      });
      this.#watch(true);
    }
    return () => {
      this.#listeners.delete(listener);
      if (!this.#listeners.size && this.#stop) {
        this.#stop();
        this.#stop = null;
        this.#watch(false);
      }
    };
  }
  /** Release observation before the shared request client tears down its pending work. */
  dispose() {
    if (this.#disposed) return;
    const watching = this.#stop !== null;
    this.#disposed = true;
    this.#context = null;
    this.#stop?.();
    this.#stop = null;
    this.#listeners.clear();
    if (watching) this.#watch(false);
  }
  /** Report live transport failures while discarding obsolete teardown feedback. */
  #watch(enabled) {
    void this.port.execute({ type: "watch", enabled }).then((result) => {
      if (!this.#disposed && result.kind === "failure")
        this.port.report(new Error(result.message));
    }).catch((error) => {
      if (!this.#disposed) this.port.report(error);
    });
  }
};

// integrations/shared/CanvasClient.ts
var CanvasClient = class {
  /** Bind an authenticated session and injected clock without claiming its transport lifetime. */
  constructor(session, port) {
    this.session = session;
    this.port = port;
    this.sources = new CanvasSourceConnection({
      identity: () => this.#context?.canvas.id ?? "",
      execute: (command) => this.#sourceRequest(command),
      listen: (listener) => {
        this.#sourceListeners.add(listener);
        return () => {
          this.#sourceListeners.delete(listener);
        };
      },
      report: (error) => {
        this.port.report(error);
      }
    });
  }
  sources;
  #pending = /* @__PURE__ */ new Map();
  #sourceListeners = /* @__PURE__ */ new Set();
  #next = 0;
  #context = null;
  #disposed = false;
  /** Discover only capabilities actually reported by this canvas implementation. */
  readCanvas() {
    return this.#context?.canvas ?? null;
  }
  /** Read the current document before preparing an insertion against that exact identity. */
  async readContext() {
    const reply = await this.#request({ type: "context.request" }, "context.reply");
    if (reply.type !== "context.reply") throw new Error("Invalid canvas context reply.");
    this.#context = reply.context;
    return reply.context;
  }
  /** Insert once using the caller's captured destination rather than a later active document. */
  async insertImage(image, cancellation) {
    if (cancellation?.cancelled())
      return { status: "cancelled", message: "Canvas insertion cancelled." };
    const reply = await this.#request({ type: "image.insert", image }, "image.reply", cancellation);
    if (reply.type !== "image.reply") throw new Error("Invalid canvas image reply.");
    return reply.result;
  }
  /** Accept only the expected reply kind from this session and retire its deadline before callbacks run. */
  receive(value) {
    if (this.#disposed || !isCanvasMessage(value) || value.session !== this.session) return false;
    if (value.type === "source.changed") {
      for (const listener of this.#sourceListeners) listener(value.change);
      return true;
    }
    const pending = this.#pending.get(value.requestId);
    if (!pending || pending.type !== value.type && value.type !== "error.reply") return false;
    this.#pending.delete(value.requestId);
    pending.stop();
    if (value.type === "error.reply") pending.reject(new Error(value.failure.message));
    else pending.accept(value);
    return true;
  }
  /** Refuse all late responses and cancel outstanding host work when the session leaves. */
  dispose() {
    if (this.#disposed) return;
    this.sources.dispose();
    this.#disposed = true;
    for (const [id, pending] of this.#pending) {
      pending.stop();
      if (pending.type === "image.reply") this.#cancel(id);
      pending.reject(new Error("Canvas connection closed."));
    }
    this.#pending.clear();
    this.#context = null;
    this.#sourceListeners.clear();
  }
  /** Translate source query failures without introducing another transport or request sequence. */
  async #sourceRequest(command) {
    try {
      const reply = await this.#request({ type: "source.request", command }, "source.reply");
      if (reply.type !== "source.reply") throw new Error("Invalid canvas source reply.");
      return reply.result;
    } catch (error) {
      return {
        kind: "failure",
        status: "unavailable",
        message: error instanceof Error ? error.message.slice(0, 512) : "Canvas source request failed."
      };
    }
  }
  /** Register reply ownership before synchronous transports can answer, and bound every request. */
  #request(request, reply, cancellation) {
    if (this.#disposed) return Promise.reject(new Error("Canvas connection closed."));
    const requestId = ++this.#next;
    return new Promise((resolve, reject) => {
      let stopDeadline = () => {
      };
      let stopCancellation = () => {
      };
      const stop = () => {
        stopDeadline();
        stopCancellation();
      };
      this.#pending.set(requestId, { type: reply, accept: resolve, reject, stop });
      stopDeadline = this.port.later(3e4, () => {
        this.#pending.delete(requestId);
        stop();
        if (request.type === "image.insert") this.#cancel(requestId);
        reject(new Error("Canvas request timed out; it was not retried."));
      });
      stopCancellation = cancellation?.subscribe(() => {
        const pending = this.#pending.get(requestId);
        if (!pending) return;
        this.#pending.delete(requestId);
        stop();
        this.#cancel(requestId);
        resolve({
          protocol: "sugarcubes.canvas",
          version: 1,
          session: this.session,
          requestId,
          type: "image.reply",
          result: { status: "cancelled", message: "Canvas insertion cancelled." }
        });
      }) ?? (() => {
      });
      if (!this.#pending.has(requestId)) return;
      try {
        const envelope = {
          protocol: "sugarcubes.canvas",
          version: 1,
          session: this.session,
          requestId
        };
        this.port.send({ ...envelope, ...request });
      } catch (error) {
        this.#pending.delete(requestId);
        stop();
        reject(
          error instanceof Error ? error : new Error("Canvas transport failed.", { cause: error })
        );
      }
    });
  }
  /** Signal cancellation without inventing a new mutation identity. */
  #cancel(requestId) {
    try {
      this.port.send({
        protocol: "sugarcubes.canvas",
        version: 1,
        session: this.session,
        requestId,
        type: "cancel"
      });
    } catch (error) {
      this.port.report(error);
    }
  }
};

// integrations/shared/CanvasImagePolicy.ts
var MAX_CANVAS_IMAGE_BYTES = 64 * 1024 * 1024;
function canvasPngDimensions(bytes) {
  const signature = [137, 80, 78, 71, 13, 10, 26, 10];
  if (bytes.length < 33 || bytes.length > MAX_CANVAS_IMAGE_BYTES || signature.some((byte, index) => bytes[index] !== byte) || bytes[12] !== 73 || bytes[13] !== 72 || bytes[14] !== 68 || bytes[15] !== 82)
    throw new Error("Canvas insertion requires a valid bounded PNG image.");
  const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
  const width = view.getUint32(16, false), height = view.getUint32(20, false);
  if (width < 1 || height < 1 || width > 3e4 || height > 3e4 || width * height > 64 * 1024 * 1024)
    throw new Error("This image exceeds the canvas insertion pixel budget.");
  return Object.freeze({ width, height });
}

// integrations/shared/CanvasByteEncoding.ts
var ALPHABET = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
function encodeCanvasBytes(bytes) {
  const blocks = [];
  let block = "";
  for (let index = 0; index < bytes.length; index += 3) {
    const first = bytes[index] ?? 0, second = bytes[index + 1] ?? 0, third = bytes[index + 2] ?? 0;
    block += ALPHABET.charAt(first >> 2) + ALPHABET.charAt((first & 3) << 4 | second >> 4) + (index + 1 < bytes.length ? ALPHABET.charAt((second & 15) << 2 | third >> 6) : "=") + (index + 2 < bytes.length ? ALPHABET.charAt(third & 63) : "=");
    if (block.length >= 16384) {
      blocks.push(block);
      block = "";
    }
  }
  blocks.push(block);
  return blocks.join("");
}
function decodeCanvasBytes(data, maximumBytes) {
  if (!data.length || data.length > Math.ceil(maximumBytes / 3) * 4 || data.length % 4 !== 0)
    throw new Error("Invalid canvas byte encoding.");
  const padding = data.endsWith("==") ? 2 : data.endsWith("=") ? 1 : 0;
  const size = data.length / 4 * 3 - padding;
  if (size > maximumBytes) throw new Error("Canvas image transfer exceeds its byte budget.");
  const bytes = new Uint8Array(size);
  let offset = 0;
  for (let index = 0; index < data.length; index += 4) {
    const a = ALPHABET.indexOf(data.charAt(index)), b = ALPHABET.indexOf(data.charAt(index + 1));
    const c = ALPHABET.indexOf(data.charAt(index + 2)), d = ALPHABET.indexOf(data.charAt(index + 3));
    const final = index + 4 === data.length;
    if (a < 0 || b < 0 || !final && (c < 0 || d < 0) || final && (padding === 0 ? c < 0 || d < 0 : padding === 1 ? c < 0 || d !== -1 || (c & 3) !== 0 : c !== -1 || d !== -1 || (b & 15) !== 0))
      throw new Error("Invalid canvas byte encoding.");
    bytes[offset++] = a << 2 | b >> 4;
    if (offset < size) bytes[offset++] = (b & 15) << 4 | c >> 2;
    if (offset < size) bytes[offset++] = (c & 3) << 6 | d;
  }
  return bytes;
}

// integrations/shared/CanvasImageEncoding.ts
var MAX_CANVAS_ENCODED_CHARACTERS = Math.ceil(MAX_CANVAS_IMAGE_BYTES / 3) * 4;
function encodeCanvasImage(bytes) {
  canvasPngDimensions(bytes);
  return { format: "png", encoding: "base64", data: encodeCanvasBytes(bytes) };
}

// integrations/shared/CanvasCancellation.ts
var CanvasCancellation = class {
  #cancelled = false;
  #listeners = /* @__PURE__ */ new Set();
  /** Read cancellation before allocating a resource or committing document changes. */
  cancelled() {
    return this.#cancelled;
  }
  /** Observe cancellation and immediately notify late subscribers to an already cancelled request. */
  subscribe(listener) {
    if (this.#cancelled) {
      listener();
      return () => {
      };
    }
    this.#listeners.add(listener);
    return () => this.#listeners.delete(listener);
  }
  /** Cancel once and retire every observer before invoking its callback. */
  cancel() {
    if (this.#cancelled) return;
    this.#cancelled = true;
    const listeners = [...this.#listeners];
    this.#listeners.clear();
    for (const listener of listeners) listener();
  }
};

// integrations/contracts/CanvasHost.ts
var WEB_CANVAS = Object.freeze({
  id: "web",
  label: "SugarCubes Web",
  capabilities: Object.freeze({ displayOutput: true, insertImageLayer: false })
});

// integrations/shared/CanvasMasksValidation.generated.ts
function isRecord2(value) {
  return !!value && typeof value === "object" && !Array.isArray(value);
}
function isCanvasDocument2(value) {
  return isRecord2(value) && Object.keys(value).length === 4 && Object.hasOwn(value, "id") && typeof value["id"] === "string" && value["id"].length <= 128 && Object.hasOwn(value, "geometryRevision") && typeof value["geometryRevision"] === "string" && value["geometryRevision"].length <= 512 && Object.hasOwn(value, "width") && typeof value["width"] === "number" && Number.isSafeInteger(value["width"]) && value["width"] >= 1 && value["width"] <= 3e5 && Object.hasOwn(value, "height") && typeof value["height"] === "number" && Number.isSafeInteger(value["height"]) && value["height"] >= 1 && value["height"] <= 3e5;
}
function isCanvasMaskPixels(value) {
  return isRecord2(value) && Object.keys(value).length === 3 && Object.hasOwn(value, "width") && typeof value["width"] === "number" && Number.isSafeInteger(value["width"]) && value["width"] >= 1 && value["width"] <= 65536 && Object.hasOwn(value, "height") && typeof value["height"] === "number" && Number.isSafeInteger(value["height"]) && value["height"] >= 1 && value["height"] <= 65536 && Object.hasOwn(value, "data") && typeof value["data"] === "string" && value["data"].length <= 89478488;
}
function isCanvasMaskReference(value) {
  return isRecord2(value) && Object.keys(value).length === 3 && Object.hasOwn(value, "canvasId") && typeof value["canvasId"] === "string" && value["canvasId"].length <= 128 && Object.hasOwn(value, "document") && isCanvasDocument2(value["document"]) && Object.hasOwn(value, "id") && typeof value["id"] === "string" && value["id"].length <= 128;
}
function isCanvasMaskSnapshot(value) {
  return isRecord2(value) && Object.keys(value).length === 3 && Object.hasOwn(value, "mask") && isCanvasMaskReference(value["mask"]) && Object.hasOwn(value, "revision") && typeof value["revision"] === "string" && value["revision"].length <= 512 && Object.hasOwn(value, "pixels") && isCanvasMaskPixels(value["pixels"]);
}
function isCanvasMaskContext(value) {
  return isRecord2(value) && Object.keys(value).length === 3 && Object.hasOwn(value, "kind") && value["kind"] === "context" && Object.hasOwn(value, "supported") && typeof value["supported"] === "boolean" && Object.hasOwn(value, "document") && (isCanvasDocument2(value["document"]) || value["document"] === null);
}
function isCanvasMaskCreate(value) {
  return isRecord2(value) && Object.keys(value).length === 5 && Object.hasOwn(value, "type") && value["type"] === "create" && Object.hasOwn(value, "document") && isCanvasDocument2(value["document"]) && Object.hasOwn(value, "name") && typeof value["name"] === "string" && value["name"].length <= 128 && Object.hasOwn(value, "source") && (value["source"] === "selection" || value["source"] === "blank" || value["source"] === "pixels") && Object.hasOwn(value, "pixels") && (isCanvasMaskPixels(value["pixels"]) || value["pixels"] === null);
}
function isCanvasMaskActivate(value) {
  return isRecord2(value) && Object.keys(value).length === 2 && Object.hasOwn(value, "type") && value["type"] === "activate" && Object.hasOwn(value, "mask") && isCanvasMaskReference(value["mask"]);
}
function isCanvasMaskCapture(value) {
  return isRecord2(value) && Object.keys(value).length === 3 && Object.hasOwn(value, "type") && value["type"] === "capture" && Object.hasOwn(value, "masks") && Array.isArray(value["masks"]) && value["masks"].length <= 64 && value["masks"].every((item) => isCanvasMaskReference(item)) && Object.hasOwn(value, "maximumSize") && typeof value["maximumSize"] === "number" && Number.isSafeInteger(value["maximumSize"]) && value["maximumSize"] >= 0 && value["maximumSize"] <= 1024;
}
function isCanvasMaskCommand(value) {
  return isRecord2(value) && Object.keys(value).length === 1 && Object.hasOwn(value, "type") && value["type"] === "context" || isCanvasMaskCreate(value) || isCanvasMaskActivate(value) || isCanvasMaskCapture(value) || isRecord2(value) && Object.keys(value).length === 2 && Object.hasOwn(value, "type") && value["type"] === "release" && Object.hasOwn(value, "mask") && (isCanvasMaskReference(value["mask"]) || value["mask"] === null);
}
function isCanvasMaskResult(value) {
  return isCanvasMaskContext(value) || isRecord2(value) && Object.keys(value).length === 2 && Object.hasOwn(value, "kind") && value["kind"] === "created" && Object.hasOwn(value, "mask") && isCanvasMaskReference(value["mask"]) || isRecord2(value) && Object.keys(value).length === 1 && Object.hasOwn(value, "kind") && value["kind"] === "activated" || isRecord2(value) && Object.keys(value).length === 2 && Object.hasOwn(value, "kind") && value["kind"] === "captured" && Object.hasOwn(value, "snapshots") && Array.isArray(value["snapshots"]) && value["snapshots"].length <= 64 && value["snapshots"].every((item) => isCanvasMaskSnapshot(item)) || isRecord2(value) && Object.keys(value).length === 3 && Object.hasOwn(value, "kind") && value["kind"] === "failure" && Object.hasOwn(value, "status") && (value["status"] === "unavailable" || value["status"] === "unsupported" || value["status"] === "stale" || value["status"] === "cancelled" || value["status"] === "error") && Object.hasOwn(value, "message") && typeof value["message"] === "string" && value["message"].length <= 512;
}
function isCanvasMaskRequest(value) {
  return isRecord2(value) && Object.keys(value).length === 5 && Object.hasOwn(value, "protocol") && value["protocol"] === "sugarcubes.canvas.masks" && Object.hasOwn(value, "version") && value["version"] === 1 && Object.hasOwn(value, "session") && typeof value["session"] === "string" && value["session"].length <= 80 && new RegExp("^[a-zA-Z0-9-]{16,80}$").test(value["session"]) && Object.hasOwn(value, "requestId") && typeof value["requestId"] === "number" && Number.isSafeInteger(value["requestId"]) && value["requestId"] >= 1 && value["requestId"] <= 2147483647 && Object.hasOwn(value, "command") && isCanvasMaskCommand(value["command"]);
}
function isCanvasMaskReply(value) {
  return isRecord2(value) && Object.keys(value).length === 5 && Object.hasOwn(value, "protocol") && value["protocol"] === "sugarcubes.canvas.masks" && Object.hasOwn(value, "version") && value["version"] === 1 && Object.hasOwn(value, "session") && typeof value["session"] === "string" && value["session"].length <= 80 && new RegExp("^[a-zA-Z0-9-]{16,80}$").test(value["session"]) && Object.hasOwn(value, "requestId") && typeof value["requestId"] === "number" && Number.isSafeInteger(value["requestId"]) && value["requestId"] >= 1 && value["requestId"] <= 2147483647 && Object.hasOwn(value, "result") && isCanvasMaskResult(value["result"]);
}
function isCanvasMaskChanged(value) {
  return isRecord2(value) && Object.keys(value).length === 7 && Object.hasOwn(value, "protocol") && value["protocol"] === "sugarcubes.canvas.masks" && Object.hasOwn(value, "version") && value["version"] === 1 && Object.hasOwn(value, "session") && typeof value["session"] === "string" && value["session"].length <= 80 && new RegExp("^[a-zA-Z0-9-]{16,80}$").test(value["session"]) && Object.hasOwn(value, "type") && value["type"] === "changed" && Object.hasOwn(value, "documentId") && typeof value["documentId"] === "string" && value["documentId"].length <= 128 && Object.hasOwn(value, "revision") && typeof value["revision"] === "string" && value["revision"].length <= 512 && Object.hasOwn(value, "maskId") && typeof value["maskId"] === "string" && value["maskId"].length <= 128;
}
function isCanvasMaskMessage(value) {
  return isCanvasMaskRequest(value) || isCanvasMaskReply(value) || isCanvasMaskChanged(value);
}

// integrations/shared/CanvasMaskClient.ts
var CanvasMaskClient = class {
  /** Bind a single trusted navigation and its runtime clock. */
  constructor(session, port, identity = () => "") {
    this.session = session;
    this.port = port;
    this.identity = identity;
  }
  #pending = /* @__PURE__ */ new Map();
  #listeners = /* @__PURE__ */ new Set();
  #next = 0;
  #disposed = false;
  /** Read the negotiated canvas identity without guessing from transport or editor names. */
  get canvasId() {
    return this.identity();
  }
  /** Submit one capability operation and retire it on timeout or connection loss. */
  execute(command) {
    if (this.#disposed)
      return Promise.resolve({
        kind: "failure",
        status: "unavailable",
        message: "Canvas mask connection closed."
      });
    const requestId = ++this.#next;
    return new Promise((resolve) => {
      const stop = this.port.later(3e4, () => {
        this.#pending.delete(requestId);
        resolve({
          kind: "failure",
          status: "unavailable",
          message: "Canvas mask operation timed out; it was not retried."
        });
      });
      this.#pending.set(requestId, { resolve, stop });
      try {
        this.port.send({
          protocol: "sugarcubes.canvas.masks",
          version: 1,
          session: this.session,
          requestId,
          command
        });
      } catch (error) {
        this.#pending.delete(requestId);
        stop();
        resolve({
          kind: "failure",
          status: "unavailable",
          message: error instanceof Error ? error.message.slice(0, 512) : "Canvas mask transport failed."
        });
      }
    });
  }
  /** Receive committed changes independently of in-flight replies. */
  subscribe(listener) {
    this.#listeners.add(listener);
    return () => {
      this.#listeners.delete(listener);
    };
  }
  /** Validate the complete bounded envelope before exposing any native value. */
  receive(value) {
    if (this.#disposed || !isCanvasMaskMessage(value) || value.session !== this.session)
      return false;
    if ("type" in value) {
      for (const listener of this.#listeners) listener(value);
      return true;
    }
    if (!("result" in value)) return false;
    const pending = this.#pending.get(value.requestId);
    if (!pending) return false;
    this.#pending.delete(value.requestId);
    pending.stop();
    pending.resolve(value.result);
    return true;
  }
  /** Settle outstanding operations before releasing native-change consumers. */
  dispose() {
    this.#disposed = true;
    for (const pending of this.#pending.values()) {
      pending.stop();
      pending.resolve({
        kind: "failure",
        status: "unavailable",
        message: "Canvas mask connection closed."
      });
    }
    this.#pending.clear();
    this.#listeners.clear();
  }
};

// integrations/shared/CanvasMaskPixels.ts
var MAX_MASK_PIXELS = 64 * 1024 * 1024;
function encodeMaskPixels(width, height, bytes) {
  if (!Number.isSafeInteger(width) || !Number.isSafeInteger(height) || width < 1 || height < 1 || width > 65536 || height > 65536 || width * height !== bytes.length || bytes.length > MAX_MASK_PIXELS)
    throw new Error("Invalid or oversized mask coverage.");
  return { width, height, data: encodeCanvasBytes(bytes) };
}
function decodeMaskPixels(pixels) {
  const size = pixels.width * pixels.height;
  if (!Number.isSafeInteger(pixels.width) || !Number.isSafeInteger(pixels.height) || pixels.width < 1 || pixels.height < 1 || pixels.width > 65536 || pixels.height > 65536 || size > MAX_MASK_PIXELS)
    throw new Error("Invalid or oversized mask coverage.");
  const bytes = decodeCanvasBytes(pixels.data, MAX_MASK_PIXELS);
  if (bytes.length !== size) throw new Error("Invalid mask coverage dimensions.");
  return bytes;
}

// integrations/shared/CanvasRasterEncoding.ts
var MAX_CANVAS_RASTER_BYTES = 64 * 1024 * 1024;
function canvasRasterByteLength(width, height, format) {
  if (!Number.isSafeInteger(width) || !Number.isSafeInteger(height) || width < 1 || height < 1 || width > 65536 || height > 65536)
    throw new Error("Invalid canvas raster geometry or format.");
  const size = width * height * (format === "rgba8" ? 4 : 1);
  if (size > MAX_CANVAS_RASTER_BYTES)
    throw new Error("The canvas raster exceeds its transfer budget.");
  return size;
}
function encodeCanvasRaster(width, height, format, bytes) {
  if (bytes.length !== canvasRasterByteLength(width, height, format))
    throw new Error("The canvas raster byte count differs from its geometry.");
  return { width, height, format, data: encodeCanvasBytes(bytes) };
}
function decodeCanvasRaster(pixels) {
  const size = canvasRasterByteLength(pixels.width, pixels.height, pixels.format);
  const bytes = decodeCanvasBytes(pixels.data, size);
  if (bytes.length !== size)
    throw new Error("The canvas raster byte count differs from its geometry.");
  return bytes;
}

// integrations/shared/CanvasSourceIdentity.ts
function canvasSourceTargetKey(target) {
  return JSON.stringify([
    target.kind,
    target.kind === "layer" || target.kind === "composition-from-layer" ? target.layerId : null
  ]);
}
function canvasInputSourceKey(source) {
  return source.kind === "file" ? JSON.stringify(["file", source.file]) : JSON.stringify([
    "canvas",
    source.canvasId,
    source.documentId,
    canvasSourceTargetKey(source.target)
  ]);
}
function sameCanvasSourceDocument(first, second) {
  return first.id === second.id && first.geometryRevision === second.geometryRevision && first.width === second.width && first.height === second.height;
}
export {
  CanvasCancellation,
  CanvasClient,
  CanvasMaskClient,
  MAX_CANVAS_IMAGE_BYTES,
  MAX_CANVAS_RASTER_BYTES,
  WEB_CANVAS,
  canvasInputSourceKey,
  canvasRasterByteLength,
  canvasSourceTargetKey,
  decodeCanvasRaster,
  decodeMaskPixels,
  encodeCanvasImage,
  encodeCanvasRaster,
  encodeMaskPixels,
  isCanvasInputSource,
  isCanvasInputTarget,
  isCanvasMaskReference,
  sameCanvasSourceDocument
};
