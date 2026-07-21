mod fields;
mod registry;

#[cfg(test)]
mod tests;

pub(crate) use registry::{
    BootstrapVisibilityResource, build_bootstrap_visibility_registry,
};
