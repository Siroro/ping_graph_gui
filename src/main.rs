#![windows_subsystem = "windows"]

use eframe::egui::{self, Label};
use egui_plot::{Line, Plot, PlotBounds, PlotPoints};
use ping::ping;
use std::net::ToSocketAddrs;
use std::thread::sleep;
use std::time::{Duration, Instant};

#[global_allocator]
static ALLOC: snmalloc_rs::SnMalloc = snmalloc_rs::SnMalloc;

struct PingApp {
    address: String,
    ping_times: Vec<(f64, f64)>,
    last_ping: Instant,
}

impl Default for PingApp {
    fn default() -> Self {
        Self {
            address: "8.8.8.8".to_string(),
            ping_times: Vec::new(),
            last_ping: Instant::now(),
        }
    }
}

impl eframe::App for PingApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("Ping Graph");

            ui.horizontal(|ui| {
                ui.label("Address to ping:");
                ui.text_edit_singleline(&mut self.address);
                if ui.button("Reset").clicked() {
                    self.ping_times.clear();
                    self.last_ping = Instant::now();
                }
            });

            
            if self.last_ping.elapsed() >= Duration::from_secs(1) {
                self.last_ping = Instant::now();
                
                // Attempt to resolve the address (works for both IP addresses and hostnames)
                match (self.address.as_str(), 0).to_socket_addrs() {
                    
                    Ok(mut addrs) => {
                        if let Some(sock_addr) = addrs.next() {
                            let ip = sock_addr.ip();
                            let start = Instant::now();
                            match ping(ip, None, None, None, None, None) {
                                Ok(_) => {
                                    let duration = start.elapsed();
                                    let ping_time = duration.as_millis() as f64;
                                    let time = self.ping_times.len() as f64;
                                    self.ping_times.push((time, ping_time));
                                    println!("Ping time: {:.2} ms", ping_time);
                                }
                                Err(e) => println!("Ping failed: {}", e),
                            }
                        } else {
                            println!("Could not resolve address: {}", self.address);
                        }
                    }
                    Err(e) =>  { 
                        ui.label(format!("Invalid address: {}. Error: {}", self.address, e.to_string()));
                        println!("Invalid address: {}. Error: {}", self.address, e);
                    }
                }
            }
            
            let stats = calculate_ping_stats(&self.ping_times);
            let (_, worst, _) = stats.unwrap_or((0.0, 100.0, 0.0));
            
            let points: Vec<[f64; 2]> = self.ping_times
                .iter()
                .map(|(x, y)| [*x, *y])
                .collect();
            
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
                        [size, worst + 10.0]
                        
                    ));
                });
                
            
            match stats {
                Some((best, worst, average)) => {
                    ui.label(format!(
                        "{:.2}ms best, {:.2}ms worst, {:.2}ms average",
                        best, worst, average
                    ));
                },
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

fn main() -> Result<(), eframe::Error> {
    let options: eframe::NativeOptions = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([800.0, 600.0])
            .with_min_inner_size([300.0, 220.0]),
        ..Default::default()
    };
    eframe::run_native(
        "Ping Graph",
        options,
        Box::new(|_cc| Ok(Box::new(PingApp::default()))),
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