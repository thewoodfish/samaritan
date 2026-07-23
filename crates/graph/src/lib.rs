//! # samaritan-graph
//!
//! The relation graph as a walkable structure — the machine-checkable subset
//! of `GRAPH.md`, loaded from `REGISTRY.md`'s relations section.
//!
//! Pure and traversable. An analyzer walks this; it contains no mining
//! knowledge of its own — every edge comes from the registry.
//!
//! Built in stage 3.

#[cfg(test)]
mod tests {
    #[test]
    fn crate_builds() {}
}
