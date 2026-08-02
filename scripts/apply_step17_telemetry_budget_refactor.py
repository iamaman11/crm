#!/usr/bin/env python3
"""Make Step 17 telemetry budget-neutral without changing behavior."""

from __future__ import annotations

from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def read(path: str) -> str:
    return (ROOT / path).read_text(encoding="utf-8")


def write(path: str, content: str) -> None:
    (ROOT / path).write_text(content, encoding="utf-8")


def replace_once(path: str, old: str, new: str) -> None:
    content = read(path)
    count = content.count(old)
    if count != 1:
        raise RuntimeError(f"{path}: expected one target, found {count}: {old[:100]!r}")
    write(path, content.replace(old, new, 1))


# Private generated catalog is included directly in runtime; remove two public items.
replace_once(
    "crates/crm-application-runtime/src/lib.rs",
    "mod generated_contract_telemetry;\n",
    "",
)
replace_once(
    "crates/crm-application-runtime/src/lib.rs",
    '\npub const CRATE_NAME: &str = "crm-application-runtime";\n',
    "",
)
replace_once(
    "crates/crm-application-runtime/src/runtime.rs",
    "use crate::generated_contract_telemetry::DEPRECATED_CONTRACTS;\n",
    'include!("generated_contract_telemetry.rs");\n',
)

# Preserve Debug behavior while removing process-host boilerplate.
replace_once(
    "crates/crm-application-runtime/src/runtime.rs",
    '''impl fmt::Debug for ApplicationComponents {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ApplicationComponents")
            .field("module_count", &self.module_ids.len())
            .field("tenant_count", &self.tenant_ids.len())
            .field("ready", &self.is_ready())
            .finish()
    }
}
''',
    '''impl fmt::Debug for ApplicationComponents {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ApplicationComponents")
            .finish_non_exhaustive()
    }
}
''',
)
replace_once(
    "crates/crm-application-runtime/src/runtime.rs",
    '''pub struct ApplicationRuntime {
    config: ApplicationConfig,
    components: Arc<ApplicationComponents>,
}

impl fmt::Debug for ApplicationRuntime {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ApplicationRuntime")
            .field("http_bind", &self.config.http_bind)
            .field("grpc_bind", &self.config.grpc_bind)
            .field("components", &self.components)
            .finish()
    }
}
''',
    '''#[derive(Debug)]
pub struct ApplicationRuntime {
    config: ApplicationConfig,
    components: Arc<ApplicationComponents>,
}
''',
)

# Return registries and renderer together; no callback state in the process host.
replace_once(
    "crates/crm-capability-adapters/src/contract_usage_telemetry.rs",
    '''/// The callback receives a deterministic Prometheus renderer sharing the same
/// counters. Recording remains synchronous and cannot alter gateway outcomes.
pub fn meter_contract_registries<F>(
    catalog: &'static [(&'static str, &'static str, &'static str, &'static str, u32)],
    registries: [Arc<dyn CapabilityRegistryPort>; 2],
    definitions: [&[CapabilityDefinition]; 2],
    publish_metrics: F,
) -> Result<[Arc<dyn CapabilityRegistryPort>; 2], SdkError>
where
    F: FnOnce(Arc<dyn Fn() -> String + Send + Sync>),
{
''',
    '''/// The returned renderer shares the same counters. Recording remains
/// synchronous and cannot alter gateway outcomes.
pub fn meter_contract_registries(
    catalog: &'static [(&'static str, &'static str, &'static str, &'static str, u32)],
    registries: [Arc<dyn CapabilityRegistryPort>; 2],
    definitions: [&[CapabilityDefinition]; 2],
) -> Result<
    (
        [Arc<dyn CapabilityRegistryPort>; 2],
        Arc<dyn Fn() -> String + Send + Sync>,
    ),
    SdkError,
> {
''',
)
replace_once(
    "crates/crm-capability-adapters/src/contract_usage_telemetry.rs",
    '''    let renderer: Arc<dyn Fn() -> String + Send + Sync> =
        Arc::new(move || renderer_metrics.render_prometheus());
    publish_metrics(renderer);

''',
    '''    let renderer: Arc<dyn Fn() -> String + Send + Sync> =
        Arc::new(move || renderer_metrics.render_prometheus());

''',
)
replace_once(
    "crates/crm-capability-adapters/src/contract_usage_telemetry.rs",
    "    Ok([mutation, query])\n",
    "    Ok(([mutation, query], renderer))\n",
)

runtime_old = '''        let mut contract_usage_metrics_text: Arc<dyn Fn() -> String + Send + Sync> =
            Arc::new(String::new);
        let [mutation_registry, query_registry] = meter_contract_registries(
            DEPRECATED_CONTRACTS,
            [
                composition.mutation_registry(),
                composition.query_registry(),
            ],
            [&mutation_definitions, &query_definitions],
            |renderer| contract_usage_metrics_text = renderer,
        )
        .map_err(|error| ApplicationRuntimeError::Assembly(error.to_string()))?;
'''
runtime_new = '''        let ([mutation_registry, query_registry], contract_usage_metrics_text) =
            meter_contract_registries(
                DEPRECATED_CONTRACTS,
                [
                    composition.mutation_registry(),
                    composition.query_registry(),
                ],
                [&mutation_definitions, &query_definitions],
            )
            .map_err(|error| ApplicationRuntimeError::Assembly(error.to_string()))?;
'''
replace_once("crates/crm-application-runtime/src/runtime.rs", runtime_old, runtime_new)
replace_once(
    "crates/crm-application-runtime/src/runtime.rs",
    '''async fn metrics(State(state): State<HttpState>) -> impl IntoResponse {
    (
        StatusCode::OK,
        (state.components.contract_usage_metrics_text)(),
    )
}
''',
    '''async fn metrics(State(state): State<HttpState>) -> String {
    (state.components.contract_usage_metrics_text)()
}
''',
)

adapter = "crates/crm-capability-adapters/src/contract_usage_telemetry.rs"
replace_once(
    adapter,
    '''        let mut render = None;
        let registries: [Arc<dyn CapabilityRegistryPort>; 2] = [
''',
    '''        let registries: [Arc<dyn CapabilityRegistryPort>; 2] = [
''',
)
replace_once(
    adapter,
    '''        let [mutation, query] = meter_contract_registries(
            CATALOG,
            registries,
            [std::slice::from_ref(&definition), &[]],
            |renderer| render = Some(renderer),
        )
        .unwrap();
        let render = render.unwrap();
''',
    '''        let ([mutation, query], render) = meter_contract_registries(
            CATALOG,
            registries,
            [std::slice::from_ref(&definition), &[]],
        )
        .unwrap();
''',
)
replace_once(
    adapter,
    '''        let mut render = None;
        let registries: [Arc<dyn CapabilityRegistryPort>; 2] = [
''',
    '''        let registries: [Arc<dyn CapabilityRegistryPort>; 2] = [
''',
)
replace_once(
    adapter,
    '''        let [mutation, _] = meter_contract_registries(
            CATALOG,
            registries,
            [std::slice::from_ref(&definition), &[]],
            |renderer| render = Some(renderer),
        )
        .unwrap();
''',
    '''        let ([mutation, _], render) = meter_contract_registries(
            CATALOG,
            registries,
            [std::slice::from_ref(&definition), &[]],
        )
        .unwrap();
''',
)
replace_once(adapter, "        assert!(render.unwrap()().ends_with(\" 0\\n\"));\n", "        assert!(render().ends_with(\" 0\\n\"));\n")
replace_once(
    adapter,
    '''            [&[], &[]],
            |_| {},
        )
''',
    '''            [&[], &[]],
        )
''',
)
replace_once(
    adapter,
    '''            [&[wrong_owner], &[]],
            |_| {},
        )
''',
    '''            [&[wrong_owner], &[]],
        )
''',
)
replace_once(
    adapter,
    '''        let mut render = None;
        meter_contract_registries(
            CATALOG,
            [Arc::clone(&no_registry), no_registry],
            [&[definition], &[]],
            |renderer| render = Some(renderer),
        )
        .unwrap();
        let render = render.unwrap();
''',
    '''        let (_, render) = meter_contract_registries(
            CATALOG,
            [Arc::clone(&no_registry), no_registry],
            [&[definition], &[]],
        )
        .unwrap();
''',
)

# Generator emits a private rustfmt-stable constant for direct include!.
generator = "scripts/generate_contract_telemetry_catalog.py"
replace_once(
    generator,
    '            "pub(crate) const DEPRECATED_CONTRACTS: &[(&str, &str, &str, &str, u32)] = &[];\\n"\n',
    '            "const DEPRECATED_CONTRACTS: &[(&str, &str, &str, &str, u32)] = &[];\\n"\n',
)
replace_once(
    generator,
    '        "pub(crate) const DEPRECATED_CONTRACTS: &[(&str, &str, &str, &str, u32)] = &[\\n"\n',
    '        "const DEPRECATED_CONTRACTS: &[(&str, &str, &str, &str, u32)] = &[\\n"\n',
)
write(
    "crates/crm-application-runtime/src/generated_contract_telemetry.rs",
    "// @generated by scripts/generate_contract_telemetry_catalog.py; do not edit.\n"
    "const DEPRECATED_CONTRACTS: &[(&str, &str, &str, &str, u32)] = &[];\n",
)

print("Step 17 telemetry budget refactor applied.")
