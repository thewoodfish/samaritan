//! The typed relation graph and its traversal primitives.

use std::collections::BTreeMap;

use crate::config::RelationsConfig;

/// A qualified reference to one observable or attribute, e.g.
/// `haul_cycle.cycle_time`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct NodeRef {
    pub subject: String,
    pub field: String,
}

impl NodeRef {
    /// Parse `subject.field`. `None` if there is no `.`.
    pub fn parse(s: &str) -> Option<NodeRef> {
        s.split_once('.').map(|(subject, field)| NodeRef {
            subject: subject.to_owned(),
            field: field.to_owned(),
        })
    }

    pub fn qualified(&self) -> String {
        format!("{}.{}", self.subject, self.field)
    }
}

/// The mode of a decomposition. Distinguished in the type, not a string, so a
/// walk can never treat a product as a sum.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DecompMode {
    /// The whole is the sum of its parts; parts share the whole's unit.
    Additive,
    /// The whole is the product of its parts; parts differ by unit.
    Multiplicative,
}

impl DecompMode {
    pub fn parse(s: &str) -> Option<DecompMode> {
        match s {
            "additive" => Some(DecompMode::Additive),
            "multiplicative" => Some(DecompMode::Multiplicative),
            _ => None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Decompose {
    pub whole: NodeRef,
    pub mode: DecompMode,
    pub parts: Vec<NodeRef>,
}

#[derive(Debug, Clone)]
pub struct Partition {
    pub metric: NodeRef,
    /// Attribute names, in diagnostic order.
    pub by: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct Confound {
    pub factor: NodeRef,
    pub affects: NodeRef,
    /// Free text: a framing in which this confounder applies. Guidance for the
    /// model, not machinery.
    pub conditional: Option<String>,
}

#[derive(Debug, Clone)]
pub struct Influence {
    pub from: NodeRef,
    pub to: NodeRef,
    /// Seconds before the effect appears.
    pub lag: u64,
    /// Seconds the effect outlasts its cause.
    pub persistence: u64,
}

impl Influence {
    /// How far back a window must widen to capture this cause: `lag +
    /// persistence` (`REGISTRY.md`, influences).
    pub fn widening(&self) -> u64 {
        self.lag + self.persistence
    }
}

#[derive(Debug, Clone)]
pub struct RollsUp {
    pub from: String,
    pub to: String,
}

/// Why a relations config could not be built into a typed graph. These are the
/// structural failures; reference resolution against the vocabulary is the
/// registry's job.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BuildError {
    BadNodeRef(String),
    MissingMode(String),
    BadMode { whole: String, mode: String },
}

/// A node reached during a walk, with how it was reached.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reached {
    pub node: NodeRef,
    pub depth: u32,
    /// Accumulated `lag + persistence` along the path from the start — how far
    /// back the window must widen to include this cause.
    pub window_widening: u64,
}

/// The typed, walkable relation graph.
#[derive(Debug, Clone, Default)]
pub struct RelationGraph {
    decomposes: Vec<Decompose>,
    partitions: Vec<Partition>,
    confounds: Vec<Confound>,
    influences: Vec<Influence>,
    rolls_up: Vec<RollsUp>,
}

fn node(s: &str) -> Result<NodeRef, BuildError> {
    NodeRef::parse(s).ok_or_else(|| BuildError::BadNodeRef(s.to_owned()))
}

impl RelationGraph {
    /// Build the typed graph from parsed config. Structural failures only —
    /// the registry checks references against the vocabulary separately.
    pub fn from_config(cfg: &RelationsConfig) -> Result<RelationGraph, BuildError> {
        let mut g = RelationGraph::default();

        for d in &cfg.decomposes {
            let mode = match &d.mode {
                None => return Err(BuildError::MissingMode(d.whole.clone())),
                Some(m) => DecompMode::parse(m).ok_or_else(|| BuildError::BadMode {
                    whole: d.whole.clone(),
                    mode: m.clone(),
                })?,
            };
            g.decomposes.push(Decompose {
                whole: node(&d.whole)?,
                mode,
                parts: d.parts.iter().map(|p| node(p)).collect::<Result<_, _>>()?,
            });
        }
        for p in &cfg.partitions {
            g.partitions.push(Partition {
                metric: node(&p.metric)?,
                by: p.by.clone(),
            });
        }
        for c in &cfg.confounds {
            g.confounds.push(Confound {
                factor: node(&c.factor)?,
                affects: node(&c.affects)?,
                conditional: c.conditional.clone(),
            });
        }
        for i in &cfg.influences {
            g.influences.push(Influence {
                from: node(&i.from)?,
                to: node(&i.to)?,
                lag: i.lag.unwrap_or(0),
                persistence: i.persistence.unwrap_or(0),
            });
        }
        for r in &cfg.rolls_up {
            g.rolls_up.push(RollsUp {
                from: r.from.clone(),
                to: r.to.clone(),
            });
        }
        Ok(g)
    }

    pub fn decomposes(&self) -> &[Decompose] {
        &self.decomposes
    }
    pub fn partitions(&self) -> &[Partition] {
        &self.partitions
    }
    pub fn confounds(&self) -> &[Confound] {
        &self.confounds
    }
    pub fn influences(&self) -> &[Influence] {
        &self.influences
    }
    pub fn rolls_up(&self) -> &[RollsUp] {
        &self.rolls_up
    }

    // ---- neighbour lookups -------------------------------------------------

    /// The decomposition of `whole`, if it has one.
    pub fn decomposition(&self, whole: &NodeRef) -> Option<&Decompose> {
        self.decomposes.iter().find(|d| &d.whole == whole)
    }

    /// The diagnostic partition keys for `metric`.
    pub fn partition_keys(&self, metric: &NodeRef) -> &[String] {
        self.partitions
            .iter()
            .find(|p| &p.metric == metric)
            .map(|p| p.by.as_slice())
            .unwrap_or(&[])
    }

    /// The factors that confound `affected` — alternatives to rule out.
    pub fn confounders_of(&self, affected: &NodeRef) -> Vec<&Confound> {
        self.confounds
            .iter()
            .filter(|c| &c.affects == affected)
            .collect()
    }

    /// Influence edges pointing *into* `node` — its upstream causes.
    pub fn upstream(&self, node: &NodeRef) -> Vec<&Influence> {
        self.influences.iter().filter(|i| &i.to == node).collect()
    }

    /// Influence edges leading *out of* `node` — its downstream effects.
    pub fn downstream(&self, node: &NodeRef) -> Vec<&Influence> {
        self.influences.iter().filter(|i| &i.from == node).collect()
    }

    // ---- walks -------------------------------------------------------------

    /// Every node reachable by expanding decompositions from `root`, bounded by
    /// `max_depth`, never revisiting a node within the walk. `root` itself is
    /// not included.
    pub fn decomposition_walk(&self, root: &NodeRef, max_depth: u32) -> Vec<NodeRef> {
        let mut visited: Vec<NodeRef> = Vec::new();
        let mut stack: Vec<(NodeRef, u32)> = vec![(root.clone(), 0)];
        let mut seen: Vec<NodeRef> = vec![root.clone()];
        while let Some((cur, depth)) = stack.pop() {
            if depth >= max_depth {
                continue;
            }
            if let Some(dec) = self.decomposition(&cur) {
                for part in &dec.parts {
                    if !seen.contains(part) {
                        seen.push(part.clone());
                        visited.push(part.clone());
                        stack.push((part.clone(), depth + 1));
                    }
                }
            }
        }
        visited.sort();
        visited
    }

    /// Walk influence edges *backward* from `start` — from effect to cause —
    /// bounded by `max_depth`, never revisiting a node. Each node carries the
    /// accumulated `lag + persistence` along the path, the amount the query
    /// window must widen to include it. When a node is reachable by several
    /// paths, the largest widening is kept (the most conservative).
    pub fn backward_influence_walk(&self, start: &NodeRef, max_depth: u32) -> Vec<Reached> {
        // node -> best (depth, widening) seen so far.
        let mut best: BTreeMap<NodeRef, (u32, u64)> = BTreeMap::new();
        // frontier: (node, depth, widening_so_far)
        let mut frontier: Vec<(NodeRef, u32, u64)> = vec![(start.clone(), 0, 0)];

        while let Some((cur, depth, widening)) = frontier.pop() {
            if depth >= max_depth {
                continue;
            }
            for edge in self.upstream(&cur) {
                let cause = edge.from.clone();
                if cause == *start {
                    continue; // never fold the start back in
                }
                let new_widening = widening + edge.widening();
                let new_depth = depth + 1;
                // Keep the largest widening when a node is reached by several
                // paths — the most conservative amount to widen the window.
                let improved = best.get(&cause).is_none_or(|(_, w)| *w < new_widening);
                if improved {
                    best.insert(cause.clone(), (new_depth, new_widening));
                    frontier.push((cause, new_depth, new_widening));
                }
            }
        }

        let mut out: Vec<Reached> = best
            .into_iter()
            .map(|(node, (depth, window_widening))| Reached {
                node,
                depth,
                window_widening,
            })
            .collect();
        out.sort_by(|a, b| a.node.cmp(&b.node));
        out
    }
}
