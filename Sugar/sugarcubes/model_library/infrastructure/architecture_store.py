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
"""Persist architecture observations independently of provider and user presentation."""

from __future__ import annotations

from ..domain.architecture import ModelArchitecture
from ..domain.models import ModelEvidence
from .catalog_database import ModelCatalogDatabase


class SqliteModelArchitectures:
    """Publish known and unknown classifications against current catalog evidence only."""

    def __init__(self, database: ModelCatalogDatabase) -> None:
        """Share the catalog transaction and revision owner."""
        self._database = database

    def current(self, evidence: ModelEvidence, signature: str) -> bool:
        """Reuse unchanged file observations across restarts and metadata refreshes."""
        with self._database.connect() as connection:
            return (
                connection.execute(
                    "SELECT 1 FROM model_architectures WHERE id=? AND size_bytes=? AND modified_ns=? AND signature=?",
                    (
                        evidence.location_id,
                        evidence.size_bytes,
                        evidence.modified_ns,
                        signature,
                    ),
                ).fetchone()
                is not None
            )

    def save(
        self,
        evidence: ModelEvidence,
        signature: str,
        architecture: ModelArchitecture | None,
    ) -> bool:
        """Reject obsolete workers and publish independent evidence with one revision update."""
        with self._database.connect() as connection:
            connection.execute("BEGIN IMMEDIATE")
            row = connection.execute(
                "SELECT size_bytes,modified_ns FROM model_files WHERE id=?",
                (evidence.location_id,),
            ).fetchone()
            if row is None or (row["size_bytes"], row["modified_ns"]) != (
                evidence.size_bytes,
                evidence.modified_ns,
            ):
                return False
            values = (
                evidence.size_bytes,
                evidence.modified_ns,
                signature,
                architecture.base if architecture else None,
                architecture.comfy_class if architecture else None,
            )
            previous = connection.execute(
                "SELECT size_bytes,modified_ns,signature,base,comfy_class FROM model_architectures WHERE id=?",
                (evidence.location_id,),
            ).fetchone()
            if previous is not None and tuple(previous) == values:
                return False
            connection.execute(
                "INSERT OR REPLACE INTO model_architectures VALUES (?,?,?,?,?,?)",
                (evidence.location_id, *values),
            )
            self._database.advance_revision(connection)
            return True
