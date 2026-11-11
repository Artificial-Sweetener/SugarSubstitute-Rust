// integrations/shared/MemoryValidation.generated.ts
function isRecord(value) {
  return !!value && typeof value === "object" && !Array.isArray(value);
}
function isMemoryLevel(value) {
  return value === "normal" || value === "elevated" || value === "high" || value === "critical";
}
function isMemoryPreference(value) {
  return isRecord(value) && Object.keys(value).length === 2 && Object.hasOwn(value, "mode") && value["mode"] === "share" && Object.hasOwn(value, "percent") && typeof value["percent"] === "number" && Number.isSafeInteger(value["percent"]) && value["percent"] >= 10 && value["percent"] <= 75 || isRecord(value) && Object.keys(value).length === 2 && Object.hasOwn(value, "mode") && value["mode"] === "fixed" && Object.hasOwn(value, "bytes") && typeof value["bytes"] === "number" && Number.isSafeInteger(value["bytes"]) && value["bytes"] >= 1 && value["bytes"] <= 9007199254740991;
}
function isMemoryAllowanceMessage(value) {
  return isRecord(value) && Object.keys(value).length === 10 && Object.hasOwn(value, "protocol") && value["protocol"] === "sugarcubes.memory" && Object.hasOwn(value, "version") && value["version"] === 1 && Object.hasOwn(value, "session") && typeof value["session"] === "string" && value["session"].length <= 80 && new RegExp("^[a-zA-Z0-9-]{16,80}$").test(value["session"]) && Object.hasOwn(value, "type") && value["type"] === "allowance" && Object.hasOwn(value, "generation") && typeof value["generation"] === "number" && Number.isSafeInteger(value["generation"]) && value["generation"] >= 0 && value["generation"] <= 9007199254740991 && Object.hasOwn(value, "level") && isMemoryLevel(value["level"]) && Object.hasOwn(value, "allowanceBytes") && typeof value["allowanceBytes"] === "number" && Number.isSafeInteger(value["allowanceBytes"]) && value["allowanceBytes"] >= 0 && value["allowanceBytes"] <= 9007199254740991 && Object.hasOwn(value, "ceilingBytes") && typeof value["ceilingBytes"] === "number" && Number.isSafeInteger(value["ceilingBytes"]) && value["ceilingBytes"] >= 0 && value["ceilingBytes"] <= 9007199254740991 && Object.hasOwn(value, "totalBytes") && typeof value["totalBytes"] === "number" && Number.isSafeInteger(value["totalBytes"]) && value["totalBytes"] >= 0 && value["totalBytes"] <= 9007199254740991 && Object.hasOwn(value, "preference") && isMemoryPreference(value["preference"]);
}
export {
  isMemoryAllowanceMessage
};
