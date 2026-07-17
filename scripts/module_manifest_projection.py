#!/usr/bin/env python3
"""Project authoring module manifests into the runtime-only manifest shape."""

from __future__ import annotations

from copy import deepcopy
from typing import Any


def runtime_manifest_projection(manifest: dict[str, Any]) -> dict[str, Any]:
    """Remove build-time contract bindings from normalized runtime manifest IR.

    Protobuf coordinates are a repository/CI concern. Runtime module identity keeps
    only versioned capability and event IDs, so moving an RPC without changing the
    module's runtime contract identity does not churn installation state.
    """

    projected = deepcopy(manifest)
    provides = projected.get("provides")
    if not isinstance(provides, dict):
        return projected
    for category in ("capabilities", "events"):
        items = provides.get(category)
        if not isinstance(items, list):
            continue
        for item in items:
            if isinstance(item, dict):
                item.pop("binding", None)
    return projected
