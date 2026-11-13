#    SugarCubes - composable workflow units for ComfyUI
#    Copyright (C) 2026  Artificial Sweetener and contributors
#
#    This program is free software: you can redistribute it and/or modify
#    it under the terms of the GNU Affero General Public License as published by
#    the Free Software Foundation, either version 3 of the License, or
#    (at your option) any later version.
#
#    This program is distributed in the hope that it will be useful,
#    but WITHOUT ANY WARRANTY; without even the implied warranty of
#    MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
#    GNU Affero General Public License for more details.
#
#    You should have received a copy of the GNU Affero General Public License
#    along with this program.  If not, see <https://www.gnu.org/licenses/>.
"""Persist and validate local repo-ownership policy settings."""

from __future__ import annotations

from dataclasses import asdict, dataclass
import json
import os
from pathlib import Path
from typing import Any, Optional

from ...cube_model.cube_identity import CubeIdentityError, validate_github_repo_ref
from ..responses import BackendError

_MANAGED_DIRNAME = ".sugarcubes"
_POLICY_FILENAME = "identity_policy.json"
_DOTENV_FILENAME = ".env"
_SYSTEM_OWNER = "artificial-sweetener"
_PROCESS_ENV_SOURCE = "process_env"
_DOTENV_SOURCE = "dotenv"
_FILE_SOURCE = "file"
_DEFAULT_SOURCE = "default"
_ENV_MANAGED_SOURCES = frozenset({_PROCESS_ENV_SOURCE, _DOTENV_SOURCE})
_ENV_OWNER_KEY = "SUGARCUBES_CLAIMED_GITHUB_OWNER"
_ENV_SYSTEM_CLAIM_KEY = "SUGARCUBES_ALLOW_SYSTEM_OWNER_CLAIM"
_TRUTHY_ENV_VALUES = frozenset({"1", "true", "yes", "on"})
_FALSY_ENV_VALUES = frozenset({"", "0", "false", "no", "off"})


@dataclass(frozen=True)
class IdentityPolicy:
    """Represent the persisted local identity policy settings."""

    claimed_github_owner: str = ""


@dataclass(frozen=True)
class EffectiveIdentityPolicy:
    """Represent the effective merged identity policy used at runtime."""

    claimed_github_owner: str = ""
    allow_system_owner_claim: bool = False
    claimed_github_owner_source: str = _DEFAULT_SOURCE
    allow_system_owner_claim_source: str = _DEFAULT_SOURCE
    env_override_active: bool = False


@dataclass(frozen=True)
class _PolicyOverride:
    """Capture one policy override layer plus the fields it actually provides."""

    source: str
    has_claimed_github_owner: bool = False
    claimed_github_owner: str = ""
    has_allow_system_owner_claim: bool = False
    allow_system_owner_claim: bool = False


class IdentityPolicyService:
    """Own backend-persisted identity policy for ownership checks."""

    def __init__(self, extension_root: Path) -> None:
        """Initialize one identity policy service bound to the extension data root."""

        self.extension_root = self._resolve_extension_root(extension_root)

    def data_root(self) -> Path:
        """Return the extension-owned backend data directory."""

        return self.extension_root / _MANAGED_DIRNAME

    def policy_path(self) -> Path:
        """Return the persisted identity policy path."""

        return self.data_root() / _POLICY_FILENAME

    def dotenv_path(self) -> Path:
        """Return the repo-root `.env` path used for local machine overrides."""

        return self.extension_root / _DOTENV_FILENAME

    def get_policy(self) -> EffectiveIdentityPolicy:
        """Return the effective merged identity policy."""

        file_policy = self._read_persisted_policy()
        merged_claimed_owner = file_policy.claimed_github_owner
        merged_allow_system_owner_claim = False
        claimed_github_owner_source = (
            _FILE_SOURCE if merged_claimed_owner else _DEFAULT_SOURCE
        )
        allow_system_owner_claim_source = _DEFAULT_SOURCE

        for override in (self._read_dotenv_policy(), self._read_process_env_policy()):
            if override.has_claimed_github_owner:
                merged_claimed_owner = override.claimed_github_owner
                claimed_github_owner_source = override.source
            if override.has_allow_system_owner_claim:
                merged_allow_system_owner_claim = override.allow_system_owner_claim
                allow_system_owner_claim_source = override.source

        normalized = self._normalize_policy_payload(
            claimed_github_owner=merged_claimed_owner,
            allow_system_owner_claim=merged_allow_system_owner_claim,
        )
        env_override_active = (
            claimed_github_owner_source in _ENV_MANAGED_SOURCES
            or allow_system_owner_claim_source in _ENV_MANAGED_SOURCES
        )
        return EffectiveIdentityPolicy(
            claimed_github_owner=normalized.claimed_github_owner,
            allow_system_owner_claim=merged_allow_system_owner_claim,
            claimed_github_owner_source=claimed_github_owner_source,
            allow_system_owner_claim_source=allow_system_owner_claim_source,
            env_override_active=env_override_active,
        )

    def set_policy(
        self,
        *,
        claimed_github_owner: Optional[str] = None,
    ) -> dict[str, Any]:
        """Persist one partial identity policy update and return the effective payload."""

        self._reject_env_managed_updates(
            claimed_github_owner=claimed_github_owner,
        )
        current = self._read_persisted_policy()
        normalized = self._normalize_policy_payload(
            claimed_github_owner=(
                current.claimed_github_owner
                if claimed_github_owner is None
                else claimed_github_owner
            ),
            allow_system_owner_claim=self._effective_allow_system_owner_claim(),
        )
        self._write_policy(normalized)
        return self.serialize_policy()

    def serialize_policy(
        self, policy: Optional[EffectiveIdentityPolicy] = None
    ) -> dict[str, Any]:
        """Serialize one effective identity policy for route responses."""

        resolved = policy or self.get_policy()
        return {
            "claimed_github_owner": resolved.claimed_github_owner,
            "allow_system_owner_claim": resolved.allow_system_owner_claim,
            "has_claimed_github_owner": bool(resolved.claimed_github_owner),
            "claimed_github_owner_source": resolved.claimed_github_owner_source,
            "allow_system_owner_claim_source": resolved.allow_system_owner_claim_source,
            "env_override_active": resolved.env_override_active,
        }

    def _resolve_extension_root(self, extension_root: Path) -> Path:
        """Resolve the repository root regardless of being passed a file or directory."""

        resolved = extension_root.resolve()
        if resolved.is_file():
            return resolved.parent
        return resolved

    def _read_persisted_policy(self) -> IdentityPolicy:
        """Read and normalize the file-backed identity policy layer."""

        path = self.policy_path()
        path.parent.mkdir(parents=True, exist_ok=True)
        if not path.exists():
            policy = IdentityPolicy()
            self._write_policy(policy)
            return policy
        try:
            payload = json.loads(path.read_text(encoding="utf-8"))
        except (OSError, ValueError) as exc:
            raise BackendError("Identity policy is invalid", status=500) from exc
        if not isinstance(payload, dict):
            raise BackendError("Identity policy is invalid", status=500)
        normalized = self._normalize_policy_payload(
            claimed_github_owner=payload.get("claimed_github_owner"),
            allow_system_owner_claim=self._effective_allow_system_owner_claim(),
        )
        if payload != asdict(normalized):
            self._write_policy(normalized)
        return normalized

    def _read_process_env_policy(self) -> _PolicyOverride:
        """Read identity-policy overrides from the process environment."""

        return self._read_env_mapping(os.environ, source=_PROCESS_ENV_SOURCE)

    def _read_dotenv_policy(self) -> _PolicyOverride:
        """Read identity-policy overrides from the repo-root `.env` file."""

        dotenv_path = self.dotenv_path()
        if not dotenv_path.exists():
            return _PolicyOverride(source=_DOTENV_SOURCE)
        try:
            lines = dotenv_path.read_text(encoding="utf-8").splitlines()
        except OSError as exc:
            raise BackendError("Identity policy .env is invalid", status=500) from exc
        dotenv_values: dict[str, str] = {}
        for raw_line in lines:
            entry = raw_line.strip()
            if not entry or entry.startswith("#") or "=" not in entry:
                continue
            key, raw_value = entry.split("=", 1)
            normalized_key = key.strip()
            if not normalized_key:
                continue
            dotenv_values[normalized_key] = self._strip_env_quotes(raw_value.strip())
        return self._read_env_mapping(dotenv_values, source=_DOTENV_SOURCE)

    def _read_env_mapping(
        self,
        values: dict[str, str] | os._Environ[str],
        *,
        source: str,
    ) -> _PolicyOverride:
        """Read supported policy keys from one environment-like mapping."""

        claimed_present = _ENV_OWNER_KEY in values
        allow_present = _ENV_SYSTEM_CLAIM_KEY in values
        claimed_value = values.get(_ENV_OWNER_KEY, "") if claimed_present else ""
        allow_value = (
            self._parse_bool_env(values.get(_ENV_SYSTEM_CLAIM_KEY, ""), source=source)
            if allow_present
            else False
        )
        return _PolicyOverride(
            source=source,
            has_claimed_github_owner=claimed_present,
            claimed_github_owner=claimed_value,
            has_allow_system_owner_claim=allow_present,
            allow_system_owner_claim=allow_value,
        )

    def _parse_bool_env(self, value: Any, *, source: str) -> bool:
        """Parse one boolean policy value from env or `.env`."""

        normalized = (
            value.strip().lower()
            if isinstance(value, str)
            else str(value).strip().lower()
        )
        if normalized in _TRUTHY_ENV_VALUES:
            return True
        if normalized in _FALSY_ENV_VALUES:
            return False
        raise BackendError(
            "Identity policy environment configuration is invalid: "
            f"{_ENV_SYSTEM_CLAIM_KEY} from {source} must be one of "
            "'1', 'true', 'yes', 'on', '0', 'false', 'no', 'off', or empty",
            status=500,
        )

    def _strip_env_quotes(self, value: str) -> str:
        """Remove one layer of matching surrounding quotes from an env value."""

        if len(value) >= 2 and value[0] == value[-1] and value[0] in {"'", '"'}:
            return value[1:-1]
        return value

    def _reject_env_managed_updates(
        self,
        *,
        claimed_github_owner: Optional[str],
    ) -> None:
        """Reject writes to identity fields that are controlled by env or `.env`."""

        current = self.get_policy()
        if (
            claimed_github_owner is not None
            and current.claimed_github_owner_source in _ENV_MANAGED_SOURCES
        ):
            raise BackendError(
                "Cannot update claimed GitHub owner: value is managed by environment configuration",
                status=409,
            )

    def _write_policy(self, policy: IdentityPolicy) -> None:
        """Persist one normalized policy payload to disk."""

        path = self.policy_path()
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(
            json.dumps(asdict(policy), indent=2) + "\n",
            encoding="utf-8",
        )

    def _normalize_policy_payload(
        self,
        *,
        claimed_github_owner: Any,
        allow_system_owner_claim: Any,
    ) -> IdentityPolicy:
        """Return one validated identity policy from untrusted payload values."""

        allow_system = bool(allow_system_owner_claim)
        normalized_owner = self._normalize_claimed_owner(claimed_github_owner)
        if normalized_owner.lower() == _SYSTEM_OWNER and not allow_system:
            normalized_owner = ""
        return IdentityPolicy(
            claimed_github_owner=normalized_owner,
        )

    def _effective_allow_system_owner_claim(self) -> bool:
        """Return the effective system-owner gate from env sources only."""

        for override in (self._read_dotenv_policy(), self._read_process_env_policy()):
            if override.has_allow_system_owner_claim:
                return override.allow_system_owner_claim
        return False

    def _normalize_claimed_owner(self, value: Any) -> str:
        """Normalize one claimed GitHub owner string or reject it."""

        cleaned = value.strip() if isinstance(value, str) else ""
        if not cleaned:
            return ""
        if cleaned.lower() == "local":
            raise BackendError(
                "GitHub owner 'local' is reserved by the managed local workspace",
                status=400,
            )
        try:
            owner, _repo = validate_github_repo_ref(cleaned, "placeholder")
        except CubeIdentityError as exc:
            raise BackendError(str(exc), status=400) from exc
        return owner
