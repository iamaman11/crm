from pathlib import Path

path = Path("crates/crm-customer-enrichment-worker-composition/src/lib.rs")
text = path.read_text()


def replace_once(old: str, new: str, label: str) -> None:
    global text
    count = text.count(old)
    if count != 1:
        raise SystemExit(f"{label}: expected exactly one match, found {count}")
    text = text.replace(old, new, 1)


replace_once(
    """pub enum ProviderDispatchExecution {
    Recorded(ProviderDispatchWorkerResult),
    Conflicting(ProviderResponseConflictDraft),
}
""",
    """pub enum ProviderDispatchExecution {
    Recorded(Box<ProviderDispatchWorkerResult>),
    Conflicting(ProviderResponseConflictDraft),
}
""",
    "box recorded result",
)
replace_once(
    "ProviderDispatchExecution::Recorded(result) => Ok(result),",
    "ProviderDispatchExecution::Recorded(result) => Ok(*result),",
    "unbox ordinary execution",
)
replace_once(
    """        Ok(ProviderDispatchExecution::Recorded(
            ProviderDispatchWorkerResult {
                dispatch_replayed: dispatch_result.replayed,
                response_replayed: response_result.replayed,
                response_reconciliation,
                response,
            },
        ))
""",
    """        Ok(ProviderDispatchExecution::Recorded(Box::new(
            ProviderDispatchWorkerResult {
                dispatch_replayed: dispatch_result.replayed,
                response_replayed: response_result.replayed,
                response_reconciliation,
                response,
            },
        )))
""",
    "box recorded construction",
)
replace_once(
    """fn response_reconciliation_error(error: SdkError) -> SdkError {
    if is_response_reconciliation_conflict(&error) {
        return conflicting_provider_replay();
    }
    error
}

""",
    "",
    "remove obsolete reconciliation mapper",
)

path.write_text(text)
print("finalized structured provider conflict lint shape")
