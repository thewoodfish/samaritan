//! # samaritan-registry
//!
//! Loads the registry and rejects an invalid one, with every check in
//! `REGISTRY.md` (E01–E23, W01–W09).
//!
//! This is where drift between spec and code surfaces first: if the vocabulary
//! and the mappings disagree with themselves, the loader says so.
//!
//! Built in stage 2.

#[cfg(test)]
mod tests {
    #[test]
    fn crate_builds() {}
}
