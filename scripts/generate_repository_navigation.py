#!/usr/bin/env python3
"""Correct the temporary repository-step-12 batch-2 materializer.

The executable interface intentionally retains the permanent ``--write`` and
``--check`` markers. The canonical generator is restored before acceptance.
"""

from __future__ import annotations

from pathlib import Path
import subprocess

MATERIALIZER_COMMIT = "d77822675fb106b81305c6e98c7add916b00d7ba"

source = subprocess.check_output(
    [
        "git",
        "show",
        f"{MATERIALIZER_COMMIT}:scripts/generate_repository_navigation.py",
    ],
    text=True,
)

identity_insertion = '''    replace_once(
        root,
        "crates/crm-identity-resolution-capability-composition/src/lib.rs",
        "#![forbid(unsafe_code)]\\n",
        "#![forbid(unsafe_code)]\\n\\nmod production_contribution;\\npub use production_contribution::*;\\n",
    )
'''
customer_data_insertion = '''    replace_once(
        root,
        "crates/crm-customer-data-operations-execution-composition/src/lib.rs",
        "#![forbid(unsafe_code)]\\n",
        "#![forbid(unsafe_code)]\\n\\nmod production_contribution;\\npub use production_contribution::*;\\n",
    )
'''
data_quality_insertion = '''    replace_once(
        root,
        "crates/crm-data-quality-source-composition/src/lib.rs",
        "mod materialization_sink;\\n",
        "mod capability_execution;\\nmod registration;\\nmod production_contribution;\\npub use capability_execution::DataQualityCapabilityExecutor;\\npub use production_contribution::*;\\npub use registration::DataQualityAggregatePlanner;\\n\\nmod materialization_sink;\\n",
    )
'''
materialize_anchor = "def materialize(root: Path) -> None:\n"
materialize_replacement = '''def materialize(root: Path) -> None:
    contribution_declaration = "mod production_contribution;\\npub use production_contribution::*;"
    duplicate_declaration = (
        contribution_declaration + "\\n\\n" + contribution_declaration
    )
    for relative in (
        "crates/crm-identity-resolution-capability-composition/src/lib.rs",
        "crates/crm-customer-data-operations-execution-composition/src/lib.rs",
    ):
        path = root / relative
        content = path.read_text(encoding="utf-8")
        while duplicate_declaration in content:
            content = content.replace(duplicate_declaration, contribution_declaration, 1)
        path.write_text(content, encoding="utf-8")

    data_quality_path = root / "crates/crm-data-quality-source-composition/src/lib.rs"
    data_quality_content = data_quality_path.read_text(encoding="utf-8")
    data_quality_lines = (
        "mod capability_execution;",
        "mod production_contribution;",
        "mod registration;",
        "pub use capability_execution::DataQualityCapabilityExecutor;",
        "pub use production_contribution::*;",
        "pub use registration::DataQualityAggregatePlanner;",
    )
    retained_lines = [
        line
        for line in data_quality_content.splitlines()
        if line not in data_quality_lines
    ]
    data_quality_content = "\\n".join(retained_lines) + "\\n"
    data_quality_block = (
        "mod capability_execution;\\n"
        "mod production_contribution;\\n"
        "mod registration;\\n"
        "pub use capability_execution::DataQualityCapabilityExecutor;\\n"
        "pub use production_contribution::*;\\n"
        "pub use registration::DataQualityAggregatePlanner;\\n\\n"
    )
    anchor = "mod materialization_sink;\\n"
    if data_quality_content.count(anchor) != 1:
        raise RuntimeError("Data Quality materialization sink anchor is not unique")
    data_quality_path.write_text(
        data_quality_content.replace(anchor, data_quality_block + anchor, 1),
        encoding="utf-8",
    )
'''
resolution_before = '''    subprocess.run(["cargo", "metadata", "--format-version", "1", "--no-deps"], cwd=root, check=True, stdout=subprocess.DEVNULL)
'''
resolution_after = '''    native_path = root / "crates/crm-application-runtime/src/native_composition.rs"
    native_content = native_path.read_text(encoding="utf-8")
    residual_data_quality_queries = """    let data_quality_queries = Arc::new(DataQualityQueryAdapter::new(
        store.clone(),
        visibility_authorizer.clone(),
    ));
    add_activated_queries(
        &mut contributions,
        data_quality_query_capability_definitions()?,
        data_quality_queries,
        activation,
    )?;

"""
    if residual_data_quality_queries in native_content:
        native_content = native_content.replace(residual_data_quality_queries, "", 1)
        native_path.write_text(native_content, encoding="utf-8")
    elif "DataQualityQueryAdapter::new" in native_content:
        raise RuntimeError("residual Data Quality query bypass changed shape")

    subprocess.run(
        ["cargo", "check", "--workspace", "--all-targets", "--all-features"],
        cwd=root,
        check=True,
    )
'''
identity_candidate_before = (
    "select_definitions(&definitions, CANDIDATE_MUTATION_CAPABILITY_IDS),"
)
identity_candidate_after = (
    "select_definitions(&definitions, &CANDIDATE_MUTATION_CAPABILITY_IDS),"
)
identity_merge_before = (
    "select_definitions(&definitions, MERGE_MUTATION_CAPABILITY_IDS),"
)
identity_merge_after = (
    "select_definitions(&definitions, &MERGE_MUTATION_CAPABILITY_IDS),"
)

for before, after, label in (
    (identity_insertion, "", "Identity Resolution repeated module insertion"),
    (customer_data_insertion, "", "Customer Data Operations repeated module insertion"),
    (data_quality_insertion, "", "Data Quality repeated module insertion"),
    (materialize_anchor, materialize_replacement, "idempotent declaration normalization"),
    (resolution_before, resolution_after, "all-target lockfile and compilation resolution"),
    (identity_candidate_before, identity_candidate_after, "candidate capability selection borrowing"),
    (identity_merge_before, identity_merge_after, "merge capability selection borrowing"),
):
    if source.count(before) != 1:
        raise RuntimeError(f"temporary {label} anchor is not unique")
    source = source.replace(before, after, 1)

namespace = {"__name__": "__main__", "__file__": str(Path(__file__).resolve())}
exec(compile(source, namespace["__file__"], "exec"), namespace)
