use crm_application_runtime::{application_mutation_definitions, application_query_definitions};
use crm_capability_runtime::CapabilityDefinition;
use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::PathBuf;

const PLATFORM_ROUTE_OWNERS: [&str; 2] = ["crm.metadata", "crm.search"];
const FIRST_PARTY_EMPTY_ROUTE_MODULES: [&str; 1] = ["crm.sales-activities-link"];

type RouteCoordinate = (String, String, String);

#[derive(Debug, Deserialize)]
struct BindingRegistry {
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

#[test]
fn production_routes_exactly_cover_first_party_contract_bindings() {
    let registry = binding_registry();
    let manifest_modules: BTreeSet<String> = registry
        .modules
        .iter()
        .map(|module| module.module_id.clone())
        .collect();
    let bound_routes: BTreeSet<RouteCoordinate> = registry
        .modules
        .iter()
        .flat_map(|module| {
            module.capabilities.iter().map(|capability| {
                (
                    module.module_id.clone(),
                    capability.id.clone(),
                    capability.version.clone(),
                )
            })
        })
        .collect();

    let mutation_definitions = application_mutation_definitions().unwrap();
    let query_definitions = application_query_definitions().unwrap();
    assert_route_kinds_are_disjoint(&mutation_definitions, &query_definitions);

    let runtime_routes =
        unique_runtime_routes(mutation_definitions.iter().chain(query_definitions.iter()));
    let first_party_routes: BTreeSet<RouteCoordinate> = runtime_routes
        .iter()
        .filter(|(module_id, _, _)| manifest_modules.contains(module_id))
        .cloned()
        .collect();

    assert_eq!(
        first_party_routes, bound_routes,
        "production route coverage drifted from module contract bindings"
    );

    let platform_owners: BTreeSet<String> = runtime_routes
        .iter()
        .map(|(module_id, _, _)| module_id.clone())
        .filter(|module_id| !manifest_modules.contains(module_id))
        .collect();
    assert_eq!(
        platform_owners,
        PLATFORM_ROUTE_OWNERS
            .into_iter()
            .map(str::to_owned)
            .collect(),
        "a non-manifest production route owner requires explicit platform classification"
    );

    let first_party_route_owners: BTreeSet<String> = first_party_routes
        .iter()
        .map(|(module_id, _, _)| module_id.clone())
        .collect();
    let empty_route_modules: BTreeSet<String> = manifest_modules
        .difference(&first_party_route_owners)
        .cloned()
        .collect();
    assert_eq!(
        empty_route_modules,
        FIRST_PARTY_EMPTY_ROUTE_MODULES
            .into_iter()
            .map(str::to_owned)
            .collect(),
        "a first-party module without production routes requires explicit classification"
    );
}

fn binding_registry() -> BindingRegistry {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../contracts/module-contract-bindings.json");
    serde_json::from_slice(&fs::read(path).unwrap()).unwrap()
}

fn unique_runtime_routes<'a>(
    definitions: impl IntoIterator<Item = &'a CapabilityDefinition>,
) -> BTreeSet<RouteCoordinate> {
    let mut routes = BTreeSet::new();
    for definition in definitions {
        let coordinate = route_coordinate(definition);
        assert!(
            routes.insert(coordinate.clone()),
            "duplicate production route coordinate: {coordinate:?}"
        );
    }
    routes
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
