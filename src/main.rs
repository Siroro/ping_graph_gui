#![windows_subsystem = "windows"]

use eframe::egui::{self};
use egui_plot::{Line, Plot, PlotBounds};
use ping::ping;
use std::net::ToSocketAddrs;
use std::sync::{mpsc, Arc, RwLock};
use std::thread::{self};
use std::time::{Duration, Instant};

struct PingApp {
    ping_times: Vec<[f64; 2]>,       // Stores ping times
    stats: Option<(f64, f64, f64)>,   // Cached ping statistics: (best, worst, average)
    ping_times_updated: bool,         // Flag indicating if ping times were updated
    last_ping: Instant,               // Last time a ping was sent
    shared_data: Arc<RwLock<PingSharedState>>, // Shared address to ping
    rx: mpsc::Receiver<f64>,          // Receiver to get ping times from the thread
}

impl Default for PingApp {
    fn default() -> Self {
        let (_, rx) = mpsc::channel(); // Initialize both sender and receiver
        Self {
            ping_times: Vec::new(),
            stats: None,                    // No stats initially
            ping_times_updated: false,      // No updates initially
            last_ping: Instant::now(),
            shared_data: Arc::new(RwLock::new(PingSharedState {
                address: "8.8.8.8".to_string(), // Default address
                error: "".to_string()
            })),
            rx, // Set up receiver for ping times
        }
    }
}

impl PingApp {
    fn new(shared_data: Arc<RwLock<PingSharedState>>, rx: mpsc::Receiver<f64>) -> Self {
        Self {
            shared_data,
            rx,
            ..Default::default()
        }
    }
}

impl eframe::App for PingApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("Ping Graph");

            let mut address = {
                let shared_data = self.shared_data.read().unwrap();
                shared_data.address.clone()
            };

            ui.horizontal(|ui| {
                ui.label("Address to ping:");
                if ui.text_edit_singleline(&mut address).changed() {
                    let mut shared_data = self.shared_data.write().unwrap();
                    shared_data.address = address;
                }
                if ui.button("Reset").clicked() {
                    self.ping_times.clear();
                    self.last_ping = Instant::now();
                    self.ping_times_updated = true;
                }
                let shared_data = self.shared_data.read().unwrap();
                let mut err = shared_data.error.clone();
                err.truncate(90);
                ui.label(egui::RichText::new(err).color(egui::Color32::RED));
            });

            // Check for new ping times
            while let Ok(ping_time) = self.rx.try_recv() {
                let time = self.ping_times.len() as f64;
                self.ping_times.push([time, ping_time]);
                self.ping_times_updated = true;
            }

            if self.ping_times_updated {
                ctx.request_repaint();
                self.stats = calculate_ping_stats(&self.ping_times);
                self.ping_times_updated = false;
            }

            let (_, worst, _) = self.stats.unwrap_or((0.0, 100.0, 0.0));

            Plot::new("ping_plot")
                .view_aspect(2.0)
                .allow_scroll(false)
                .allow_zoom(false)
                .allow_drag(false)
                .show(ui, |plot_ui| {
                    plot_ui.line(Line::new(self.ping_times.clone()));
                    let size = self.ping_times.len() as f64;
                    plot_ui.set_plot_bounds(PlotBounds::from_min_max(
                        [0.0, 0.0],
                        [size, worst + 10.0],
                    ));
                });

            if let Some((best, worst, average)) = self.stats {
                ui.label(format!(
                    "{:.2}ms best, {:.2}ms worst, {:.2}ms average",
                    best, worst, average
                ));
            } else {
                ui.label("No ping times available.");
            }

            ui.add_space(10.0); 
        });
        ctx.request_repaint();
        std::thread::sleep(Duration::from_millis(16));
    }
}
struct PingSharedState {
    address: String,
    error: String
}

fn main() -> Result<(), eframe::Error> {
    let options: eframe::NativeOptions = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([800.0, 600.0])
            .with_min_inner_size([300.0, 220.0]),
        ..Default::default()
    };

    let shared_ping_data: Arc<RwLock<PingSharedState>> = Arc::new(RwLock::new(PingSharedState {
        address: "8.8.8.8".to_string(),
        error: "".to_string()
    }));
    let shared_ping_data_for_thread = Arc::clone(&shared_ping_data);

    let (tx, rx) = std::sync::mpsc::channel();
    thread::spawn(move || {
        loop {
            let shared_data = shared_ping_data_for_thread.read().unwrap(); 
            let start = Instant::now();
            let address = &shared_data.address.clone();
            drop(shared_data);
            let mut success  = false;
            match (address.as_str(), 0).to_socket_addrs() {
                Ok(mut addrs) => {
                    if let Some(sock_addr) = addrs.next() {
                        let ip = sock_addr.ip();
                        match ping(ip, None, None, None, None, None) {
                            Ok(_) => {
                                let duration = start.elapsed();
                                tx.send(duration.as_millis() as f64).unwrap();
                                let mut shared_data = shared_ping_data_for_thread.write().unwrap(); 
                                shared_data.error = "".to_string();
                                success = true;
                            }
                            Err(e) => { 
                                let mut shared_data = shared_ping_data_for_thread.write().unwrap(); 
                                shared_data.error = format!("Ping failed: {}", e);
                                println!("Ping failed: {}", e)
                            },
                        }
                    } else {
                        let mut shared_data = shared_ping_data_for_thread.write().unwrap(); 
                        shared_data.error = format!("Could not resolve address: {}", address);
                        println!("Could not resolve address: {}", address);
                    }
                }
                Err(e) => {
                    let mut shared_data = shared_ping_data_for_thread.write().unwrap(); 
                    shared_data.error = format!("Invalid address: {}. Error: {}", address, e);
                    println!("Invalid address: {}. Error: {}", address, e);
                }
            }

            if success {
                thread::sleep(Duration::from_secs(1));
            }
        }
    });

    let shared_ping_data_for_app = Arc::clone(&shared_ping_data);
    eframe::run_native(
        "Ping Graph",
        options,
        Box::new(|_cc| Ok(Box::new(PingApp::new(shared_ping_data_for_app, rx)))),
    )
}

fn calculate_ping_stats(ping_times: &[[f64; 2]]) -> Option<(f64, f64, f64)> {
    if ping_times.is_empty() {
        return None;
    }

    let mut min_ping = f64::INFINITY;
    let mut max_ping = f64::NEG_INFINITY;
    let mut total_ping = 0.0;
    let mut count = 0;

    for &[_time, ping] in ping_times.iter() {
        if ping < min_ping {
            min_ping = ping;
        }
        if ping > max_ping {
            max_ping = ping;
        }
        total_ping += ping;
        count += 1;
    }

    let avg_ping = total_ping / count as f64;

    Some((min_ping, max_ping, avg_ping))
}