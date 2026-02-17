pub mod cheat_search;
pub mod gl_game;
pub mod hex_viewer;

use cheat_search::CheatSearchUi;
use hex_viewer::HexViewerState;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ActiveTab {
    HexViewer,
    CheatSearch,
}

pub struct CheatToolUi {
    pub active_tab: ActiveTab,
    pub hex_viewer: HexViewerState,
    pub cheat_search_ui: CheatSearchUi,
    pub panel_visible: bool,
    /// Frozen snapshot shown in the panel. Updated only on Refresh.
    pub ram_snapshot: Vec<u8>,
    /// Set to true when Refresh is clicked; consumed by the main loop.
    pub refresh_requested: bool,
    /// When true, emulation is paused (no ticks). Useful for cheat search.
    pub paused: bool,
}

impl CheatToolUi {
    pub fn new() -> Self {
        Self {
            active_tab: ActiveTab::HexViewer,
            hex_viewer: HexViewerState::new(),
            cheat_search_ui: CheatSearchUi::new(),
            panel_visible: false,
            ram_snapshot: vec![0u8; 0x2000],
            refresh_requested: false,
            paused: false,
        }
    }

    /// Copy live RAM into the display snapshot.
    pub fn refresh(&mut self, ram: &[u8]) {
        let prev = self.ram_snapshot.clone();
        let len = ram.len().min(self.ram_snapshot.len());
        self.ram_snapshot[..len].copy_from_slice(&ram[..len]);
        self.hex_viewer.update_prev(&prev);
    }

    pub fn show_panel(
        &mut self,
        ui: &mut egui::Ui,
        ram_writes: &mut Vec<(usize, u8)>,
        live_ram: &[u8],
        cheat_path: Option<&std::path::Path>,
    ) {
        ui.horizontal(|ui| {
            ui.selectable_value(&mut self.active_tab, ActiveTab::HexViewer, "Hex Viewer");
            ui.selectable_value(&mut self.active_tab, ActiveTab::CheatSearch, "Cheat Search");
            ui.separator();
            ui.checkbox(&mut self.paused, "Pause");
        });
        ui.separator();

        match self.active_tab {
            ActiveTab::HexViewer => {
                ui.horizontal(|ui| {
                    if ui.button("Refresh").clicked() {
                        self.refresh_requested = true;
                    }
                    ui.label("(Update hex display from live RAM)");
                });
                ui.separator();
                let snap = &self.ram_snapshot;
                self.hex_viewer.show(ui, snap, ram_writes);
            }
            ActiveTab::CheatSearch => {
                self.cheat_search_ui.show(ui, live_ram, cheat_path);
            }
        }
    }
}
