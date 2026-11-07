#!/usr/bin/env python3
"""HTTPS login smoke against the actual Compose-built Caddy + crm-api + React."""
from __future__ import annotations
import argparse
from http.cookiejar import CookieJar
from pathlib import Path
import json
import ssl
import sys
import time
from urllib.error import HTTPError, URLError
from urllib.parse import urlencode
from urllib.request import HTTPCookieProcessor, HTTPSHandler, Request, build_opener


def credentials(path: Path) -> tuple[str, str, str, str]:
    entries = {}
    for line in path.read_text(encoding="utf-8").splitlines():
        if not line or line.startswith("#") or "=" not in line:
            continue
        key, value = line.split("=", 1)
        entries[key] = value.strip().strip("'")
    user = json.loads(entries["CRM_PILOT_OPERATORS"])[0]
    return user["username"], user["password"], user["tenant_id"], user["actor_id"]


def status(opener, url: str, expected: int, **kwargs):
    request = Request(url, **kwargs)
    try:
        response = opener.open(request, timeout=30)
    except HTTPError as error:
        response = error
    with response:
        if response.status != expected:
            raise AssertionError(
                f"unexpected HTTP status for {request.full_url}: "
                f"{response.status} (expected {expected})"
            )
        return response.read(), response.headers


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--env", type=Path, required=True)
    parser.add_argument("--origin", required=True)
    parser.add_argument("--local-insecure-tls", action="store_true")
    parser.add_argument("--ca-bundle", type=Path, help="Explicit trusted local CA bundle")
    args = parser.parse_args()
    origin = args.origin.rstrip("/")
    if not origin.startswith("https://"):
        raise ValueError("Only HTTPS origins can be smoke tested")
    if args.local_insecure_tls and origin != "https://localhost":
        raise ValueError("Untrusted CI-only TLS is permitted solely on https://localhost")
    context = ssl.create_default_context()
    if args.ca_bundle and args.local_insecure_tls:
        raise ValueError("CA verification and insecure TLS modes are mutually exclusive")
    if args.ca_bundle:
        context.load_verify_locations(cafile=str(args.ca_bundle))
    if args.local_insecure_tls:
        context.check_hostname = False
        context.verify_mode = ssl.CERT_NONE
    cookies = CookieJar()
    opener = build_opener(HTTPCookieProcessor(cookies), HTTPSHandler(context=context))

    # Caddy can be running slightly before its local certificate is ready.
    for attempt in range(30):
        try:
            home, headers = status(opener, origin + "/", 200)
            break
        except (URLError, TimeoutError):
            if attempt == 29:
                raise
            time.sleep(1)
    if b"<html" not in home.lower() or b"root" not in home:
        raise AssertionError("Caddy did not serve the built React shell")
    if headers.get("x-content-type-options") != "nosniff":
        raise AssertionError("TLS proxy security headers missing")
    status(opener, origin + "/auth/session", 401)
    username, password, tenant, actor = credentials(args.env)
    body = urlencode({"username": username, "password": password}).encode()
    status(opener, origin + "/auth/login", 403,
           data=body, headers={"Origin": "https://untrusted.example"})
    status(opener, origin + "/auth/login", 401,
           data=urlencode({"username": username, "password": "incorrect"}).encode(),
           headers={"Origin": origin})
    # The existing login endpoint redirects to / after issuing a secure cookie.
    status(opener, origin + "/auth/login", 200, data=body,
           headers={"Origin": origin})
    session_data, _ = status(opener, origin + "/auth/session", 200)
    session = json.loads(session_data)
    if (session.get("status"), session.get("tenantId"), session.get("actorId")) != (
        "authenticated", tenant, actor
    ):
        raise AssertionError("Authenticated session tenant/actor mismatch")
    csrf = session.get("csrfToken")
    if not isinstance(csrf, str) or not csrf:
        raise AssertionError("CSRF token missing from authenticated session")
    status(opener, origin + "/auth/logout", 401, data=b"",
           headers={"Origin": origin})
    status(opener, origin + "/auth/session", 200)
    status(opener, origin + "/auth/logout", 204, data=b"",
           headers={"Origin": origin, "x-crm-csrf": csrf})
    status(opener, origin + "/auth/session", 401)
    print("PASS: production-built React over HTTPS; origin and password rejection; "
          "cookie authentication; CSRF denial; logout and session revocation")


if __name__ == "__main__":
    try:
        main()
    except (AssertionError, ValueError, KeyError, OSError, HTTPError, URLError) as error:
        print(f"HTTPS pilot smoke failed: {error}", file=sys.stderr)
        sys.exit(1)
