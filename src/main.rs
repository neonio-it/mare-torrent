// Sem janela de console na versão final.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod engine;
mod settings;
mod theme;
mod util;
mod widgets;

use eframe::egui;

fn main() -> eframe::Result {
    // Um .torrent ou magnet passado na linha de comando (ex.: "Abrir com" no Explorer).
    let initial = std::env::args().nth(1).and_then(|a| engine::Source::parse(&a));

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Maré")
            .with_app_id("mare-torrent")
            .with_inner_size([1060.0, 690.0])
            .with_min_inner_size([780.0, 480.0])
            .with_drag_and_drop(true)
            .with_icon(app_icon()),
        centered: true,
        ..Default::default()
    };

    eframe::run_native(
        "Maré",
        options,
        Box::new(move |cc| Ok(Box::new(app::MareApp::new(cc, initial)))),
    )
}

/// Ícone da janela desenhado em código: quadrado azul arredondado com duas ondas.
fn app_icon() -> egui::IconData {
    const N: usize = 64;
    let mut rgba = vec![0u8; N * N * 4];
    let bg = [0x4Cu8, 0x9E, 0xFF];
    let radius = 15.0f32;
    let waves = [(-0.16f32, 0.0f32), (0.12, 0.9)];
    for y in 0..N {
        for x in 0..N {
            let (fx, fy) = (x as f32 + 0.5, y as f32 + 0.5);
            // Cobertura do quadrado arredondado (com antisserrilhado simples).
            let dx = (radius - fx).max(fx - (N as f32 - radius)).max(0.0);
            let dy = (radius - fy).max(fy - (N as f32 - radius)).max(0.0);
            let d = (dx * dx + dy * dy).sqrt() - radius;
            let cover = (0.5 - d).clamp(0.0, 1.0);
            if cover <= 0.0 {
                continue;
            }
            // Ondas brancas.
            let w = N as f32;
            let mut wave = 0.0f32;
            let t = (fx / w - 0.18) / 0.64;
            if (0.0..=1.0).contains(&t) {
                for (dy_rel, phase) in waves {
                    let cy = w / 2.0 + w * dy_rel + (t * std::f32::consts::TAU + phase).sin() * w * 0.07;
                    let dist = (fy - cy).abs();
                    wave = wave.max((w * 0.045 + 0.5 - dist).clamp(0.0, 1.0));
                }
            }
            let i = (y * N + x) * 4;
            for c in 0..3 {
                rgba[i + c] = (bg[c] as f32 * (1.0 - wave) + 255.0 * wave) as u8;
            }
            rgba[i + 3] = (cover * 255.0) as u8;
        }
    }
    egui::IconData { rgba, width: N as u32, height: N as u32 }
}
