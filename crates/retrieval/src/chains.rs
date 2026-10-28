//! Quotation chains (specs/19, "Cross-resource links", item 7; specs/03, "Translation
//! heritage UX"): A quotes B, which quotes C, traversed with explicit depth limits, cycle
//! detection and path deduplication. Every hop carries its edge's provenance, method, source
//! work and review state, and a hop whose basis is not documented stops the chain with
//! [`BASIS_NOT_DOCUMENTED`] instead of being drawn.

use serde::{Deserialize, Serialize};

use crate::{Direction, Edge, Locator, ProvenanceKind};

/// Review state of a claim (specs/03, "Typed claims"), as an edge carries it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReviewState {
    /// Generated or extracted, not yet attested or reviewed.
    Proposed,
    /// Attested by the source that asserts it (a curated set, a parser's documented rule).
    SourceAttested,
    /// Reviewed by a person.
    Reviewed,
    /// Published.
    Published,
    /// Rejected on review.
    Rejected,
    /// Published but disputed; must stay visibly attributed.
    Disputed,
    /// Replaced by a later claim.
    Superseded,
    /// Withdrawn.
    Retracted,
}

impl ReviewState {
    /// The name stored in packs and the link index.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            ReviewState::Proposed => "proposed",
            ReviewState::SourceAttested => "source_attested",
            ReviewState::Reviewed => "reviewed",
            ReviewState::Published => "published",
            ReviewState::Rejected => "rejected",
            ReviewState::Disputed => "disputed",
            ReviewState::Superseded => "superseded",
            ReviewState::Retracted => "retracted",
        }
    }

    /// Parses the stored name.
    #[must_use]
    pub fn parse(text: &str) -> Option<ReviewState> {
        Some(match text {
            "proposed" => ReviewState::Proposed,
            "source_attested" => ReviewState::SourceAttested,
            "reviewed" => ReviewState::Reviewed,
            "published" => ReviewState::Published,
            "rejected" => ReviewState::Rejected,
            "disputed" => ReviewState::Disputed,
            "superseded" => ReviewState::Superseded,
            "retracted" => ReviewState::Retracted,
            _ => return None,
        })
    }

    /// Whether a claim in this state may be shown as a documented relation.
    #[must_use]
    pub fn is_documented(self) -> bool {
        matches!(
            self,
            ReviewState::SourceAttested
                | ReviewState::Reviewed
                | ReviewState::Published
                | ReviewState::Disputed
        )
    }
}

/// The message a chain stops with when a hop's basis is not documented.
pub const BASIS_NOT_DOCUMENTED: &str = "basis not documented";

impl Edge {
    /// Whether the edge states its basis: a curated, extracted or user edge whose method
    /// names its source or rule and whose review state is at least source-attested. A
    /// model-similarity row never documents a quotation, whatever its score.
    #[must_use]
    pub fn basis_documented(&self) -> bool {
        self.provenance != ProvenanceKind::ModelSimilarity
            && !self.method.trim().is_empty()
            && self.review_state.is_documented()
    }
}

/// One hop of a chain with its provenance.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChainHop {
    /// Where the hop starts.
    pub from: Locator,
    /// Where it lands.
    pub to: Locator,
    /// Relation type.
    pub rel: String,
    /// Edge provenance.
    pub provenance: ProvenanceKind,
    /// Parser version, model version or curated source name.
    pub method: String,
    /// The pack or work asserting the edge.
    pub source_work: String,
    /// Review state of the edge.
    pub review_state: ReviewState,
    /// Edge weight.
    pub weight: f32,
}

impl ChainHop {
    /// The hop an edge makes when followed in `direction` (`Both` reads as outbound).
    #[must_use]
    pub fn from_edge(edge: &Edge, direction: Direction) -> ChainHop {
        let (from, to) = match direction {
            Direction::Inbound => (edge.dst.clone(), edge.src.clone()),
            _ => (edge.src.clone(), edge.dst.clone()),
        };
        ChainHop {
            from,
            to,
            rel: edge.rel.clone(),
            provenance: edge.provenance,
            method: edge.method.clone(),
            source_work: edge.source_work.clone(),
            review_state: edge.review_state,
            weight: edge.weight,
        }
    }
}

/// Why a chain ends where it does.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "reason", rename_all = "snake_case")]
pub enum ChainStop {
    /// The next hop exists but states no basis; shown as [`BASIS_NOT_DOCUMENTED`].
    BasisNotDocumented {
        /// The edge that was not followed.
        edge: Edge,
    },
    /// The depth limit was reached and the end node has further documented hops.
    DepthLimit,
    /// The node limit was reached before the end node was expanded.
    NodeLimit,
}

impl ChainStop {
    /// Display message.
    #[must_use]
    pub fn message(&self) -> &'static str {
        match self {
            ChainStop::BasisNotDocumented { .. } => BASIS_NOT_DOCUMENTED,
            ChainStop::DepthLimit => "depth limit reached",
            ChainStop::NodeLimit => "node limit reached",
        }
    }
}

/// One chain from the start locator to `end`, with the first path found. A node reached
/// through several intermediate works appears once.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct QuotationChain {
    /// The last node.
    pub end: Locator,
    /// The hops from the start, in order.
    pub hops: Vec<ChainHop>,
    /// Why the chain stops here, if a limit or an undocumented hop stopped it.
    pub stop: Option<ChainStop>,
}

impl QuotationChain {
    /// Hops in the chain.
    #[must_use]
    pub fn depth(&self) -> u8 {
        self.hops.len() as u8
    }
}

/// Limits for [`crate::RelationStore::citation_chains`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChainOptions {
    /// Relation types that count as quotation (`quotes`, `explicitly_cites`); empty means
    /// `quotes` only.
    pub rels: Vec<String>,
    /// Direction to follow.
    pub direction: Direction,
    /// Maximum hops, 1 to 6.
    pub max_depth: u8,
    /// Maximum edges followed per node per hop; 0 means unlimited.
    pub fan_out: u32,
    /// Maximum chains returned.
    pub max_nodes: usize,
}

impl Default for ChainOptions {
    fn default() -> ChainOptions {
        ChainOptions {
            rels: vec!["quotes".to_owned()],
            direction: Direction::Outbound,
            max_depth: 6,
            fan_out: 50,
            max_nodes: 1000,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn edge(provenance: ProvenanceKind, method: &str, state: ReviewState) -> Edge {
        Edge {
            src: Locator {
                work: "a".into(),
                locator: "1".into(),
            },
            rel: "quotes".into(),
            dst: Locator {
                work: "b".into(),
                locator: "2".into(),
            },
            weight: 1.0,
            provenance,
            source_work: "a".into(),
            method: method.into(),
            review_state: state,
        }
    }

    #[test]
    fn documented_basis_needs_provenance_method_and_review_state() {
        assert!(
            edge(ProvenanceKind::Curated, "tsk", ReviewState::SourceAttested).basis_documented()
        );
        assert!(
            edge(ProvenanceKind::Extracted, "parser@1", ReviewState::Reviewed).basis_documented()
        );
        assert!(edge(ProvenanceKind::User, "note", ReviewState::Published).basis_documented());
        assert!(edge(ProvenanceKind::Curated, "tsk", ReviewState::Disputed).basis_documented());
        assert!(
            !edge(
                ProvenanceKind::ModelSimilarity,
                "e5@1",
                ReviewState::Published
            )
            .basis_documented()
        );
        assert!(!edge(ProvenanceKind::Curated, "  ", ReviewState::Published).basis_documented());
        assert!(!edge(ProvenanceKind::Curated, "tsk", ReviewState::Proposed).basis_documented());
        assert!(!edge(ProvenanceKind::Curated, "tsk", ReviewState::Rejected).basis_documented());
        assert!(!edge(ProvenanceKind::Curated, "tsk", ReviewState::Retracted).basis_documented());
    }

    #[test]
    fn hops_follow_the_direction_and_states_round_trip() {
        let e = edge(ProvenanceKind::Curated, "tsk", ReviewState::Reviewed);
        let out = ChainHop::from_edge(&e, Direction::Outbound);
        assert_eq!((out.from.work.as_str(), out.to.work.as_str()), ("a", "b"));
        let inb = ChainHop::from_edge(&e, Direction::Inbound);
        assert_eq!((inb.from.work.as_str(), inb.to.work.as_str()), ("b", "a"));
        for s in [
            ReviewState::Proposed,
            ReviewState::SourceAttested,
            ReviewState::Reviewed,
            ReviewState::Published,
            ReviewState::Rejected,
            ReviewState::Disputed,
            ReviewState::Superseded,
            ReviewState::Retracted,
        ] {
            assert_eq!(ReviewState::parse(s.name()), Some(s));
        }
        let stop = ChainStop::BasisNotDocumented { edge: e };
        assert_eq!(stop.message(), BASIS_NOT_DOCUMENTED);
        assert!(
            serde_json::to_string(&stop)
                .unwrap()
                .contains("basis_not_documented")
        );
        assert_eq!(ChainOptions::default().rels, vec!["quotes"]);
    }
}
