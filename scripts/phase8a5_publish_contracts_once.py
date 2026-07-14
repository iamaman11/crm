from __future__ import annotations

import json
from pathlib import Path

BINDINGS_PATH = Path("contracts/module-contract-bindings.json")
MODULE_ID = "crm.identity-resolution"


def capability(
    capability_id: str,
    method: str,
    request: str,
    response: str,
) -> dict[str, str]:
    return {
        "id": capability_id,
        "version": "1.0.0",
        "rpc": f"crm.identity_resolution.v1.IdentityResolutionService.{method}",
        "request": f"crm.identity_resolution.v1.{request}",
        "response": f"crm.identity_resolution.v1.{response}",
    }


def event(event_id: str, message: str) -> dict[str, str]:
    return {
        "id": event_id,
        "version": "1.0.0",
        "message": f"crm.identity_resolution.v1.{message}",
    }


def main() -> None:
    document = json.loads(BINDINGS_PATH.read_text())
    modules = document["modules"]
    if any(module.get("module_id") == MODULE_ID for module in modules):
        return

    published = {
        "module_id": MODULE_ID,
        "capabilities": [
            capability(
                "identity_resolution.candidate.register",
                "RegisterDuplicateCandidate",
                "RegisterDuplicateCandidateRequest",
                "RegisterDuplicateCandidateResponse",
            ),
            capability(
                "identity_resolution.candidate.evidence.refresh",
                "RefreshDuplicateCandidateEvidence",
                "RefreshDuplicateCandidateEvidenceRequest",
                "RefreshDuplicateCandidateEvidenceResponse",
            ),
            capability(
                "identity_resolution.candidate.dismiss",
                "DismissDuplicateCandidate",
                "DismissDuplicateCandidateRequest",
                "DismissDuplicateCandidateResponse",
            ),
            capability(
                "identity_resolution.candidate.confirm_duplicate",
                "ConfirmDuplicateCandidate",
                "ConfirmDuplicateCandidateRequest",
                "ConfirmDuplicateCandidateResponse",
            ),
            capability(
                "identity_resolution.candidate.get",
                "GetDuplicateCandidateCase",
                "GetDuplicateCandidateCaseRequest",
                "GetDuplicateCandidateCaseResponse",
            ),
            capability(
                "identity_resolution.candidate.list_by_party",
                "ListDuplicateCandidateCasesByParty",
                "ListDuplicateCandidateCasesByPartyRequest",
                "ListDuplicateCandidateCasesByPartyResponse",
            ),
        ],
        "events": [
            event(
                "identity_resolution.candidate.registered",
                "DuplicateCandidateRegisteredEvent",
            ),
            event(
                "identity_resolution.candidate.evidence_refreshed",
                "DuplicateCandidateEvidenceRefreshedEvent",
            ),
            event(
                "identity_resolution.candidate.dismissed",
                "DuplicateCandidateDismissedEvent",
            ),
            event(
                "identity_resolution.candidate.confirmed_duplicate",
                "DuplicateCandidateConfirmedEvent",
            ),
        ],
    }

    customer360_index = next(
        index
        for index, module in enumerate(modules)
        if module.get("module_id") == "crm.customer360"
    )
    modules.insert(customer360_index, published)
    BINDINGS_PATH.write_text(json.dumps(document, indent=2) + "\n")


if __name__ == "__main__":
    main()
