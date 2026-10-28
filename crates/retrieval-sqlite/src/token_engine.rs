//! The columnar token engine: dense feature columns and per-container maps held in memory
//! per edition, roaring postings read from the pack per `(feature, value)`, and a join over
//! slots for proximity, agreement and structural constraints (specs/19 item 3).

use std::cell::RefCell;
use std::collections::HashMap;

use openlogos_retrieval::{
    Constraint, Facet, Result, RetrievalError, TokenHits, TokenMatch, TokenPos, TokenQuery, Unit,
    VerseId,
};
use roaring::RoaringBitmap;
use rusqlite::{Connection, params};

use crate::vectors::storage;

/// One feature's dictionary and dense column.
#[derive(Debug)]
pub struct FeatureColumn {
    /// Feature id in the pack.
    pub id: i64,
    /// Value text by value id (index 0 is the absent value, empty).
    pub values: Vec<String>,
    /// Value id per position.
    pub ids: Vec<u32>,
    /// Value text → value id.
    pub lookup: HashMap<String, u32>,
}

/// One container kind: its nodes and the innermost node per token.
#[derive(Debug)]
pub struct ContainerLayer {
    /// This kind's id in the pack.
    pub kind_id: u32,
    /// `(parent id, first pos, last pos)` by container id − 1.
    pub nodes: Vec<(u32, u32, u32)>,
    /// Innermost container id per position (0 = none).
    pub innermost: Vec<u32>,
    /// Kind of every container id − 1 (indexes `ContainerLayer`s by kind id − 1).
    pub kind_of: std::rc::Rc<Vec<u32>>,
}

/// Everything the engine keeps in memory for one edition's token layer.
#[derive(Debug)]
pub struct TokenColumns {
    /// Verse id per position.
    pub verse: Vec<u32>,
    /// Feature columns by name, in pack order.
    pub features: Vec<(String, FeatureColumn)>,
    /// Container layers by kind name.
    pub containers: HashMap<String, ContainerLayer>,
    /// Postings cache: `(feature id, value id)` → bitmap.
    postings: RefCell<HashMap<(i64, u32), std::rc::Rc<RoaringBitmap>>>,
}

fn u32s(blob: &[u8]) -> Vec<u32> {
    blob.as_chunks::<4>()
        .0
        .iter()
        .map(|c| u32::from_le_bytes(*c))
        .collect()
}

impl TokenColumns {
    /// Loads the columns of the pack attached as `schema`.
    pub fn load(conn: &Connection, schema: &str) -> Result<TokenColumns> {
        let verse: Vec<u32> = {
            let mut stmt = conn
                .prepare(&format!(
                    "SELECT verse_id FROM \"{schema}\".token ORDER BY pos"
                ))
                .map_err(storage)?;
            let rows = stmt
                .query_map([], |r| r.get::<_, i64>(0))
                .map_err(storage)?;
            rows.map(|r| r.map(|v| v as u32))
                .collect::<std::result::Result<_, _>>()
                .map_err(storage)?
        };
        let mut features = Vec::new();
        {
            let mut stmt = conn
                .prepare(&format!(
                    "SELECT f.id, f.name, c.value_ids FROM \"{schema}\".feature f
                     JOIN \"{schema}\".token_column c ON c.feature_id = f.id ORDER BY f.id"
                ))
                .map_err(storage)?;
            let mut values_stmt = conn
                .prepare(&format!(
                    "SELECT value_id, value FROM \"{schema}\".feature_value WHERE feature_id = ?1 ORDER BY value_id"
                ))
                .map_err(storage)?;
            let rows = stmt
                .query_map([], |r| {
                    Ok((
                        r.get::<_, i64>(0)?,
                        r.get::<_, String>(1)?,
                        r.get::<_, Vec<u8>>(2)?,
                    ))
                })
                .map_err(storage)?;
            for row in rows {
                let (id, name, blob) = row.map_err(storage)?;
                let mut values = vec![String::new()];
                let mut lookup = HashMap::new();
                let vrows = values_stmt
                    .query_map([id], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)))
                    .map_err(storage)?;
                for v in vrows {
                    let (vid, text) = v.map_err(storage)?;
                    debug_assert_eq!(vid as usize, values.len());
                    lookup.insert(text.clone(), vid as u32);
                    values.push(text);
                }
                features.push((
                    name,
                    FeatureColumn {
                        id,
                        values,
                        ids: u32s(&blob),
                        lookup,
                    },
                ));
            }
        }
        let mut containers = HashMap::new();
        {
            let mut all: Vec<(u32, u32, u32, u32)> = Vec::new();
            let mut stmt = conn
                .prepare(&format!(
                    "SELECT id, kind_id, coalesce(parent_id, 0), first_pos, last_pos FROM \"{schema}\".container ORDER BY id"
                ))
                .map_err(storage)?;
            let rows = stmt
                .query_map([], |r| {
                    Ok((
                        r.get::<_, i64>(0)? as u32,
                        r.get::<_, i64>(1)? as u32,
                        r.get::<_, i64>(2)? as u32,
                        r.get::<_, i64>(3)? as u32,
                        r.get::<_, i64>(4)? as u32,
                    ))
                })
                .map_err(storage)?;
            let mut nodes = Vec::new();
            for row in rows {
                let (id, kind, parent, first, last) = row.map_err(storage)?;
                debug_assert_eq!(id as usize, nodes.len() + 1);
                nodes.push((parent, first, last));
                all.push((id, kind, first, last));
            }
            let kind_of =
                std::rc::Rc::new(all.iter().map(|(_, kind, _, _)| *kind).collect::<Vec<_>>());
            let nodes = std::rc::Rc::new(nodes);
            let mut kinds = conn
                .prepare(&format!(
                    "SELECT k.id, k.name, m.container_ids FROM \"{schema}\".container_kind k
                     JOIN \"{schema}\".token_container m ON m.kind_id = k.id"
                ))
                .map_err(storage)?;
            let rows = kinds
                .query_map([], |r| {
                    Ok((
                        r.get::<_, i64>(0)?,
                        r.get::<_, String>(1)?,
                        r.get::<_, Vec<u8>>(2)?,
                    ))
                })
                .map_err(storage)?;
            for row in rows {
                let (kind_id, name, blob) = row.map_err(storage)?;
                containers.insert(
                    name,
                    ContainerLayer {
                        kind_id: kind_id as u32,
                        nodes: nodes.as_ref().clone(),
                        innermost: u32s(&blob),
                        kind_of: kind_of.clone(),
                    },
                );
            }
        }
        Ok(TokenColumns {
            verse,
            features,
            containers,
            postings: RefCell::new(HashMap::new()),
        })
    }

    /// Number of tokens.
    pub fn len(&self) -> usize {
        self.verse.len()
    }

    /// True when the edition has no token layer.
    pub fn is_empty(&self) -> bool {
        self.verse.is_empty()
    }

    /// The feature column by name.
    pub fn feature(&self, name: &str) -> Option<&FeatureColumn> {
        self.features
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, c)| c)
    }

    /// The posting bitmap of `feature = value`, read from the pack once and cached.
    pub fn posting(
        &self,
        conn: &Connection,
        schema: &str,
        feature: &str,
        value: &str,
    ) -> Result<std::rc::Rc<RoaringBitmap>> {
        let column = self
            .feature(feature)
            .ok_or_else(|| RetrievalError::LayerUnavailable(format!("feature `{feature}`")))?;
        let Some(value_id) = column.lookup.get(&crate::tokenizer::feature_key(value)) else {
            return Ok(std::rc::Rc::new(RoaringBitmap::new()));
        };
        let key = (column.id, *value_id);
        if let Some(b) = self.postings.borrow().get(&key) {
            return Ok(b.clone());
        }
        let blob: Vec<u8> = conn
            .query_row(
                &format!("SELECT bitmap FROM \"{schema}\".posting WHERE feature_id = ?1 AND value_id = ?2"),
                params![column.id, i64::from(*value_id)],
                |r| r.get(0),
            )
            .map_err(storage)?;
        let bitmap = RoaringBitmap::deserialize_from(&blob[..])
            .map_err(|e| RetrievalError::Storage(format!("posting bitmap: {e}")))?;
        let bitmap = std::rc::Rc::new(bitmap);
        self.postings.borrow_mut().insert(key, bitmap.clone());
        Ok(bitmap)
    }

    /// The scope key of a position for a unit; `None` when the token is outside every
    /// container of that kind.
    fn unit_key(&self, unit: &Unit, pos: TokenPos) -> Result<Option<u32>> {
        Ok(match unit {
            Unit::Verse => Some(self.verse[pos as usize]),
            Unit::Chapter => Some(self.verse[pos as usize] / 1000),
            Unit::Book => Some(self.verse[pos as usize] / 1_000_000),
            Unit::Container(kind) => {
                let layer = self.containers.get(kind).ok_or_else(|| {
                    RetrievalError::LayerUnavailable(format!("container `{kind}`"))
                })?;
                match layer.innermost[pos as usize] {
                    0 => None,
                    id => Some(id),
                }
            }
        })
    }

    fn layer(&self, unit: &Unit) -> Result<&ContainerLayer> {
        match unit {
            Unit::Container(kind) => self
                .containers
                .get(kind)
                .ok_or_else(|| RetrievalError::LayerUnavailable(format!("container `{kind}`"))),
            other => Err(RetrievalError::InvalidQuery(format!(
                "structural constraints need a container kind, not {other:?}"
            ))),
        }
    }

    /// True when some container of the layer's kind holds both positions: walk up from the
    /// innermost container of `a` through parents of the same kind and test the span.
    fn share_ancestor(&self, unit: &Unit, a: TokenPos, b: TokenPos) -> Result<bool> {
        let layer = self.layer(unit)?;
        let mut id = layer.innermost[a as usize];
        while id != 0 {
            let (parent, first, last) = layer.nodes[id as usize - 1];
            if layer.kind_of[id as usize - 1] == layer.kind_id && first <= b && b <= last {
                return Ok(true);
            }
            id = parent;
        }
        Ok(false)
    }

    fn satisfied(&self, constraint: &Constraint, bound: &[TokenPos]) -> Result<Option<bool>> {
        let get = |i: usize| bound.get(i).copied();
        Ok(match constraint {
            Constraint::Within { a, b, distance } => match (get(*a), get(*b)) {
                (Some(x), Some(y)) => Some(x.abs_diff(y) <= *distance),
                _ => None,
            },
            Constraint::Before { a, b, distance } => match (get(*a), get(*b)) {
                (Some(x), Some(y)) => Some(x < y && distance.is_none_or(|d| y - x <= d)),
                _ => None,
            },
            Constraint::Agree { a, b, features } => match (get(*a), get(*b)) {
                (Some(x), Some(y)) => {
                    let mut ok = true;
                    for f in features {
                        let col = self.feature(f).ok_or_else(|| {
                            RetrievalError::LayerUnavailable(format!("feature `{f}`"))
                        })?;
                        let (vx, vy) = (col.ids[x as usize], col.ids[y as usize]);
                        if vx == 0 || vx != vy {
                            ok = false;
                            break;
                        }
                    }
                    Some(ok)
                }
                _ => None,
            },
            Constraint::Peer { a, b, unit } => match (get(*a), get(*b)) {
                (Some(x), Some(y)) => {
                    let layer = self.layer(unit)?;
                    let (ix, iy) = (layer.innermost[x as usize], layer.innermost[y as usize]);
                    Some(ix != 0 && ix == iy)
                }
                _ => None,
            },
            Constraint::XPeer { a, b, unit } => match (get(*a), get(*b)) {
                (Some(x), Some(y)) => {
                    let layer = self.layer(unit)?;
                    let (ix, iy) = (layer.innermost[x as usize], layer.innermost[y as usize]);
                    Some(ix != 0 && iy != 0 && ix != iy)
                }
                _ => None,
            },
            Constraint::Ancestor { a, b, unit } => match (get(*a), get(*b)) {
                (Some(x), Some(y)) => {
                    Some(self.share_ancestor(unit, x, y)? || self.share_ancestor(unit, y, x)?)
                }
                _ => None,
            },
        })
    }

    /// Evaluates a query against this edition (attached as `schema`).
    pub fn search(&self, conn: &Connection, schema: &str, query: &TokenQuery) -> Result<TokenHits> {
        if query.slots.is_empty() {
            return Err(RetrievalError::InvalidQuery(
                "a token query needs at least one slot".into(),
            ));
        }
        for c in &query.constraints {
            let (a, b) = match c {
                Constraint::Within { a, b, .. }
                | Constraint::Before { a, b, .. }
                | Constraint::Agree { a, b, .. }
                | Constraint::Peer { a, b, .. }
                | Constraint::XPeer { a, b, .. }
                | Constraint::Ancestor { a, b, .. } => (*a, *b),
            };
            if a >= query.slots.len() || b >= query.slots.len() || a == b {
                return Err(RetrievalError::InvalidQuery(format!(
                    "constraint refers to slots {a} and {b}"
                )));
            }
        }
        // Validate the scope layer up front so "layer unavailable" is reported even when no
        // slot matches.
        if let Unit::Container(kind) = &query.scope {
            self.layer(&Unit::Container(kind.clone()))?;
        }

        let n = self.len() as u32;
        let mut candidates: Vec<RoaringBitmap> = Vec::with_capacity(query.slots.len());
        for slot in &query.slots {
            let mut acc: Option<RoaringBitmap> = None;
            for p in &slot.predicates {
                let bitmap = self.posting(conn, schema, &p.feature, &p.value)?;
                acc = Some(match acc {
                    None => (*bitmap).clone(),
                    Some(a) => a & bitmap.as_ref(),
                });
            }
            let mut set = acc.unwrap_or_else(|| {
                let mut all = RoaringBitmap::new();
                all.insert_range(0..n);
                all
            });
            if let Some(book) = query.book {
                let lo = self
                    .verse
                    .partition_point(|v| *v < u32::from(book) * 1_000_000)
                    as u32;
                let hi = self
                    .verse
                    .partition_point(|v| *v < (u32::from(book) + 1) * 1_000_000)
                    as u32;
                let mut range = RoaringBitmap::new();
                range.insert_range(lo..hi);
                set &= range;
            }
            candidates.push(set);
        }

        if query.slots.len() == 1 {
            let set = &candidates[0];
            return Ok(TokenHits {
                count: set.len(),
                matches: set
                    .iter()
                    .take(query.limit)
                    .map(|p| TokenMatch { positions: vec![p] })
                    .collect(),
            });
        }

        // Group every slot but the first by scope key.
        let mut grouped: Vec<HashMap<u32, Vec<TokenPos>>> = Vec::with_capacity(query.slots.len());
        grouped.push(HashMap::new());
        for set in candidates.iter().skip(1) {
            let mut map: HashMap<u32, Vec<TokenPos>> = HashMap::new();
            for pos in set {
                if let Some(key) = self.unit_key(&query.scope, pos)? {
                    map.entry(key).or_default().push(pos);
                }
            }
            grouped.push(map);
        }

        let mut count = 0u64;
        let mut matches = Vec::new();
        let mut bound: Vec<TokenPos> = Vec::with_capacity(query.slots.len());
        let empty: Vec<TokenPos> = Vec::new();
        for first in &candidates[0] {
            let Some(key) = self.unit_key(&query.scope, first)? else {
                continue;
            };
            bound.clear();
            bound.push(first);
            if !self.check(&query.constraints, &bound)? {
                continue;
            }
            self.extend(
                query,
                &grouped,
                key,
                &empty,
                &mut bound,
                &mut count,
                &mut matches,
            )?;
        }
        Ok(TokenHits { count, matches })
    }

    fn check(&self, constraints: &[Constraint], bound: &[TokenPos]) -> Result<bool> {
        for c in constraints {
            if self.satisfied(c, bound)? == Some(false) {
                return Ok(false);
            }
        }
        Ok(true)
    }

    #[allow(clippy::too_many_arguments)]
    fn extend(
        &self,
        query: &TokenQuery,
        grouped: &[HashMap<u32, Vec<TokenPos>>],
        key: u32,
        empty: &Vec<TokenPos>,
        bound: &mut Vec<TokenPos>,
        count: &mut u64,
        matches: &mut Vec<TokenMatch>,
    ) -> Result<()> {
        let slot = bound.len();
        if slot == query.slots.len() {
            *count += 1;
            if matches.len() < query.limit {
                matches.push(TokenMatch {
                    positions: bound.clone(),
                });
            }
            return Ok(());
        }
        let options = grouped[slot].get(&key).unwrap_or(empty);
        for pos in options {
            if bound.contains(pos) {
                continue;
            }
            bound.push(*pos);
            if self.check(&query.constraints, bound)? {
                self.extend(query, grouped, key, empty, bound, count, matches)?;
            }
            bound.pop();
        }
        Ok(())
    }

    /// Groups positions by a feature value (or `book`) and counts.
    pub fn facets(&self, positions: &[TokenPos], feature: &str) -> Result<Vec<Facet>> {
        let mut counts: HashMap<String, u64> = HashMap::new();
        if feature == "book" {
            for p in positions {
                *counts
                    .entry(VerseId(self.verse[*p as usize]).book().to_string())
                    .or_default() += 1;
            }
        } else {
            let col = self
                .feature(feature)
                .ok_or_else(|| RetrievalError::LayerUnavailable(format!("feature `{feature}`")))?;
            for p in positions {
                let id = col.ids[*p as usize];
                *counts.entry(col.values[id as usize].clone()).or_default() += 1;
            }
        }
        let mut facets: Vec<Facet> = counts
            .into_iter()
            .map(|(value, count)| Facet { value, count })
            .collect();
        facets.sort_by(|a, b| b.count.cmp(&a.count).then_with(|| a.value.cmp(&b.value)));
        Ok(facets)
    }
}
