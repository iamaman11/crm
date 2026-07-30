from __future__ import annotations

import json
from pathlib import Path


def replace_once(path: str, old: str, new: str, label: str) -> None:
    target = Path(path)
    text = target.read_text(encoding="utf-8")
    count = text.count(old)
    if count != 1:
        raise SystemExit(f"{label}: found {count}, expected 1")
    target.write_text(text.replace(old, new), encoding="utf-8")


replace_once(
    "modules/crm-customer-privacy/module.yaml",
    'version: "0.2.0"',
    'version: "0.3.0"',
    "Customer Privacy version bump",
)
replace_once(
    "modules/crm-customer-data-operations/module.yaml",
    "version: 0.3.0",
    "version: 0.4.0",
    "Customer Data Operations version bump",
)
replace_once(
    "modules/crm-customer-data-operations/module.yaml",
    """    - customer_data.export_execution_stage
    - customer_data.export_execution_outcome
  ui_extensions: []
""",
    """    - customer_data.export_execution_stage
    - customer_data.export_execution_outcome
    - customer_data.privacy_export_job
  ui_extensions: []
""",
    "CDO provided private job",
)
replace_once(
    "modules/crm-customer-data-operations/module.yaml",
    """    - customer_data.export_execution_stage
    - customer_data.export_execution_outcome
  private_state_namespaces:
""",
    """    - customer_data.export_execution_stage
    - customer_data.export_execution_outcome
    - customer_data.privacy_export_job
  private_state_namespaces:
""",
    "CDO stored private job",
)
replace_once(
    "modules/crm-customer-data-operations/module.yaml",
    """    - crm.customer-data-operations.export_execution_stage
    - crm.customer-data-operations.export_execution_outcome
security:
""",
    """    - crm.customer-data-operations.export_execution_stage
    - crm.customer-data-operations.export_execution_outcome
    - crm.customer-data-operations.privacy_export_job
security:
""",
    "CDO private namespace",
)
replace_once(
    "modules/crm-customer-data-operations/module.yaml",
    """lifecycle:
  upgrade_policy: manual
  rollback_policy: supported
  uninstall_policy: retain_business_records
  migrations_path: modules/crm-customer-data-operations/migrations
  retained_record_types:
    - customer_data.import_job
    - customer_data.import_row
    - customer_data.export_job
    - customer_data.export_selection_boundary
    - customer_data.export_selection_progress
    - customer_data.export_selection_item
    - customer_data.export_execution_stage
    - customer_data.export_execution_outcome
""",
    """lifecycle:
  upgrade_policy: manual
  rollback_policy: supported
  uninstall_policy: retain_business_records
  migrations_path: modules/crm-customer-data-operations/migrations
  retained_record_types:
    - customer_data.import_job
    - customer_data.import_row
    - customer_data.export_job
    - customer_data.export_selection_boundary
    - customer_data.export_selection_progress
    - customer_data.export_selection_item
    - customer_data.export_execution_stage
    - customer_data.export_execution_outcome
    - customer_data.privacy_export_job
""",
    "CDO retained private job",
)
replace_once(
    "modules/crm-customer-privacy/src/access_export.rs",
    "use sha2::Digest as _;\n",
    "",
    "unused Digest import",
)
replace_once(
    "crates/crm-customer-data-operations-execution-composition/src/privacy_export.rs",
    '    format!("{domain}-{}", hex(&hasher.finalize().into()))\n',
    '    let digest: [u8; 32] = hasher.finalize().into();\n    format!("{domain}-{}", hex(&digest))\n',
    "stable id digest conversion",
)

packet_path = Path("repository-packet.json")
packet = json.loads(packet_path.read_text(encoding="utf-8"))
path = "modules/crm-customer-data-operations/module.yaml"
if path not in packet["allowed_paths"]:
    packet["allowed_paths"].append(path)
    packet["allowed_paths"].sort()
packet_path.write_text(json.dumps(packet, indent=2) + "\n", encoding="utf-8")
