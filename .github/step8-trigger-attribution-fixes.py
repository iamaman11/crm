import re
from pathlib import Path


def replace_once(text: str, old: str, new: str, label: str) -> str:
    count = text.count(old)
    if count != 1:
        raise SystemExit(f"{label}: expected one match, found {count}")
    return text.replace(old, new, 1)


application_path = Path("crates/crm-customer-privacy-application/src/execution.rs")
application = application_path.read_text()
application = replace_once(
    application,
    "use crm_application_composition::ModuleActivationPort;\n",
    """use crate::{
    RETENTION_APPROVAL_TRIGGER_CAPABILITY, RETENTION_LEGAL_HOLD_TRIGGER_CAPABILITY,
    RETENTION_TRIGGER_CAPABILITY_VERSION,
};
use crm_application_composition::ModuleActivationPort;
""",
    "registered execution trigger imports",
)
application = replace_once(
    application,
    """    if invocation.initiating_capability_id.as_str() != OWNER_ACTION_DISPATCH_CAPABILITY
        || invocation.initiating_capability_version.as_str() != OWNER_EXECUTION_CAPABILITY_VERSION
    {
        return Err(execution_configuration_invalid(
            "owner execution invocation uses an unexpected internal coordinate",
        ));
    }
""",
    """    if invocation.initiating_capability_version.as_str() != RETENTION_TRIGGER_CAPABILITY_VERSION
        || !matches!(
            invocation.initiating_capability_id.as_str(),
            RETENTION_APPROVAL_TRIGGER_CAPABILITY | RETENTION_LEGAL_HOLD_TRIGGER_CAPABILITY
        )
    {
        return Err(execution_configuration_invalid(
            "owner execution has no registered initiating Customer Privacy capability",
        ));
    }
""",
    "registered execution trigger validation",
)
test_marker = "\n#[cfg(test)]\n"
if application.count(test_marker) != 1:
    raise SystemExit("application execution test module marker is not unique")
production, tests = application.split(test_marker, 1)
fixture_pattern = re.compile(
    r"(initiating_capability_id\s*:\s*)"
    r".*?"
    r"(,\s*\n\s*initiating_capability_version\s*:)",
    re.DOTALL,
)
tests, fixture_matches = fixture_pattern.subn(
    r"\1CapabilityId::try_new(RETENTION_APPROVAL_TRIGGER_CAPABILITY).unwrap()\2",
    tests,
    count=1,
)
if fixture_matches != 1:
    raise SystemExit(
        f"registered application test trigger: expected one fixture field, found {fixture_matches}"
    )
application_path.write_text(production + test_marker + tests)

test_path = Path(
    "crates/crm-application-runtime/tests/customer_privacy_owner_execution_postgres.rs"
)
test = test_path.read_text()
test = replace_once(
    test,
    """        initiating_capability_id: CapabilityId::try_new("customer_privacy.owner_action.dispatch")
            .unwrap(),
""",
    """        initiating_capability_id: CapabilityId::try_new("customer_privacy.case.approve")
            .unwrap(),
""",
    "registered acceptance trigger",
)
test_path.write_text(test)
