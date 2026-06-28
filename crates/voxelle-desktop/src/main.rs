use anyhow::{Context, Result};
use eframe::egui;
use std::collections::BTreeMap;
use std::net::SocketAddr;
use std::path::PathBuf;
use voxelle_app::{
    HomeScreenView, PeerRecord, RuntimeState, VoxelleHome, VoxelleRuntime, DEFAULT_ROOM_ID,
};

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions::default();
    eframe::run_native(
        "Voxelle",
        options,
        Box::new(|_cc| Ok(Box::new(VoxelleDesktop::new()))),
    )
}

struct VoxelleDesktop {
    home: VoxelleHome,
    runtime: Option<VoxelleRuntime>,
    task_runtime: tokio::runtime::Runtime,
    bind_addr: String,
    advertise_addr: String,
    peer_record_input: String,
    message_input: String,
    last_action: Option<String>,
    peer_actions: BTreeMap<String, String>,
}

impl VoxelleDesktop {
    fn new() -> Self {
        let home = VoxelleHome::new(default_home_path());
        let task_runtime = tokio::runtime::Runtime::new().expect("create desktop async runtime");
        let mut app = Self {
            home,
            runtime: None,
            task_runtime,
            bind_addr: "[::1]:0".to_string(),
            advertise_addr: String::new(),
            peer_record_input: String::new(),
            message_input: String::new(),
            last_action: None,
            peer_actions: BTreeMap::new(),
        };
        app.run_action("Initialized home", |app| {
            app.home.init(DEFAULT_ROOM_ID)?;
            Ok(())
        });
        app
    }

    fn run_action(
        &mut self,
        ok_message: impl Into<String>,
        action: impl FnOnce(&mut Self) -> Result<()>,
    ) {
        let ok_message = ok_message.into();
        match action(self) {
            Ok(()) => self.last_action = Some(ok_message),
            Err(error) => self.last_action = Some(format!("Error: {error:#}")),
        }
    }

    fn view(&self) -> Result<HomeScreenView> {
        self.home
            .home_screen_view(self.runtime.as_ref())
            .context("build home screen")
    }

    fn go_online(&mut self) -> Result<()> {
        let bind = self
            .bind_addr
            .trim()
            .parse::<SocketAddr>()
            .context("parse bind address")?;
        let advertise = parse_optional_socket_addr(&self.advertise_addr)?;
        self.runtime = Some(self.home.listen(bind, advertise)?);
        Ok(())
    }

    fn go_offline(&mut self) -> Result<()> {
        if let Some(runtime) = self.runtime.take() {
            self.task_runtime.block_on(runtime.stop());
        }
        Ok(())
    }

    fn import_peer_record(&mut self) -> Result<()> {
        let record: PeerRecord =
            serde_json::from_str(&self.peer_record_input).context("parse peer record JSON")?;
        let peer_id = record.endpoint.peer_id.clone();
        self.home.import_peer_record(record)?;
        self.peer_record_input.clear();
        self.peer_actions.remove(&peer_id);
        Ok(())
    }

    fn send_message(&mut self) -> Result<()> {
        let text = self.message_input.trim();
        if text.is_empty() {
            anyhow::bail!("message is empty");
        }
        self.home.send_message(text, None)?;
        self.message_input.clear();
        Ok(())
    }

    fn diagnose_peer(&mut self, peer_id: &str) -> Result<()> {
        let peer = self
            .home
            .known_peers()?
            .into_iter()
            .find(|peer| peer.endpoint.peer_id == peer_id)
            .context("peer not found")?;
        let report = self.task_runtime.block_on(self.home.diagnose_peer(&peer))?;
        let value = if report.reachable {
            "reachable".to_string()
        } else {
            format!(
                "unreachable: {}",
                report
                    .error
                    .unwrap_or_else(|| "no error detail".to_string())
            )
        };
        self.peer_actions.insert(peer_id.to_string(), value);
        Ok(())
    }

    fn sync_peer(&mut self, peer_id: &str) -> Result<()> {
        let peer = self
            .home
            .known_peers()?
            .into_iter()
            .find(|peer| peer.endpoint.peer_id == peer_id)
            .context("peer not found")?;
        let report = self.task_runtime.block_on(self.home.sync_peer(&peer, 64))?;
        self.peer_actions.insert(
            peer_id.to_string(),
            format!(
                "sync: governance accepted {}, room accepted {}",
                report.governance.accepted, report.room.accepted
            ),
        );
        Ok(())
    }

    fn header(&self, ui: &mut egui::Ui, view: &HomeScreenView) {
        ui.heading("Voxelle");
        ui.label(view.profile.home.display().to_string());
        if let Some(action) = &self.last_action {
            ui.label(action);
        }
    }

    fn profile_section(&mut self, ui: &mut egui::Ui, view: &HomeScreenView) {
        ui.heading("Profile");
        egui::Grid::new("profile_grid")
            .num_columns(2)
            .spacing([12.0, 6.0])
            .show(ui, |ui| {
                ui.label("Peer");
                ui.monospace(short_id(&view.profile.peer_id));
                ui.end_row();
                ui.label("Device");
                ui.monospace(short_id(&view.profile.device_id));
                ui.end_row();
                ui.label("Room");
                ui.monospace(&view.profile.default_room);
                ui.end_row();
            });
    }

    fn runtime_section(&mut self, ui: &mut egui::Ui, view: &HomeScreenView) {
        ui.heading("Runtime");
        match view.runtime.state {
            RuntimeState::Offline => {
                ui.label("Offline");
                ui.horizontal(|ui| {
                    ui.label("Bind");
                    ui.text_edit_singleline(&mut self.bind_addr);
                });
                ui.horizontal(|ui| {
                    ui.label("Advertise");
                    ui.text_edit_singleline(&mut self.advertise_addr);
                });
                if ui.button("Go online").clicked() {
                    self.run_action("Online", Self::go_online);
                }
            }
            RuntimeState::Online => {
                ui.label("Online");
                egui::Grid::new("runtime_grid")
                    .num_columns(2)
                    .spacing([12.0, 6.0])
                    .show(ui, |ui| {
                        ui.label("Listen");
                        ui.monospace(
                            view.runtime
                                .listen_addr
                                .map(|addr| addr.to_string())
                                .unwrap_or_default(),
                        );
                        ui.end_row();
                        ui.label("Advertise");
                        ui.monospace(
                            view.runtime
                                .advertised_addr
                                .map(|addr| addr.to_string())
                                .unwrap_or_default(),
                        );
                        ui.end_row();
                    });
                for note in &view.runtime.reachability_notes {
                    ui.label(note);
                }
                if ui.button("Go offline").clicked() {
                    self.run_action("Offline", Self::go_offline);
                }
            }
        }
    }

    fn invite_section(&mut self, ui: &mut egui::Ui, view: &HomeScreenView) {
        ui.heading("Invite");
        if let Some(invite) = &view.invite {
            let mut text = invite.peer_record_json.clone();
            ui.add(
                egui::TextEdit::multiline(&mut text)
                    .font(egui::TextStyle::Monospace)
                    .desired_rows(8)
                    .interactive(false),
            );
        } else {
            ui.label("Offline");
        }
    }

    fn import_section(&mut self, ui: &mut egui::Ui) {
        ui.heading("Import");
        ui.add(
            egui::TextEdit::multiline(&mut self.peer_record_input)
                .font(egui::TextStyle::Monospace)
                .desired_rows(8),
        );
        if ui.button("Import peer").clicked() {
            self.run_action("Peer imported", Self::import_peer_record);
        }
    }

    fn peers_section(&mut self, ui: &mut egui::Ui, view: &HomeScreenView) {
        ui.heading("Peers");
        if view.peers.is_empty() {
            ui.label("None");
            return;
        }

        egui::ScrollArea::vertical()
            .id_salt("peers")
            .max_height(240.0)
            .show(ui, |ui| {
                for peer in &view.peers {
                    ui.separator();
                    ui.strong(&peer.label);
                    egui::Grid::new(format!("peer_{}", peer.peer_id))
                        .num_columns(2)
                        .spacing([12.0, 6.0])
                        .show(ui, |ui| {
                            ui.label("Peer");
                            ui.monospace(short_id(&peer.peer_id));
                            ui.end_row();
                            ui.label("Device");
                            ui.monospace(short_id(&peer.device_id));
                            ui.end_row();
                            ui.label("Address");
                            ui.monospace(peer.addr.to_string());
                            ui.end_row();
                            ui.label("Room");
                            ui.monospace(&peer.default_room);
                            ui.end_row();
                        });
                    ui.horizontal(|ui| {
                        if ui.button("Diagnose").clicked() {
                            self.run_action("Diagnostic complete", |app| {
                                app.diagnose_peer(&peer.peer_id)
                            });
                        }
                        if ui.button("Sync").clicked() {
                            self.run_action("Sync complete", |app| app.sync_peer(&peer.peer_id));
                        }
                    });
                    if let Some(result) = self.peer_actions.get(&peer.peer_id) {
                        ui.label(result);
                    }
                }
            });
    }

    fn room_section(&mut self, ui: &mut egui::Ui, view: &HomeScreenView) {
        ui.heading("Room");
        ui.monospace(&view.room.room_id);
        egui::ScrollArea::vertical()
            .id_salt("room")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                if view.room.messages.is_empty() {
                    ui.label("No messages");
                }
                for message in &view.room.messages {
                    ui.separator();
                    ui.monospace(short_id(&message.author_peer_id));
                    ui.label(&message.text);
                }
            });

        ui.separator();
        ui.add(
            egui::TextEdit::multiline(&mut self.message_input)
                .desired_rows(4)
                .hint_text("Message"),
        );
        if ui.button("Send").clicked() {
            self.run_action("Message sent", Self::send_message);
        }
    }
}

impl eframe::App for VoxelleDesktop {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let view = match self.view() {
            Ok(view) => view,
            Err(error) => {
                egui::CentralPanel::default().show(ctx, |ui| {
                    ui.heading("Voxelle");
                    ui.label(format!("Error: {error:#}"));
                });
                return;
            }
        };

        egui::TopBottomPanel::top("top").show(ctx, |ui| {
            self.header(ui, &view);
        });

        egui::SidePanel::left("left")
            .resizable(true)
            .default_width(380.0)
            .show(ctx, |ui| {
                self.profile_section(ui, &view);
                ui.separator();
                self.runtime_section(ui, &view);
                ui.separator();
                self.invite_section(ui, &view);
                ui.separator();
                self.import_section(ui);
                ui.separator();
                self.peers_section(ui, &view);
            });

        egui::CentralPanel::default().show(ctx, |ui| {
            self.room_section(ui, &view);
        });
    }
}

fn default_home_path() -> PathBuf {
    if let Some(path) = std::env::var_os("VOXELLE_HOME") {
        return PathBuf::from(path);
    }
    if let Some(path) = std::env::var_os("HOME") {
        return PathBuf::from(path).join(".voxelle");
    }
    std::env::current_dir()
        .unwrap_or_else(|_| PathBuf::from("."))
        .join(".voxelle")
}

fn parse_optional_socket_addr(value: &str) -> Result<Option<SocketAddr>> {
    let value = value.trim();
    if value.is_empty() {
        Ok(None)
    } else {
        Ok(Some(value.parse().context("parse advertise address")?))
    }
}

fn short_id(value: &str) -> String {
    value
        .strip_prefix("ed25519:")
        .and_then(|rest| rest.get(..12))
        .unwrap_or(value)
        .to_string()
}
