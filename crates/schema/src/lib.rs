//! # samaritan-schema
//!
//! Every contract in the Samaritan pipeline, as a type. No behaviour.
//!
//! This crate is the single source of truth in code, mirroring `SCHEMA.md`.
//! It depends on nothing else in the workspace — the language cannot be
//! defined in terms of the things that speak it.
//!
//! Stage 1 fills this in. Stage 0 only establishes that it compiles and is
//! reachable.

#[cfg(test)]
mod tests {
    #[test]
    fn crate_builds() {
        // Placeholder so `cargo test` has something green to run in stage 0.
        // Replaced by real schema round-trip tests in stage 1.
    }
}
