use egui::{self, Color32, RichText};
use pce::cheat::{CheatManager, CheatSearch, SearchFilter, WORK_RAM_SIZE};

#[derive(Clone, Copy, PartialEq, Eq)]
enum FilterKind {
    Equal,
    NotEqual,
    GreaterThan,
    LessThan,
    Increased,
    Decreased,
    Changed,
    Unchanged,
    IncreasedBy,
    DecreasedBy,
    BcdEqual,
}

impl FilterKind {
    fn label(&self) -> &'static str {
        match self {
            Self::Equal => "Equal to",
            Self::NotEqual => "Not equal to",
            Self::GreaterThan => "Greater than",
            Self::LessThan => "Less than",
            Self::Increased => "Increased",
            Self::Decreased => "Decreased",
            Self::Changed => "Changed",
            Self::Unchanged => "Unchanged",
            Self::IncreasedBy => "Increased by",
            Self::DecreasedBy => "Decreased by",
            Self::BcdEqual => "BCD (decimal)",
        }
    }

    fn needs_value(&self) -> bool {
        matches!(
            self,
            Self::Equal | Self::NotEqual | Self::GreaterThan | Self::LessThan | Self::IncreasedBy | Self::DecreasedBy | Self::BcdEqual
        )
    }

    const ALL: [FilterKind; 11] = [
        Self::Equal,
        Self::BcdEqual,
        Self::NotEqual,
        Self::GreaterThan,
        Self::LessThan,
        Self::Increased,
        Self::Decreased,
        Self::Changed,
        Self::Unchanged,
        Self::IncreasedBy,
        Self::DecreasedBy,
    ];
}

/// Decode little-endian BCD starting at `addr`: read consecutive bytes that
/// are valid BCD digits (0-9) and build a decimal string.
/// E.g. bytes [3, 1] at addr → "13", bytes [0, 5, 2] → "250".
fn decode_bcd_at(ram: &[u8], addr: usize) -> String {
    let mut digits = Vec::new();
    for i in 0..5 {
        let a = addr + i;
        if a >= ram.len() {
            break;
        }
        let b = ram[a];
        if b > 9 {
            break;
        }
        digits.push(b);
    }
    if digits.is_empty() {
        return "-".to_string();
    }
    // digits[0] = ones, digits[1] = tens, etc. — reverse for display
    let s: String = digits.iter().rev().map(|d| char::from(b'0' + d)).collect();
    s
}

/// Format an address with region label: W:xxxx or C:xxxx
fn format_addr(addr: u32, wram_size: usize) -> String {
    if (addr as usize) < wram_size {
        format!("W:{:04X}", addr)
    } else {
        format!("C:{:04X}", addr as usize - wram_size)
    }
}

/// Parse a PCE cheat address. Supports:
/// - `F8xxxx` → Work RAM offset (xxxx & 0x1FFF)
/// - `$1297` / `0x1297` / `1297` → direct offset into combined buffer
fn parse_cheat_addr(input: &str, wram_size: usize, cram_size: usize) -> Option<u32> {
    let s = input.trim().trim_start_matches('$');
    let s = s.trim_start_matches("0x").trim_start_matches("0X");
    let raw = u32::from_str_radix(s, 16).ok()?;

    // PCE bank-prefixed format: top byte is bank number
    if raw > 0xFFFF {
        let bank = (raw >> 16) as u8;
        let offset = (raw & 0x1FFF) as u32; // mask to page size
        match bank {
            0xF8 => {
                // Work RAM
                if (offset as usize) < wram_size {
                    return Some(offset);
                }
            }
            0x80..=0xF7 if cram_size > 0 => {
                // Cart RAM — bank $80 base, each bank is 8KB page
                let cart_offset = ((bank as usize - 0x80) * 0x2000) + offset as usize;
                if cart_offset < cram_size {
                    return Some((wram_size + cart_offset) as u32);
                }
            }
            _ => {}
        }
        None
    } else if (raw as usize) < wram_size + cram_size {
        Some(raw)
    } else {
        None
    }
}

pub struct CheatSearchUi {
    pub search: CheatSearch,
    pub manager: CheatManager,
    filter_kind: FilterKind,
    filter_value: String,
    new_cheat_label: String,
    new_cheat_value: String,
    /// Number of BCD digits from the last BCD search (0 = not a BCD search).
    last_bcd_digits: usize,
    /// Whether cheats have been loaded from file (one-shot on first show).
    cheats_loaded: bool,
}

impl CheatSearchUi {
    pub fn new() -> Self {
        Self {
            search: CheatSearch::new(),
            manager: CheatManager::new(),
            filter_kind: FilterKind::Equal,
            filter_value: String::new(),
            new_cheat_label: String::new(),
            new_cheat_value: String::new(),
            last_bcd_digits: 0,
            cheats_loaded: false,
        }
    }

    pub fn show(&mut self, ui: &mut egui::Ui, ram: &[u8], cheat_path: Option<&std::path::Path>) {
        let wram_size = WORK_RAM_SIZE;
        let has_cram = ram.len() > wram_size;

        // Auto-load cheats from file on first call
        if !self.cheats_loaded {
            self.cheats_loaded = true;
            if let Some(path) = cheat_path {
                if path.exists() {
                    match self.manager.load_from_file(path) {
                        Ok(()) => eprintln!("Loaded {} cheats from {}", self.manager.entries.len(), path.display()),
                        Err(e) => eprintln!("Failed to load cheats: {}", e),
                    }
                }
            }
        }

        ui.horizontal(|ui| {
            ui.heading("Cheat Search");
            ui.separator();
            let size_label = if has_cram {
                format!(
                    "WRAM:{}KB + CRAM:{}KB",
                    wram_size / 1024,
                    (ram.len() - wram_size) / 1024
                )
            } else {
                format!("WRAM:{}KB", wram_size / 1024)
            };
            ui.label(RichText::new(size_label).small());
        });
        ui.separator();

        ui.horizontal(|ui| {
            if ui.button("Snapshot").clicked() {
                self.search.snapshot(ram);
            }
            if ui.button("Reset").clicked() {
                self.search.reset();
            }
            ui.label(format!("Candidates: {}", self.search.candidate_count()));
            if self.search.has_snapshot() {
                ui.label(RichText::new("(snapshot taken)").color(Color32::from_rgb(0x44, 0xCC, 0x44)));
            }
        });

        ui.separator();

        ui.horizontal(|ui| {
            ui.label("Filter:");
            egui::ComboBox::from_id_salt("filter_kind")
                .selected_text(self.filter_kind.label())
                .width(130.0)
                .show_ui(ui, |ui| {
                    for kind in FilterKind::ALL {
                        ui.selectable_value(&mut self.filter_kind, kind, kind.label());
                    }
                });

            if self.filter_kind.needs_value() {
                ui.label("Value:");
                ui.add(
                    egui::TextEdit::singleline(&mut self.filter_value)
                        .desired_width(50.0),
                );
            }

            if ui.button("Apply").clicked() {
                if let Some(filter) = self.build_filter() {
                    if let SearchFilter::BcdEqual(v) = filter {
                        self.last_bcd_digits = SearchFilter::bcd_digits(v).len();
                    } else {
                        self.last_bcd_digits = 0;
                    }
                    self.search.apply_filter(filter, ram);
                }
            }
        });

        ui.separator();

        let candidates = self.search.candidates();
        let snap = self.search.previous_snapshot();
        let max_display = 200;
        let display_count = candidates.len().min(max_display);

        ui.label(format!(
            "Results (showing {}/{})",
            display_count,
            candidates.len()
        ));

        egui::ScrollArea::vertical()
            .id_salt("cheat_results")
            .max_height(150.0)
            .show(ui, |ui| {
                ui.style_mut().override_font_id = Some(egui::FontId::monospace(12.0));
                egui::Grid::new("results_grid")
                    .striped(true)
                    .show(ui, |ui| {
                        ui.label("Addr");
                        ui.label("Prev");
                        ui.label("Cur");
                        ui.label("BCD");
                        ui.label("");
                        ui.end_row();

                        let bcd_n = self.last_bcd_digits;
                        for &addr in candidates.iter().take(max_display) {
                            let cur = ram.get(addr as usize).copied().unwrap_or(0);
                            let prev = snap.map(|s| s.get(addr)).unwrap_or(0);

                            ui.label(format_addr(addr, wram_size));
                            ui.label(format!("{:02X}", prev));
                            ui.label(format!("{:02X}", cur));
                            // Decode BCD value from consecutive bytes (up to 5 digits)
                            ui.label(decode_bcd_at(ram, addr as usize));
                            if bcd_n >= 2 {
                                // BCD search: "Add" registers all digit bytes
                                if ui.small_button(format!("Add {}d", bcd_n)).clicked() {
                                    for i in 0..bcd_n {
                                        let a = addr + i as u32;
                                        let v = ram.get(a as usize).copied().unwrap_or(0);
                                        self.manager.add(
                                            a,
                                            v,
                                            format!("{}[{}]", format_addr(addr, wram_size), i),
                                        );
                                    }
                                }
                            } else {
                                if ui.small_button("Add").clicked() {
                                    self.manager.add(
                                        addr,
                                        cur,
                                        format_addr(addr, wram_size),
                                    );
                                }
                            }
                            ui.end_row();
                        }
                    });
            });

        ui.separator();
        ui.horizontal(|ui| {
            ui.heading("Active Cheats");
            ui.separator();
            if let Some(path) = cheat_path {
                if ui.button("Save").clicked() {
                    match self.manager.save_to_file(path) {
                        Ok(()) => eprintln!("Saved {} cheats to {}", self.manager.entries.len(), path.display()),
                        Err(e) => eprintln!("Failed to save cheats: {}", e),
                    }
                }
                if path.exists() {
                    if ui.button("Load").clicked() {
                        match self.manager.load_from_file(path) {
                            Ok(()) => eprintln!("Loaded {} cheats from {}", self.manager.entries.len(), path.display()),
                            Err(e) => eprintln!("Failed to load cheats: {}", e),
                        }
                    }
                }
            }
        });

        let mut remove_idx = None;
        egui::ScrollArea::vertical()
            .id_salt("cheat_entries")
            .max_height(120.0)
            .show(ui, |ui| {
                ui.style_mut().override_font_id = Some(egui::FontId::monospace(12.0));
                for (i, entry) in self.manager.entries.iter_mut().enumerate() {
                    ui.horizontal(|ui| {
                        ui.checkbox(&mut entry.enabled, "");
                        ui.label(format_addr(entry.address, wram_size));
                        ui.label("=");
                        let mut val_str = format!("{:02X}", entry.value);
                        let resp = ui.add(
                            egui::TextEdit::singleline(&mut val_str)
                                .desired_width(25.0),
                        );
                        if resp.changed() {
                            if let Ok(v) = u8::from_str_radix(val_str.trim(), 16) {
                                entry.value = v;
                            }
                        }
                        ui.text_edit_singleline(&mut entry.label);
                        if ui.small_button("X").clicked() {
                            remove_idx = Some(i);
                        }
                    });
                }
            });

        if let Some(idx) = remove_idx {
            self.manager.remove(idx);
        }

        ui.separator();
        ui.horizontal(|ui| {
            ui.label("Add:");
            ui.add(
                egui::TextEdit::singleline(&mut self.new_cheat_label)
                    .desired_width(60.0)
                    .hint_text("F8xxxx"),
            );
            ui.label("=");
            ui.add(
                egui::TextEdit::singleline(&mut self.new_cheat_value)
                    .desired_width(25.0)
                    .hint_text("xx"),
            );
            let cram_size = ram.len().saturating_sub(wram_size);
            if ui.button("Add").clicked() {
                if let (Some(addr), Ok(val)) = (
                    parse_cheat_addr(&self.new_cheat_label, wram_size, cram_size),
                    u8::from_str_radix(self.new_cheat_value.trim(), 16),
                ) {
                    self.manager.add(addr, val, format_addr(addr, wram_size));
                    self.new_cheat_label.clear();
                    self.new_cheat_value.clear();
                }
            }
        });
    }

    fn build_filter(&self) -> Option<SearchFilter> {
        let parse_val = || u8::from_str_radix(self.filter_value.trim(), 10).ok()
            .or_else(|| u8::from_str_radix(self.filter_value.trim().trim_start_matches("0x").trim_start_matches("0X"), 16).ok());

        match self.filter_kind {
            FilterKind::Equal => parse_val().map(SearchFilter::Equal),
            FilterKind::NotEqual => parse_val().map(SearchFilter::NotEqual),
            FilterKind::GreaterThan => parse_val().map(SearchFilter::GreaterThan),
            FilterKind::LessThan => parse_val().map(SearchFilter::LessThan),
            FilterKind::Increased => Some(SearchFilter::Increased),
            FilterKind::Decreased => Some(SearchFilter::Decreased),
            FilterKind::Changed => Some(SearchFilter::Changed),
            FilterKind::Unchanged => Some(SearchFilter::Unchanged),
            FilterKind::IncreasedBy => parse_val().map(SearchFilter::IncreasedBy),
            FilterKind::DecreasedBy => parse_val().map(SearchFilter::DecreasedBy),
            FilterKind::BcdEqual => {
                // Parse as decimal number (not hex)
                self.filter_value.trim().parse::<u16>().ok().map(SearchFilter::BcdEqual)
            }
        }
    }
}
