pub const WORK_RAM_SIZE: usize = 0x2000;

#[derive(Clone)]
pub struct RamSnapshot {
    data: Vec<u8>,
}

impl RamSnapshot {
    pub fn capture(ram: &[u8]) -> Self {
        Self {
            data: ram.to_vec(),
        }
    }

    pub fn get(&self, addr: u32) -> u8 {
        self.data.get(addr as usize).copied().unwrap_or(0)
    }

    pub fn len(&self) -> usize {
        self.data.len()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SearchFilter {
    Equal(u8),
    NotEqual(u8),
    GreaterThan(u8),
    LessThan(u8),
    Increased,
    Decreased,
    Changed,
    Unchanged,
    IncreasedBy(u8),
    DecreasedBy(u8),
    /// Search for a decimal value stored as little-endian BCD (1 digit per byte).
    /// E.g. BcdEqual(13) matches consecutive bytes [0x03, 0x01].
    /// Returns the address of the lowest digit (ones place).
    BcdEqual(u16),
}

impl SearchFilter {
    pub fn needs_snapshot(&self) -> bool {
        matches!(
            self,
            Self::Increased
                | Self::Decreased
                | Self::Changed
                | Self::Unchanged
                | Self::IncreasedBy(_)
                | Self::DecreasedBy(_)
        )
    }

    /// Convert a decimal value to BCD digit array (ones first).
    /// E.g. 130 → [0, 3, 1], 13 → [3, 1], 0 → [0].
    pub fn bcd_digits(mut value: u16) -> Vec<u8> {
        if value == 0 {
            return vec![0];
        }
        let mut digits = Vec::new();
        while value > 0 {
            digits.push((value % 10) as u8);
            value /= 10;
        }
        digits
    }
}

pub struct CheatSearch {
    snapshot: Option<RamSnapshot>,
    candidates: Vec<u32>,
    ram_size: usize,
}

impl CheatSearch {
    pub fn new() -> Self {
        Self {
            snapshot: None,
            candidates: (0..WORK_RAM_SIZE as u32).collect(),
            ram_size: WORK_RAM_SIZE,
        }
    }

    /// Re-initialize candidates for the given RAM size.
    pub fn resize(&mut self, size: usize) {
        if size != self.ram_size {
            self.ram_size = size;
            self.snapshot = None;
            self.candidates = (0..size as u32).collect();
        }
    }

    pub fn snapshot(&mut self, ram: &[u8]) {
        if ram.len() != self.ram_size {
            self.resize(ram.len());
        }
        self.snapshot = Some(RamSnapshot::capture(ram));
    }

    pub fn has_snapshot(&self) -> bool {
        self.snapshot.is_some()
    }

    pub fn previous_snapshot(&self) -> Option<&RamSnapshot> {
        self.snapshot.as_ref()
    }

    pub fn apply_filter(&mut self, filter: SearchFilter, current_ram: &[u8]) {
        // BCD search: replace candidates with matching start addresses
        if let SearchFilter::BcdEqual(value) = filter {
            let digits = SearchFilter::bcd_digits(value);
            let num_digits = digits.len();
            let mut matches = Vec::new();
            let ram_len = current_ram.len();
            // Check every address in current candidates
            let candidates = std::mem::take(&mut self.candidates);
            for &addr in &candidates {
                let a = addr as usize;
                if a + num_digits > ram_len {
                    continue;
                }
                let ok = digits.iter().enumerate().all(|(i, &d)| {
                    current_ram[a + i] == d
                });
                if ok {
                    matches.push(addr);
                }
            }
            self.candidates = matches;
            self.snapshot = Some(RamSnapshot::capture(current_ram));
            return;
        }

        let snap = match &self.snapshot {
            Some(s) if filter.needs_snapshot() => s,
            _ if filter.needs_snapshot() => return,
            _ => {
                self.candidates.retain(|&addr| {
                    let cur = current_ram.get(addr as usize).copied().unwrap_or(0);
                    match filter {
                        SearchFilter::Equal(v) => cur == v,
                        SearchFilter::NotEqual(v) => cur != v,
                        SearchFilter::GreaterThan(v) => cur > v,
                        SearchFilter::LessThan(v) => cur < v,
                        _ => unreachable!(),
                    }
                });
                self.snapshot = Some(RamSnapshot::capture(current_ram));
                return;
            }
        };

        let snap_clone = snap.clone();
        self.candidates.retain(|&addr| {
            let cur = current_ram.get(addr as usize).copied().unwrap_or(0);
            let prev = snap_clone.get(addr);
            match filter {
                SearchFilter::Increased => cur > prev,
                SearchFilter::Decreased => cur < prev,
                SearchFilter::Changed => cur != prev,
                SearchFilter::Unchanged => cur == prev,
                SearchFilter::IncreasedBy(d) => cur == prev.wrapping_add(d),
                SearchFilter::DecreasedBy(d) => cur == prev.wrapping_sub(d),
                _ => unreachable!(),
            }
        });
        self.snapshot = Some(RamSnapshot::capture(current_ram));
    }

    pub fn reset(&mut self) {
        self.snapshot = None;
        self.candidates = (0..self.ram_size as u32).collect();
    }

    pub fn candidates(&self) -> &[u32] {
        &self.candidates
    }

    pub fn candidate_count(&self) -> usize {
        self.candidates.len()
    }

    pub fn ram_size(&self) -> usize {
        self.ram_size
    }
}

#[derive(Clone)]
pub struct CheatEntry {
    pub address: u32,
    pub value: u8,
    pub enabled: bool,
    pub label: String,
}

pub struct CheatManager {
    pub entries: Vec<CheatEntry>,
}

impl CheatManager {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    pub fn add(&mut self, address: u32, value: u8, label: String) {
        self.entries.push(CheatEntry {
            address,
            value,
            enabled: true,
            label,
        });
    }

    pub fn remove(&mut self, index: usize) {
        if index < self.entries.len() {
            self.entries.remove(index);
        }
    }

    /// Apply cheats to a combined RAM buffer (work_ram ++ cart_ram).
    pub fn apply(&self, ram: &mut [u8]) {
        for entry in &self.entries {
            if entry.enabled && (entry.address as usize) < ram.len() {
                ram[entry.address as usize] = entry.value;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_snapshot_and_filter_equal() {
        let mut ram = vec![0u8; WORK_RAM_SIZE];
        ram[0x10] = 42;
        ram[0x20] = 42;
        ram[0x30] = 99;

        let mut search = CheatSearch::new();
        assert_eq!(search.candidate_count(), WORK_RAM_SIZE);

        search.apply_filter(SearchFilter::Equal(42), &ram);
        assert_eq!(search.candidate_count(), 2);
        assert!(search.candidates().contains(&0x10));
        assert!(search.candidates().contains(&0x20));
    }

    #[test]
    fn test_snapshot_and_filter_increased() {
        let mut ram = vec![0u8; WORK_RAM_SIZE];
        ram[0x10] = 5;
        ram[0x20] = 10;
        ram[0x30] = 3;

        let mut search = CheatSearch::new();
        search.snapshot(&ram);

        ram[0x10] = 8;  // increased
        ram[0x20] = 10; // unchanged
        ram[0x30] = 1;  // decreased

        search.apply_filter(SearchFilter::Increased, &ram);
        assert_eq!(search.candidate_count(), 1);
        assert_eq!(search.candidates()[0], 0x10);
    }

    #[test]
    fn test_snapshot_and_filter_decreased() {
        let mut ram = vec![0u8; WORK_RAM_SIZE];
        ram[0x10] = 10;
        ram[0x20] = 5;

        let mut search = CheatSearch::new();
        search.snapshot(&ram);

        ram[0x10] = 7;
        ram[0x20] = 5;

        search.apply_filter(SearchFilter::Decreased, &ram);
        assert_eq!(search.candidate_count(), 1);
        assert_eq!(search.candidates()[0], 0x10);
    }

    #[test]
    fn test_filter_unchanged() {
        let mut ram = vec![0u8; WORK_RAM_SIZE];
        ram[0x10] = 5;
        ram[0x20] = 10;

        let mut search = CheatSearch::new();
        search.snapshot(&ram);

        ram[0x10] = 5;
        ram[0x20] = 99;

        search.apply_filter(SearchFilter::Unchanged, &ram);
        let count = search.candidate_count();
        assert!(search.candidates().contains(&0x10));
        assert!(!search.candidates().contains(&0x20));
        assert_eq!(count, WORK_RAM_SIZE - 1);
    }

    #[test]
    fn test_filter_increased_by() {
        let mut ram = vec![0u8; WORK_RAM_SIZE];
        ram[0x10] = 5;
        ram[0x20] = 10;

        let mut search = CheatSearch::new();
        search.snapshot(&ram);

        ram[0x10] = 8;
        ram[0x20] = 13;

        search.apply_filter(SearchFilter::IncreasedBy(3), &ram);
        assert_eq!(search.candidate_count(), 2);
    }

    #[test]
    fn test_reset() {
        let ram = vec![0u8; WORK_RAM_SIZE];
        let mut search = CheatSearch::new();
        search.snapshot(&ram);
        search.apply_filter(SearchFilter::Equal(99), &ram);
        assert_eq!(search.candidate_count(), 0);

        search.reset();
        assert_eq!(search.candidate_count(), WORK_RAM_SIZE);
        assert!(!search.has_snapshot());
    }

    #[test]
    fn test_search_with_extended_ram() {
        // Simulate work_ram (8KB) + cart_ram (2KB)
        let size = WORK_RAM_SIZE + 0x800;
        let mut ram = vec![0u8; size];
        ram[0x2100] = 42; // In cart_ram region

        let mut search = CheatSearch::new();
        search.resize(size);
        assert_eq!(search.candidate_count(), size);

        search.apply_filter(SearchFilter::Equal(42), &ram);
        assert_eq!(search.candidate_count(), 1);
        assert_eq!(search.candidates()[0], 0x2100);
    }

    #[test]
    fn test_cheat_manager_apply() {
        let mut ram = vec![0u8; WORK_RAM_SIZE];
        let mut mgr = CheatManager::new();
        mgr.add(0x100, 99, "Lives".into());
        mgr.add(0x200, 50, "Health".into());

        mgr.apply(&mut ram);
        assert_eq!(ram[0x100], 99);
        assert_eq!(ram[0x200], 50);
    }

    #[test]
    fn test_cheat_manager_disabled() {
        let mut ram = vec![0u8; WORK_RAM_SIZE];
        let mut mgr = CheatManager::new();
        mgr.add(0x100, 99, "Lives".into());
        mgr.entries[0].enabled = false;

        mgr.apply(&mut ram);
        assert_eq!(ram[0x100], 0);
    }

    #[test]
    fn test_cheat_manager_remove() {
        let mut mgr = CheatManager::new();
        mgr.add(0x100, 99, "Lives".into());
        mgr.add(0x200, 50, "Health".into());
        assert_eq!(mgr.entries.len(), 2);

        mgr.remove(0);
        assert_eq!(mgr.entries.len(), 1);
        assert_eq!(mgr.entries[0].address, 0x200);
    }
}
