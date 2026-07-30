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
    """    - customer_data.export_execution_stage
    - customer_data.export_execution_outcome
""",
    """    - customer_data.export_execution_stage
    - customer_data.export_execution_outcome
    - customer_data.privacy_export_job
""",
    "CDO retained private job",
)

packet_path = Path("repository-packet.json")
packet = json.loads(packet_path.read_text(encoding="utf-8"))
path = "modules/crm-customer-data-operations/module.yaml"
if path not in packet["allowed_paths"]:
    packet["allowed_paths"].append(path)
    packet["allowed_paths"].sort()
packet_path.write_text(json.dumps(packet, indent=2) + "\n", encoding="utf-8")
