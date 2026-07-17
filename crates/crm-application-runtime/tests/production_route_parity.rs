use crm_application_runtime::{application_mutation_definitions, application_query_definitions};
use crm_capability_runtime::CapabilityDefinition;
use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::PathBuf;

type RouteCoordinate = (String, String, String);

#[derive(Debug, Deserialize)]
struct BindingRegistry {
    schema_version: String,
    modules: Vec<BindingModule>,
}

#[derive(Debug, Deserialize)]
struct BindingModule {
    module_id: String,
    capabilities: Vec<BindingCapability>,
}

#[derive(Debug, Deserialize)]
struct BindingCapability {
    id: String,
    version: String,
}

#[derive(Debug, Deserialize)]
struct RouteClassifications {
    schema_version: String,
    platform_runtime_routes: Vec<ClassifiedRoute>,
    non_runtime_contract_routes: Vec<ClassifiedRoute>,
    empty_runtime_modules: Vec<ClassifiedModule>,
}

#[derive(Debug, Deserialize)]
struct ClassifiedRoute {
    owner_module_id: String,
    id: String,
    version: String,
    reason: String,
}

#[derive(Debug, Deserialize)]
struct ClassifiedModule {
    module_id: String,
    reason: String,
}

#[test]
fn production_routes_exactly_cover_governed_bindings_and_exact_classifications() {
    let registry = binding_registry();
    let classifications = route_classifications();
    assert_eq!(registry.schema_version, "crm.contract-bindings/v1");
    assert_eq!(
        classifications.schema_version,
        "crm.production-route-classifications/v1"
    );

    let governed_modules: BTreeSet<String> = registry
        .modules
        .iter()
        .map(|module| module.module_id.clone())
        .collect();
    let governed_routes = unique_routes(
        registry.modules.iter().flat_map(|module| {
            module.capabilities.iter().map(|capability| {
                (
                    module.module_id.clone(),
                    capability.id.clone(),
                    capability.version.clone(),
                )
            })
        }),
        "governed contract bindings",
    );
    let bound_empty_modules: BTreeSet<String> = registry
        .modules
        .iter()
        .filter(|module| module.capabilities.is_empty())
        .map(|module| module.module_id.clone())
        .collect();

    let platform_routes = classified_routes(
        &classifications.platform_runtime_routes,
        "platform runtime routes",
    );
    let non_runtime_routes = classified_routes(
        &classifications.non_runtime_contract_routes,
        "non-runtime contract routes",
    );
    assert!(
        platform_routes.is_disjoint(&non_runtime_routes),
        "route classifications overlap"
    );
    assert!(
        platform_routes
            .iter()
            .all(|(owner, _, _)| !governed_modules.contains(owner)),
        "platform routes may not hide a governed module route"
    );
    assert!(
        non_runtime_routes.is_subset(&governed_routes),
        "non-runtime routes must name governed coordinates"
    );

    let classified_empty_modules = unique_modules(&classifications.empty_runtime_modules);
    assert_eq!(
        classified_empty_modules, bound_empty_modules,
        "route-less module classifications drifted from governed bindings"
    );

    let mutation_definitions = application_mutation_definitions().unwrap();
    let query_definitions = application_query_definitions().unwrap();
    assert_route_kinds_are_disjoint(&mutation_definitions, &query_definitions);
    let actual_routes =
        unique_runtime_routes(mutation_definitions.iter().chain(query_definitions.iter()));

    let mut expected_routes = governed_routes
        .difference(&non_runtime_routes)
        .cloned()
        .collect::<BTreeSet<_>>();
    expected_routes.extend(platform_routes);
    assert_eq!(
        actual_routes, expected_routes,
        "manifest/binding/production route parity drifted"
    );
}

fn binding_registry() -> BindingRegistry {
    serde_json::from_slice(
        &fs::read(root().join("contracts/module-contract-bindings.json")).unwrap(),
    )
    .unwrap()
}

fn route_classifications() -> RouteClassifications {
    serde_json::from_slice(
        &fs::read(root().join("contracts/production-route-classifications.json")).unwrap(),
    )
    .unwrap()
}

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn classified_routes(routes: &[ClassifiedRoute], label: &str) -> BTreeSet<RouteCoordinate> {
    for route in routes {
        assert!(
            !route.reason.trim().is_empty(),
            "{label} classification lacks a reason"
        );
    }
    unique_routes(
        routes.iter().map(|route| {
            (
                route.owner_module_id.clone(),
                route.id.clone(),
                route.version.clone(),
            )
        }),
        label,
    )
}

fn unique_modules(modules: &[ClassifiedModule]) -> BTreeSet<String> {
    let mut unique = BTreeSet::new();
    for module in modules {
        assert!(
            !module.reason.trim().is_empty(),
            "route-less module classification lacks a reason"
        );
        assert!(
            unique.insert(module.module_id.clone()),
            "duplicate route-less module classification: {}",
            module.module_id
        );
    }
    unique
}

fn unique_runtime_routes<'a>(
    definitions: impl IntoIterator<Item = &'a CapabilityDefinition>,
) -> BTreeSet<RouteCoordinate> {
    unique_routes(
        definitions.into_iter().map(route_coordinate),
        "production routes",
    )
}

fn unique_routes(
    routes: impl IntoIterator<Item = RouteCoordinate>,
    label: &str,
) -> BTreeSet<RouteCoordinate> {
    let routes = routes.into_iter().collect::<Vec<_>>();
    let unique = routes.iter().cloned().collect::<BTreeSet<_>>();
    assert_eq!(
        routes.len(),
        unique.len(),
        "duplicate coordinate in {label}"
    );
    unique
}

fn assert_route_kinds_are_disjoint(
    mutations: &[CapabilityDefinition],
    queries: &[CapabilityDefinition],
) {
    let mutation_routes: BTreeMap<RouteCoordinate, ()> = mutations
        .iter()
        .map(|definition| (route_coordinate(definition), ()))
        .collect();
    for definition in queries {
        let coordinate = route_coordinate(definition);
        assert!(
            !mutation_routes.contains_key(&coordinate),
            "production route is registered as both mutation and query: {coordinate:?}"
        );
    }
}

fn route_coordinate(definition: &CapabilityDefinition) -> RouteCoordinate {
    (
        definition.owner_module_id.as_str().to_owned(),
        definition.capability_id.as_str().to_owned(),
        definition.capability_version.as_str().to_owned(),
    )
}
