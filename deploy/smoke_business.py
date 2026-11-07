#!/usr/bin/env python3
"""Installed CRM party CRUD and tenant-denial proof through the existing HTTPS API."""
from __future__ import annotations

import argparse
from http.cookiejar import CookieJar
import json
from pathlib import Path
import ssl
import sys
from urllib.error import HTTPError, URLError
from urllib.parse import urlencode
from urllib.request import HTTPCookieProcessor, HTTPSHandler, Request, build_opener

from smoke_https import credentials, status

PARTY = "g3-installed-party"
FIRST = "G3 installed pilot party"
UPDATED = "G3 installed party after update"


def varint(number: int) -> bytes:
    if number < 0:
        raise ValueError("negative protobuf varint")
    encoded = bytearray()
    while number >= 128:
        encoded.append((number & 127) | 128)
        number >>= 7
    encoded.append(number)
    return bytes(encoded)


def number(field: int, value: int) -> bytes:
    return varint(field << 3) + varint(value)


def data(field: int, raw: bytes) -> bytes:
    return varint((field << 3) | 2) + varint(len(raw)) + raw


def source_contract(definitions: list[dict], capability_id: str) -> dict:
    matched = [item for item in definitions if item["capability_id"] == capability_id]
    if len(matched) != 1:
        raise AssertionError(f"Expected one compiled production capability {capability_id}")
    return matched[0]


def payload(definition: dict, wire: bytes) -> dict:
    contract = definition["input_contract"]
    if "protobuf" not in contract["allowed_encodings"]:
        raise AssertionError("Production contract does not allow protobuf input")
    return {
        "owner": contract["owner"],
        "schema_id": contract["schema_id"],
        "schema_version": contract["schema_version"],
        "descriptor_hash": contract["descriptor_hash"],
        "data_class": contract["allowed_data_classes"][0],
        "encoding": "protobuf",
        "maximum_size_bytes": contract["maximum_size_bytes"],
        "retention_policy_id": "standard",
        "bytes": list(wire),
    }


def invoke(opener, origin: str, definition: dict, wire: bytes, tenant: str,
           csrf: str, *, mutation: bool, key: str | None = None,
           accepted: tuple[int, ...] = (200,)) -> bytes:
    namespace = "mutations" if mutation else "queries"
    endpoint = (
        f"{origin}/v1/{namespace}/{definition['owner_module_id']}/"
        f"{definition['capability_id']}/{definition['capability_version']}"
    )
    headers = {
        "x-tenant-id": tenant,
        "x-crm-csrf": csrf,
        "content-type": "application/json",
    }
    if key is not None:
        headers["idempotency-key"] = key
    request = Request(endpoint, method="POST",
                      data=json.dumps(payload(definition, wire)).encode(),
                      headers=headers)
    try:
        response = opener.open(request, timeout=30)
    except HTTPError as error:
        response = error
    with response:
        result = response.read()
        if response.status not in accepted:
            raise AssertionError(
                f"{definition['capability_id']} returned HTTP {response.status}, "
                f"expected {accepted}; response={result[:180]!r}"
            )
        return result


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--env", type=Path, required=True)
    parser.add_argument("--catalog", type=Path, required=True)
    parser.add_argument("--phase", choices=("initial", "verify"), required=True)
    parser.add_argument("--origin", default="https://localhost")
    args = parser.parse_args()
    if args.origin != "https://localhost":
        raise ValueError("CI-only local TLS mode does not accept remote hosts")
    context = ssl.create_default_context()
    context.check_hostname = False
    context.verify_mode = ssl.CERT_NONE
    jar = CookieJar()
    opener = build_opener(HTTPCookieProcessor(jar), HTTPSHandler(context=context))
    username, password, tenant, actor = credentials(args.env)
    login = urlencode({"username": username, "password": password}).encode()
    status(opener, args.origin + "/auth/login", 200,
           data=login, headers={"Origin": args.origin})
    session = json.loads(status(opener, args.origin + "/auth/session", 200)[0])
    if session.get("actorId") != actor or session.get("tenantId") != tenant:
        raise AssertionError("Operator login bound to incorrect actor or tenant")
    csrf = session["csrfToken"]
    definitions = json.loads(args.catalog.read_text())
    create = source_contract(definitions, "parties.party.create")
    get = source_contract(definitions, "parties.party.get")
    update = source_contract(definitions, "parties.party.update")
    listing = source_contract(definitions, "parties.party.list")
    party_ref = data(1, PARTY.encode())
    get_request = data(1, party_ref)

    if args.phase == "initial":
        create_request = data(1, party_ref) + number(2, 2) + data(3, FIRST.encode())
        invoke(opener, args.origin, create, create_request, tenant, csrf,
               mutation=True, key="g3-first-party-create")
        # Authenticated operator is never permitted to claim a foreign tenant.
        invoke(opener, args.origin, create, create_request, "g3-foreign-tenant",
               csrf, mutation=True, key="g3-denied-foreign-create",
               accepted=(401, 403, 404))
        update_request = data(1, party_ref) + number(2, 1) + data(3, UPDATED.encode())
        invoke(opener, args.origin, update, update_request, tenant, csrf,
               mutation=True, key="g3-first-party-update")

    encoded = invoke(opener, args.origin, get, get_request, tenant, csrf,
                     mutation=False)
    result = json.loads(encoded)
    if UPDATED.encode() not in bytes(result["bytes"]):
        raise AssertionError("Updated persisted Party name not found in query payload")
    # Use the real ListPartiesRequest with PageRequest.page_size=25.
    page_request = data(1, number(1, 25))
    listed = json.loads(invoke(opener, args.origin, listing, page_request, tenant,
                               csrf, mutation=False))
    if PARTY.encode() not in bytes(listed["bytes"]):
        raise AssertionError("Created Party missing from tenant list response")
    print(f"PASS: real HTTPS Party {args.phase}: authenticated create/update/get/list, "
          "foreign-tenant denial and PostgreSQL persistence")


if __name__ == "__main__":
    try:
        main()
    except (AssertionError, ValueError, KeyError, OSError, HTTPError, URLError) as exc:
        print(f"Pilot business acceptance failed: {exc}", file=sys.stderr)
        sys.exit(1)
