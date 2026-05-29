use anyhow::Result;
use fxhash::FxHashMap;
use rayon::prelude::*;

// ────────────────────────────────────────────────────────────────────────────
// Data model — rows are represented as vectors of raw byte-string columns.
// The engine feeds pre-split rows; the join works on column indices.
// ────────────────────────────────────────────────────────────────────────────

/// A single row from either the left or right table.
pub type Row = Vec<String>;

/// A materialized dataset ready for joining.
pub struct JoinTable {
    /// All rows in this table.
    pub rows: Vec<Row>,
    /// Index of the join key column within each row.
    pub key_col: usize,
}

impl JoinTable {
    pub fn new(rows: Vec<Row>, key_col: usize) -> Self {
        Self { rows, key_col }
    }

    #[inline(always)]
    fn key_of<'a>(&self, row: &'a Row) -> Option<&'a str> {
        row.get(self.key_col).map(|s| s.as_str())
    }
}

// ────────────────────────────────────────────────────────────────────────────
// Join result
// ────────────────────────────────────────────────────────────────────────────

/// A matched pair of rows (left_row_idx, right_row_idx).
pub type JoinMatch = (usize, usize);

/// A fully materialized joined row (left columns ++ right columns).
pub type JoinedRow = Vec<String>;

// ────────────────────────────────────────────────────────────────────────────
// Join type
// ────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JoinType {
    Inner,
    LeftOuter,
    RightOuter,
}

// ────────────────────────────────────────────────────────────────────────────
// HashJoin Executor
// ────────────────────────────────────────────────────────────────────────────

/// Partitioned Hash Join executor.
///
/// Algorithm (Grace Hash Join variant):
///   Phase 1 — Partition: Distribute both tables into N buckets by
///             hash(key) mod N. Rows with the same key always end up in the
///             same bucket, so joins can be computed independently per bucket.
///
///   Phase 2 — Build: For each partition, build a hash-map from the smaller
///             (left) table's key → list of row indices.
///
///   Phase 3 — Probe: Stream the right table through the hash-map and emit
///             (left_idx, right_idx) match pairs.
///
/// Rayon is used to process all N partitions in parallel across CPU cores.
pub struct HashJoin {
    /// Name of the join key column in the left table.
    pub left_column: String,
    /// Name of the join key column in the right table.
    pub right_column: String,
    /// Number of partitions. More partitions = smaller per-partition RAM,
    /// better for very large tables. Typical values: 64–512.
    pub partitions: usize,
    /// Inner, Left Outer, or Right Outer.
    pub join_type: JoinType,
}

impl HashJoin {
    pub fn new(left_col: &str, right_col: &str, partitions: usize) -> Self {
        Self {
            left_column: left_col.to_string(),
            right_column: right_col.to_string(),
            partitions,
            join_type: JoinType::Inner,
        }
    }

    pub fn with_join_type(mut self, join_type: JoinType) -> Self {
        self.join_type = join_type;
        self
    }

    // ── Phase 1: Partition ────────────────────────────────────────────────

    /// Distribute `rows` into `n` buckets by hash(key[key_col]) mod n.
    /// Returns a Vec of Vec<row_index>, one entry per partition.
    fn partition_by_key(
        rows: &[Row],
        key_col: usize,
        n: usize,
    ) -> Vec<Vec<usize>> {
        let mut buckets: Vec<Vec<usize>> = (0..n).map(|_| Vec::new()).collect();
        for (idx, row) in rows.iter().enumerate() {
            let key = row.get(key_col).map(|s| s.as_str()).unwrap_or("");
            let bucket = fxhash::hash(key.as_bytes()) % n;
            buckets[bucket].push(idx);
        }
        buckets
    }

    // ── Phase 2+3: Build + Probe (per partition) ──────────────────────────

    /// Process a single partition: builds a hash-map for left rows, then
    /// probes with right rows to emit match pairs.
    fn process_partition(
        left: &JoinTable,
        right: &JoinTable,
        left_indices: &[usize],
        right_indices: &[usize],
        join_type: JoinType,
    ) -> Vec<JoinMatch> {
        // Build phase: group left row indices by key.
        let mut build_map: FxHashMap<&str, Vec<usize>> =
            FxHashMap::with_capacity_and_hasher(left_indices.len(), Default::default());

        for &l_idx in left_indices {
            if let Some(key) = left.key_of(&left.rows[l_idx]) {
                build_map.entry(key).or_default().push(l_idx);
            }
        }

        // Probe phase: look up each right-row key in the build map.
        let mut matches: Vec<JoinMatch> = Vec::new();
        let mut unmatched_right: Vec<usize> = Vec::new();

        for &r_idx in right_indices {
            let key = right.key_of(&right.rows[r_idx]).unwrap_or("");
            if let Some(left_idxs) = build_map.get(key) {
                for &l_idx in left_idxs {
                    matches.push((l_idx, r_idx));
                }
            } else if join_type == JoinType::RightOuter {
                unmatched_right.push(r_idx);
            }
        }

        // For Left Outer Join: emit rows from left that had no match.
        if join_type == JoinType::LeftOuter {
            let matched_left: fxhash::FxHashSet<usize> =
                matches.iter().map(|(l, _)| *l).collect();
            for &l_idx in left_indices {
                if !matched_left.contains(&l_idx) {
                    // usize::MAX signals "no matching right row"
                    matches.push((l_idx, usize::MAX));
                }
            }
        }

        // For Right Outer Join: emit unmatched right rows.
        for r_idx in unmatched_right {
            matches.push((usize::MAX, r_idx));
        }

        matches
    }

    // ── Public API ────────────────────────────────────────────────────────

    /// Execute the full Partitioned Hash Join and return all matched index pairs.
    pub fn execute(&self, left: &JoinTable, right: &JoinTable) -> Result<Vec<JoinMatch>> {
        let n = self.partitions;
        log::info!(
            "[HashJoin] Starting {} join on '{}' ⋈ '{}' | {} partitions | left={} rows, right={} rows",
            format!("{:?}", self.join_type),
            self.left_column, self.right_column,
            n, left.rows.len(), right.rows.len()
        );

        // Phase 1: Partition both tables in parallel.
        let (left_buckets, right_buckets) = rayon::join(
            || Self::partition_by_key(&left.rows, left.key_col, n),
            || Self::partition_by_key(&right.rows, right.key_col, n),
        );

        // Phase 2+3: Process each partition in parallel with Rayon.
        let join_type = self.join_type;
        let all_matches: Vec<JoinMatch> = left_buckets
            .par_iter()
            .zip(right_buckets.par_iter())
            .flat_map(|(l_idxs, r_idxs)| {
                Self::process_partition(left, right, l_idxs, r_idxs, join_type)
            })
            .collect();

        log::info!("[HashJoin] Complete — {} matched pairs", all_matches.len());
        Ok(all_matches)
    }

    /// Execute the join and materialize complete joined rows.
    /// Columns from the right table are appended after the left table's columns.
    pub fn execute_materialized(
        &self,
        left: &JoinTable,
        right: &JoinTable,
    ) -> Result<Vec<JoinedRow>> {
        let matches = self.execute(left, right)?;

        let empty_left = vec!["".to_string(); left.rows.first().map(|r| r.len()).unwrap_or(0)];
        let empty_right = vec!["".to_string(); right.rows.first().map(|r| r.len()).unwrap_or(0)];

        let joined: Vec<JoinedRow> = matches
            .into_par_iter()
            .map(|(l_idx, r_idx)| {
                let left_row = if l_idx == usize::MAX { &empty_left } else { &left.rows[l_idx] };
                let right_row = if r_idx == usize::MAX { &empty_right } else { &right.rows[r_idx] };
                let mut row = left_row.clone();
                row.extend_from_slice(right_row);
                row
            })
            .collect();

        Ok(joined)
    }
}

// ────────────────────────────────────────────────────────────────────────────
// Convenience: Simple in-memory join (kept for backward compat & unit tests)
// ────────────────────────────────────────────────────────────────────────────

/// Perform a simple inner hash join on two key slices.
/// Useful for small datasets and unit tests.
pub fn join_memory_simple(
    left_keys: &[String],
    right_keys: &[String],
) -> Vec<(usize, usize)> {
    let mut hash_map: FxHashMap<&str, Vec<usize>> =
        FxHashMap::with_capacity_and_hasher(left_keys.len(), Default::default());

    for (idx, key) in left_keys.iter().enumerate() {
        hash_map.entry(key.as_str()).or_default().push(idx);
    }

    let mut matches = Vec::new();
    for (idx, key) in right_keys.iter().enumerate() {
        if let Some(left_indices) = hash_map.get(key.as_str()) {
            for &l_idx in left_indices {
                matches.push((l_idx, idx));
            }
        }
    }
    matches
}

// ────────────────────────────────────────────────────────────────────────────
// Unit Tests
// ────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn make_table(data: &[(&str, &str)], key_col: usize) -> JoinTable {
        let rows = data
            .iter()
            .map(|(a, b)| vec![a.to_string(), b.to_string()])
            .collect();
        JoinTable::new(rows, key_col)
    }

    #[test]
    fn test_inner_join_basic() {
        // Left:  id | name
        // Right: id | score
        let left = make_table(&[("1", "Alice"), ("2", "Bob"), ("3", "Carol")], 0);
        let right = make_table(&[("2", "92"), ("3", "87"), ("4", "55")], 0);

        let join = HashJoin::new("id", "id", 8);
        let matches = join.execute(&left, &right).unwrap();

        // Should match id=2 and id=3
        assert_eq!(matches.len(), 2);
        let left_idxs: Vec<_> = matches.iter().map(|(l, _)| *l).collect();
        assert!(left_idxs.contains(&1)); // Bob
        assert!(left_idxs.contains(&2)); // Carol
    }

    #[test]
    fn test_left_outer_join() {
        let left = make_table(&[("1", "Alice"), ("2", "Bob")], 0);
        let right = make_table(&[("2", "92")], 0);

        let join = HashJoin::new("id", "id", 4).with_join_type(JoinType::LeftOuter);
        let matches = join.execute(&left, &right).unwrap();

        // Should have 2 rows: Bob matched, Alice unmatched (MAX right)
        assert_eq!(matches.len(), 2);
        let has_unmatched = matches.iter().any(|(_, r)| *r == usize::MAX);
        assert!(has_unmatched, "Left outer join must include unmatched left rows");
    }

    #[test]
    fn test_materialized_join() {
        let left = make_table(&[("1", "Alice"), ("2", "Bob")], 0);
        let right = make_table(&[("1", "88"), ("2", "92")], 0);

        let join = HashJoin::new("id", "id", 4);
        let rows = join.execute_materialized(&left, &right).unwrap();

        assert_eq!(rows.len(), 2);
        // Each resulting row should have 4 columns (2 from left + 2 from right)
        assert_eq!(rows[0].len(), 4);
    }

    #[test]
    fn test_simple_join_backward_compat() {
        let left_keys = vec!["a".to_string(), "b".to_string(), "c".to_string()];
        let right_keys = vec!["b".to_string(), "c".to_string(), "d".to_string()];
        let matches = join_memory_simple(&left_keys, &right_keys);
        assert_eq!(matches.len(), 2);
    }
}
