mod app;
mod db;
mod models;
mod ui;

use app::FlipsyApp;
use eframe::egui::Vec2;

pub fn create_f_icon() -> eframe::egui::IconData {
    let w = 32;
    let h = 32;
    let mut rgba = vec![0u8; w * h * 4];

    for y in 0..h {
        for x in 0..w {
            let idx = (y * w + x) * 4;
            let dx = (x as f32 - 15.5).abs() - (15.5 - 6.0);
            let dy = (y as f32 - 15.5).abs() - (15.5 - 6.0);
            let dx = dx.max(0.0);
            let dy = dy.max(0.0);
            let dist_sq = dx * dx + dy * dy;

            if dist_sq <= 6.0 * 6.0 {
                let mut r = 24u8;
                let mut g = 24u8;
                let mut b = 27u8;
                let a = 255u8;

                let in_spine = x >= 9 && x <= 13 && y >= 7 && y <= 24;
                let in_top = x >= 9 && x <= 23 && y >= 7 && y <= 10;
                let in_mid = x >= 9 && x <= 19 && y >= 14 && y <= 17;

                if in_spine || in_top || in_mid {
                    r = 255;
                    g = 255;
                    b = 255;
                }

                rgba[idx] = r;
                rgba[idx + 1] = g;
                rgba[idx + 2] = b;
                rgba[idx + 3] = a;
            } else {
                rgba[idx] = 0;
                rgba[idx + 1] = 0;
                rgba[idx + 2] = 0;
                rgba[idx + 3] = 0;
            }
        }
    }

    eframe::egui::IconData {
        rgba,
        width: w as u32,
        height: h as u32,
    }
}

fn main() {
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_title("Flipsy - Oracle Client")
            .with_icon(create_f_icon())
            .with_inner_size(Vec2::new(360.0, 580.0))
            .with_min_inner_size(Vec2::new(340.0, 480.0))
            .with_resizable(true),
        ..Default::default()
    };

    println!("[Flipsy] Initializing native window with custom F icon...");
    let result = eframe::run_native(
        "Flipsy - Oracle Client",
        options,
        Box::new(|cc| Ok(Box::new(FlipsyApp::new(cc)))),
    );

    if let Err(e) = result {
        eprintln!("[Flipsy] Error running native app: {:?}", e);
    } else {
        println!("[Flipsy] App closed cleanly.");
    }
}
