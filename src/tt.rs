use crate::moves::Move;

/// What the stored score means. Alpha-beta often stops after proving only a limit.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Bound {
    Exact, // the score is the true value
    Lower, // the true value is at least this (the search failed high)
    Upper, // the true value is at most this (the search failed low)
}

#[derive(Copy, Clone)]
pub struct Entry {
    pub key: u64,   // the full hash: tells us whether this slot's note is about OUR position
    pub score: i32, // relative to the node, with mate scores adjusted (see search.rs)
    pub mv: Move,   // best move found, or Move::NULL if none
    pub depth: u8,  // how many plies were searched below this position
    pub bound: Bound,
}

const EMPTY: Entry = Entry { key: 0, score: 0, mv: Move::NULL, depth: 0, bound: Bound::Upper };

pub struct Tt {
    entries: Vec<Entry>,
    mask: usize, // entries.len() is a power of two, so `hash & mask` picks a slot
}

impl Tt {
    /// A table of roughly `mb` mebibytes (rounded down to a power of two entries).
    pub fn new(mb: usize) -> Tt {
        let wanted = (mb.max(1) * 1024 * 1024) / std::mem::size_of::<Entry>();
        let n = 1usize << (usize::BITS - 1 - wanted.max(1).leading_zeros());
        Tt { entries: vec![EMPTY; n], mask: n - 1 }
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn clear(&mut self) {
        self.entries.fill(EMPTY);
    }

    /// Permille of slots in use, sampled from the first 1000 slots (what UCI `hashfull` reports).
    pub fn hashfull(&self) -> usize {
        let n = self.entries.len().min(1000);
        self.entries[..n].iter().filter(|e| e.key != 0).count() * 1000 / n
    }

    /// The entry in this position's slot, but only if it really is for this position.
    #[inline]
    pub fn probe(&self, key: u64) -> Option<Entry> {
        let e = self.entries[key as usize & self.mask];
        if e.key == key { Some(e) } else { None }
    }

    /// Write a note into this position's slot, replacing whatever was there.
    /// Losing the old note only costs time later, never correctness.
    #[inline]
    pub fn store(&mut self, key: u64, mv: Move, score: i32, depth: i32, bound: Bound) {
        let slot = &mut self.entries[key as usize & self.mask];
        // if this search found no best move, keep the one we already had for this position
        let mv = if mv == Move::NULL && slot.key == key { slot.mv } else { mv };
        *slot = Entry { key, score, mv, depth: depth.clamp(0, 255) as u8, bound };
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::Square;

    #[test]
    fn size_is_a_power_of_two() {
        assert_eq!(std::mem::size_of::<Entry>(), 16);
        let t = Tt::new(1);
        assert_eq!(t.len(), 65_536); // 1 MiB / 16 bytes
        assert!(Tt::new(3).len().is_power_of_two());
    }

    #[test]
    fn store_and_probe() {
        let mut t = Tt::new(1);
        let m = Move::new(Square::new(4, 1), Square::new(4, 3));
        assert!(t.probe(0xABCD).is_none());
        t.store(0xABCD, m, 30, 5, Bound::Exact);
        let e = t.probe(0xABCD).unwrap();
        assert_eq!((e.mv, e.score, e.depth, e.bound), (m, 30, 5, Bound::Exact));
    }

    #[test]
    fn same_slot_different_key_is_a_miss_and_gets_replaced() {
        let mut t = Tt::new(1);
        let a = 0x5B;
        let b = a + t.len() as u64; // same low bits (same slot), different full key
        t.store(a, Move::NULL, 10, 3, Bound::Exact);
        assert!(t.probe(b).is_none()); // slot shared, but the key check says "not mine"
        t.store(b, Move::NULL, 20, 3, Bound::Exact);
        assert!(t.probe(a).is_none()); // a's note was overwritten: only costs a re-search
        assert_eq!(t.probe(b).unwrap().score, 20);
    }

    #[test]
    fn hashfull_counts_used_slots() {
        let mut t = Tt::new(1);
        assert_eq!(t.hashfull(), 0);
        for key in 1..=1000u64 {
            t.store(key, Move::NULL, 0, 1, Bound::Exact);
        }
        assert!(t.hashfull() >= 990, "{}", t.hashfull()); // slots 1..=999 of the sampled 0..999
        t.clear();
        assert_eq!(t.hashfull(), 0);
    }

    #[test]
    fn keeps_old_move_when_new_store_has_none() {
        let mut t = Tt::new(1);
        let m = Move::new(Square::new(6, 0), Square::new(5, 2));
        t.store(7, m, 10, 4, Bound::Lower);
        t.store(7, Move::NULL, -5, 6, Bound::Upper);
        let e = t.probe(7).unwrap();
        assert_eq!(e.mv, m);
        assert_eq!(e.score, -5);
    }
}