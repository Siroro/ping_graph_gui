#![windows_subsystem = "windows"]

use eframe::egui::{self, Label};
use egui_plot::{Line, Plot, PlotBounds, PlotPoints};
use ping::ping;
use std::net::ToSocketAddrs;
use std::sync::{mpsc, Arc, RwLock};
use std::thread::{self, sleep};
use std::time::{Duration, Instant};

struct PingApp {
    ping_times: Vec<(f64, f64)>,
    last_ping: Instant,
    shared_data: Arc<RwLock<SharedPingData>>,
    rx: mpsc::Receiver<f64>, // Receiver to get ping times from the thread
}

impl Default for PingApp {
    fn default() -> Self {
        Self {
            ping_times: Vec::new(),
            last_ping: Instant::now(),
            shared_data: Arc::new(RwLock::new(SharedPingData {
                address: "8.8.8.8".to_string(),
            })),
            rx: mpsc::channel().1,
        }
    }
}

impl PingApp {
    fn new(shared_data: Arc<RwLock<SharedPingData>>, rx: mpsc::Receiver<f64>) -> Self {
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

            let mut shared_data = self.shared_data.write().unwrap(); // Write lock for modifying the address

            ui.horizontal(|ui| {
                ui.label("Address to ping:");
                ui.text_edit_singleline(&mut shared_data.address);
                if ui.button("Reset").clicked() {
                    self.ping_times.clear();
                    self.last_ping = Instant::now();
                }
            });

            drop(shared_data); // Release the write lock

            if let Ok(ping_time) = self.rx.try_recv() {
                let time = self.ping_times.len() as f64;
                self.ping_times.push((time, ping_time));
                println!("Ping time in UI: {:.2} ms", ping_time);
            }

            let stats = calculate_ping_stats(&self.ping_times);
            let (_, worst, _) = stats.unwrap_or((0.0, 100.0, 0.0));

            let points: Vec<[f64; 2]> = self.ping_times.iter().map(|(x, y)| [*x, *y]).collect();

            Plot::new("ping_plot")
                .view_aspect(2.0)
                .allow_scroll(false)
                .allow_zoom(false)
                .allow_drag(false)
                .show(ui, |plot_ui| {
                    let line = Line::new(PlotPoints::from(points.clone()));
                    plot_ui.line(line);
                    let size = points.len() as f64;
                    plot_ui.set_plot_bounds(PlotBounds::from_min_max(
                        [0.0, 0.0],
                        [size, worst + 10.0],
                    ));
                });

            match stats {
                Some((best, worst, average)) => {
                    ui.label(format!(
                        "{:.2}ms best, {:.2}ms worst, {:.2}ms average",
                        best, worst, average
                    ));
                }
                None => {
                    ui.label("No ping times available.");
                }
            }

            ui.add(Label::new(""));
        });

        // Request a repaint every frame to continuously update the UI
        ctx.request_repaint();
        sleep(Duration::from_millis(16));
    }
}

struct SharedPingData {
    address: String,
}

fn main() -> Result<(), eframe::Error> {
    let options: eframe::NativeOptions = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([800.0, 600.0])
            .with_min_inner_size([300.0, 220.0]),
        ..Default::default()
    };

    let shared_ping_data: Arc<RwLock<SharedPingData>> = Arc::new(RwLock::new(SharedPingData {
        address: "8.8.8.8".to_string(),
    }));
    let shared_ping_data_for_thread = Arc::clone(&shared_ping_data);

    let (tx, rx) = std::sync::mpsc::channel();
    thread::spawn(move || {
        loop {
            let shared_data = shared_ping_data_for_thread.read().unwrap(); // Write lock for modifying shared data
            let start = Instant::now();
            let address = &shared_data.address.clone();
            drop(shared_data);
            match (address.as_str(), 0).to_socket_addrs() {
                Ok(mut addrs) => {
                    if let Some(sock_addr) = addrs.next() {
                        let ip = sock_addr.ip();
                        match ping(ip, None, None, None, None, None) {
                            Ok(_) => {
                                let duration = start.elapsed();
                                tx.send(duration.as_millis() as f64).unwrap();
                            }
                            Err(e) => println!("Ping failed: {}", e),
                        }
                    } else {
                        println!("Could not resolve address: {}", address);
                    }
                }
                Err(e) => {
                    println!("Invalid address: {}. Error: {}", address, e);
                }
            }

            // Wait for 1 second before pinging again
            thread::sleep(Duration::from_secs(1));
        }
    });

    let shared_ping_data_for_app = Arc::clone(&shared_ping_data);
    eframe::run_native(
        "Ping Graph",
        options,
        Box::new(|_cc| Ok(Box::new(PingApp::new(shared_ping_data_for_app, rx)))),
    )
}

fn calculate_ping_stats(ping_times: &Vec<(f64, f64)>) -> Option<(f64, f64, f64)> {
    if ping_times.is_empty() {
        return None; // Return None if there are no ping times
    }

    let pings: Vec<f64> = ping_times.iter().map(|&(_, ping)| ping).collect();

    let min_ping = pings.iter().cloned().fold(f64::INFINITY, f64::min);
    let max_ping = pings.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    let avg_ping = pings.iter().sum::<f64>() / pings.len() as f64;

    Some((min_ping, max_ping, avg_ping))
}
