// integrations/shared/FileChoiceValidation.generated.ts
function isRecord(value) {
  return !!value && typeof value === "object" && !Array.isArray(value);
}
function isFileChoiceFailure(value) {
  return value === "busy" || value === "unavailable";
}
function isFileChoiceChosenMessage(value) {
  return isRecord(value) && Object.keys(value).length === 7 && Object.hasOwn(value, "protocol") && value["protocol"] === "sugarcubes.file-choice" && Object.hasOwn(value, "version") && value["version"] === 1 && Object.hasOwn(value, "session") && typeof value["session"] === "string" && value["session"].length <= 80 && new RegExp("^[a-zA-Z0-9-]{16,80}$").test(value["session"]) && Object.hasOwn(value, "type") && value["type"] === "chosen" && Object.hasOwn(value, "request") && typeof value["request"] === "string" && value["request"].length <= 48 && new RegExp("^[a-z0-9.-]{1,48}$").test(value["request"]) && Object.hasOwn(value, "name") && typeof value["name"] === "string" && value["name"].length <= 255 && Object.hasOwn(value, "relative") && typeof value["relative"] === "string" && value["relative"].length <= 1024;
}
function isFileChoiceDismissedMessage(value) {
  return isRecord(value) && Object.keys(value).length === 5 && Object.hasOwn(value, "protocol") && value["protocol"] === "sugarcubes.file-choice" && Object.hasOwn(value, "version") && value["version"] === 1 && Object.hasOwn(value, "session") && typeof value["session"] === "string" && value["session"].length <= 80 && new RegExp("^[a-zA-Z0-9-]{16,80}$").test(value["session"]) && Object.hasOwn(value, "type") && value["type"] === "dismissed" && Object.hasOwn(value, "request") && typeof value["request"] === "string" && value["request"].length <= 48 && new RegExp("^[a-z0-9.-]{1,48}$").test(value["request"]);
}
function isFileChoiceFailedMessage(value) {
  return isRecord(value) && Object.keys(value).length === 6 && Object.hasOwn(value, "protocol") && value["protocol"] === "sugarcubes.file-choice" && Object.hasOwn(value, "version") && value["version"] === 1 && Object.hasOwn(value, "session") && typeof value["session"] === "string" && value["session"].length <= 80 && new RegExp("^[a-zA-Z0-9-]{16,80}$").test(value["session"]) && Object.hasOwn(value, "type") && value["type"] === "failed" && Object.hasOwn(value, "request") && typeof value["request"] === "string" && value["request"].length <= 48 && new RegExp("^[a-z0-9.-]{1,48}$").test(value["request"]) && Object.hasOwn(value, "reason") && isFileChoiceFailure(value["reason"]);
}
function isFileChoiceHostMessage(value) {
  return isFileChoiceChosenMessage(value) || isFileChoiceDismissedMessage(value) || isFileChoiceFailedMessage(value);
}
export {
  isFileChoiceHostMessage
};
