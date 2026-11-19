// integrations/shared/FileDragValidation.generated.ts
function isRecord(value) {
  return !!value && typeof value === "object" && !Array.isArray(value);
}
function isFileDragFile(value) {
  return isRecord(value) && Object.keys(value).length === 1 && Object.hasOwn(value, "name") && typeof value["name"] === "string" && value["name"].length <= 255;
}
function isFileDragEnteredMessage(value) {
  return isRecord(value) && Object.keys(value).length === 5 && Object.hasOwn(value, "protocol") && value["protocol"] === "sugarcubes.file-drag" && Object.hasOwn(value, "version") && value["version"] === 1 && Object.hasOwn(value, "session") && typeof value["session"] === "string" && value["session"].length <= 80 && new RegExp("^[a-zA-Z0-9-]{16,80}$").test(value["session"]) && Object.hasOwn(value, "type") && value["type"] === "entered" && Object.hasOwn(value, "files") && Array.isArray(value["files"]) && value["files"].length >= 1 && value["files"].length <= 16 && value["files"].every((item) => isFileDragFile(item));
}
function isFileDragLeftMessage(value) {
  return isRecord(value) && Object.keys(value).length === 4 && Object.hasOwn(value, "protocol") && value["protocol"] === "sugarcubes.file-drag" && Object.hasOwn(value, "version") && value["version"] === 1 && Object.hasOwn(value, "session") && typeof value["session"] === "string" && value["session"].length <= 80 && new RegExp("^[a-zA-Z0-9-]{16,80}$").test(value["session"]) && Object.hasOwn(value, "type") && value["type"] === "left";
}
function isFileDragHostMessage(value) {
  return isFileDragEnteredMessage(value) || isFileDragLeftMessage(value);
}
export {
  isFileDragHostMessage
};
