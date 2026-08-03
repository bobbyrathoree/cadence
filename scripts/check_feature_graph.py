#!/usr/bin/env python3
"""Validate that a release unit graph excludes test-only Cargo features."""

from __future__ import annotations

import argparse
import json
import subprocess
import sys
from pathlib import Path
from typing import Any, TextIO

PINNED_TOOLCHAIN = "nightly-2026-07-15"
ROOT_FORBIDDEN = frozenset({"test-support", "test-faults", "lifecycle-test"})
CORE_FORBIDDEN = frozenset({"test-support"})


class AuditError(ValueError):
    pass


def _object(value: Any, label: str) -> dict[str, Any]:
    if not isinstance(value, dict):
        raise AuditError(f"{label} must be an object")
    return value


def _string_list(value: Any, label: str) -> list[str]:
    if not isinstance(value, list) or any(not isinstance(item, str) for item in value):
        raise AuditError(f"{label} must be an array of strings")
    return value


def _metadata_packages(metadata: Any) -> tuple[dict[str, str], dict[str, list[str]]]:
    document = _object(metadata, "metadata")
    packages = document.get("packages")
    if not isinstance(packages, list):
        raise AuditError("metadata.packages must be an array")

    names_by_id: dict[str, str] = {}
    ids_by_name: dict[str, list[str]] = {}
    for index, package_value in enumerate(packages):
        package = _object(package_value, f"metadata.packages[{index}]")
        package_id = package.get("id")
        name = package.get("name")
        if not isinstance(package_id, str) or not isinstance(name, str):
            raise AuditError(f"metadata.packages[{index}] needs string id and name")
        if package_id in names_by_id:
            raise AuditError(f"duplicate metadata package id: {package_id}")
        names_by_id[package_id] = name
        ids_by_name.setdefault(name, []).append(package_id)
    return names_by_id, ids_by_name


def check_feature_graph(graph: Any, metadata: Any, root_package: str) -> None:
    document = _object(graph, "unit graph")
    if set(document) != {"version", "units", "roots"}:
        raise AuditError("unit graph must contain only version, units, and roots")
    if document["version"] != 1:
        raise AuditError(f"unsupported unit graph version: {document['version']!r}")

    units_value = document["units"]
    roots_value = document["roots"]
    if not isinstance(units_value, list):
        raise AuditError("unit graph units must be an array")
    if not isinstance(roots_value, list):
        raise AuditError("unit graph roots must be an array")
    if not roots_value:
        raise AuditError("unit graph has zero roots")

    units: list[tuple[str, frozenset[str]]] = []
    for index, unit_value in enumerate(units_value):
        unit = _object(unit_value, f"units[{index}]")
        package_id = unit.get("pkg_id")
        if not isinstance(package_id, str):
            raise AuditError(f"units[{index}].pkg_id must be a string")
        features = frozenset(_string_list(unit.get("features"), f"units[{index}].features"))
        units.append((package_id, features))

    root_indices: list[int] = []
    for index, root_value in enumerate(roots_value):
        if type(root_value) is not int:
            raise AuditError(f"roots[{index}] must be an integer")
        if root_value < 0 or root_value >= len(units):
            raise AuditError(f"roots[{index}] references missing unit {root_value}")
        root_indices.append(root_value)

    names_by_id, ids_by_name = _metadata_packages(metadata)
    for package_id, _features in units:
        if package_id not in names_by_id:
            raise AuditError(f"unit pkg_id is absent from metadata: {package_id}")

    root_ids = {units[index][0] for index in root_indices}
    if len(root_ids) != 1:
        raise AuditError(f"unit graph has multiple root package ids: {sorted(root_ids)}")
    root_id = next(iter(root_ids))

    expected_root_ids = ids_by_name.get(root_package, [])
    if len(expected_root_ids) != 1:
        raise AuditError(
            f"metadata must resolve exactly one {root_package} package; "
            f"found {len(expected_root_ids)}"
        )
    expected_root_id = expected_root_ids[0]
    root_units = [features for package_id, features in units if package_id == expected_root_id]
    if root_id != expected_root_id or not root_units:
        raise AuditError(f"unit graph has zero matching root units for {root_package}")

    for features in root_units:
        forbidden = sorted(features & ROOT_FORBIDDEN)
        if forbidden:
            raise AuditError(
                f"{root_package} release unit enables forbidden features: "
                f"{', '.join(forbidden)}"
            )

    core_ids = ids_by_name.get("cadence-core", [])
    if len(core_ids) != 1:
        raise AuditError(
            f"metadata must resolve exactly one cadence-core package; found {len(core_ids)}"
        )
    core_units = [features for package_id, features in units if package_id == core_ids[0]]
    if not core_units:
        raise AuditError("unit graph has zero cadence-core units")
    for features in core_units:
        forbidden = sorted(features & CORE_FORBIDDEN)
        if forbidden:
            raise AuditError(
                "cadence-core release unit enables forbidden features: "
                + ", ".join(forbidden)
            )


def _read_json(stream: TextIO, label: str) -> Any:
    try:
        return json.load(stream)
    except (json.JSONDecodeError, UnicodeDecodeError) as error:
        raise AuditError(f"invalid {label} JSON: {error}") from error


def _load_metadata(path: Path | None) -> Any:
    if path is not None:
        with path.open(encoding="utf-8") as stream:
            return _read_json(stream, "metadata")

    command = [
        "rustup",
        "run",
        PINNED_TOOLCHAIN,
        "cargo",
        "metadata",
        "--format-version",
        "1",
    ]
    completed = subprocess.run(command, check=False, capture_output=True, text=True)
    if completed.returncode != 0:
        detail = completed.stderr.strip() or completed.stdout.strip()
        raise AuditError(f"pinned cargo metadata failed: {detail}")
    try:
        return json.loads(completed.stdout)
    except json.JSONDecodeError as error:
        raise AuditError(f"pinned cargo metadata returned invalid JSON: {error}") from error


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--root-package", required=True)
    parser.add_argument("--metadata", type=Path)
    parser.add_argument(
        "graph",
        nargs="?",
        type=Path,
        help="unit graph JSON path; stdin when omitted",
    )
    arguments = parser.parse_args()

    try:
        if arguments.graph is None:
            graph = _read_json(sys.stdin, "unit graph")
        else:
            with arguments.graph.open(encoding="utf-8") as stream:
                graph = _read_json(stream, "unit graph")
        metadata = _load_metadata(arguments.metadata)
        check_feature_graph(graph, metadata, arguments.root_package)
    except (AuditError, OSError) as error:
        print(f"feature graph audit failed: {error}", file=sys.stderr)
        return 1

    print(
        f"Feature graph clean for {arguments.root_package}: "
        "test-only root/core features absent"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
