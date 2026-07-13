/// Account mutation planner boundary.
///
/// The production `TransactionalAggregatePlanner` implementation is added in
/// the next 8A.3a step together with exact persisted JSON/event mapping. Keeping
/// the boundary explicit now prevents public capability publication from being
/// coupled to transport or PostgreSQL types.
#[derive(Debug, Default, Clone, Copy)]
pub struct CustomerAccountCapabilityPlanner;
