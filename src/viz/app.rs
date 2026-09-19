//! egui App — pheromone heatmap + ants + control panel + click-to-deposit.

use crate::config::Config;
use crate::genome::Genome;
use crate::sim::Simulator;
use crate::viz::render::render_channel;
use crate::world::Channel;
use eframe::egui;
use egui::TextureHandle;

pub struct App {
    pub sim: Simulator,
    pub genome: Genome,
    pub config: Config,
    tex: Option<TextureHandle>,
    /// 4 small thumbnails (Trail/Home/Alarm/Recruit) shown tiled
    tex_all: [Option<TextureHandle>; 4],
    channel: Channel,
    scale: f32,
    auto_gain: bool,
    ticks_per_frame: u32,
    seed: u64,
    colony_size: usize,
    env_name: String,
    /// decision layer for the live sim: "fsm" / "ann" / "cppn"
    brain: String,
    /// canvas click placement mode: "none"/"food"/"enemy"/"wall"/"erase"
    place_mode: String,
    /// collected-over-time history for the sparkline
    collected_hist: Vec<f32>,
    /// T18.4: MB KC-sparsity history for the MB-activity sparkline (brain=mb)
    mb_sparsity_hist: Vec<f32>,
    /// T19.4: per-ant neural overlay mode ("none"/"dopamine"/"carrying")
    overlay: String,
    /// T19.4: evolution history (best, mean, worst) loaded from CSV for replay
    history: Vec<(f32, f32, f32)>,
}

impl App {
    pub fn from_config(_cc: &eframe::CreationContext<'_>, cfg: Config) -> Self {
        let genome = cfg.genome.clone();
        let seed = cfg.sim.seed;
        let colony = cfg.sim.colony_size;
        let mut sim = Simulator::new(cfg.world.width, cfg.world.height, seed, &genome);
        sim.set_colony_size(colony, &genome);
        sim.world.nest = crate::world::Vec2::new(cfg.world.nest_x, cfg.world.nest_y);
        sim.world.nest_radius = cfg.world.nest_radius;
        Self {
            sim,
            genome,
            config: cfg,
            tex: None,
            tex_all: Default::default(),
            channel: Channel::Trail,
            scale: 8.0,
            auto_gain: true,
            ticks_per_frame: 1,
            seed,
            colony_size: colony,
            env_name: "rich_close".to_string(),
            brain: "fsm".to_string(),
            place_mode: "none".to_string(),
            collected_hist: Vec::new(),
            mb_sparsity_hist: Vec::new(),
            overlay: "none".to_string(),
            history: Vec::new(),
        }
    }

    fn refresh_tex(&mut self, ctx: &egui::Context) {
        let img = render_channel(&self.sim.world, self.channel, self.scale, self.auto_gain);
        let opts = egui::TextureOptions::LINEAR;
        match &mut self.tex {
            Some(t) => t.set(img, opts),
            None => self.tex = Some(ctx.load_texture("field", img, opts)),
        }
        // 4-channel thumbnails (Trail/Home/Alarm/Recruit)
        let chans = [
            crate::world::Channel::Trail,
            crate::world::Channel::Home,
            crate::world::Channel::Alarm,
            crate::world::Channel::Recruitment,
        ];
        for (i, ch) in chans.iter().enumerate() {
            let im = render_channel(&self.sim.world, *ch, self.scale, true);
            match &mut self.tex_all[i] {
                Some(t) => t.set(im, opts),
                None => {
                    self.tex_all[i] =
                        Some(ctx.load_texture(format!("field{i}"), im, opts));
                }
            }
        }
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();

        // apply brain mode to the live sim (fsm/ann/cppn/mb/cx)
        let want_mb = self.brain == "mb";
        let want_cx = self.brain == "cx";
        let want_ann = self.brain == "ann" || self.brain == "cppn";
        if self.sim.brain_mb != want_mb
            || self.sim.brain_cx != want_cx
            || self.sim.brain_ann != want_ann
        {
            self.sim.brain_mb = want_mb;
            self.sim.brain_cx = want_cx;
            self.sim.brain_ann = want_ann;
            if self.brain == "cppn" {
                for a in self.sim.ants.iter_mut() {
                    a.genome.ann_weights = crate::genome::develop_phenotype(&a.genome);
                }
            }
        }

        // run simulation steps
        if !self.sim.paused {
            for _ in 0..self.ticks_per_frame {
                self.sim.step();
            }
            // sparkline sample: collected rate over the last window
            let h = &mut self.collected_hist;
            let last = h.last().copied().unwrap_or(0.0);
            if (self.sim.collected - last).abs() > 0.5 || h.is_empty() {
                h.push(self.sim.collected);
                if h.len() > 240 {
                    h.remove(0);
                }
            }
            // T18.4: MB KC-sparsity sample (mean fraction of active KCs spiking)
            if self.sim.brain_mb && !self.sim.ants.is_empty() {
                let kt = crate::genome::KC_THRESH;
                let mkc = crate::genome::MB_KC;
                let mut s = 0.0f32;
                let mut c = 0u32;
                for a in &self.sim.ants {
                    let act = a.mb_kc_active.min(mkc);
                    if act == 0 {
                        continue;
                    }
                    s += a.mb_kc_v.iter().take(act).filter(|&&v| v >= kt).count() as f32
                        / act as f32;
                    c += 1;
                }
                let mean = if c > 0 { s / c as f32 } else { 0.0 };
                self.mb_sparsity_hist.push(mean);
                if self.mb_sparsity_hist.len() > 240 {
                    self.mb_sparsity_hist.remove(0);
                }
            }
            ctx.request_repaint();
        }

        self.refresh_tex(&ctx);

        egui::containers::Panel::top("top").show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.heading("ants-sim");
                ui.label(format!("tick {}", self.sim.tick));
                ui.label(format!("ants {}", self.sim.ants.len()));
            });
        });

        egui::containers::Panel::right("controls")
            .resizable(true)
            .show(ui, |ui| {
                ui.add_space(8.0);
                ui.heading("Controls");
                ui.horizontal(|ui| {
                    if ui.button(if self.sim.paused { "Play" } else { "Pause" }).clicked() {
                        self.sim.paused = !self.sim.paused;
                    }
                    if ui.button("Step").clicked() {
                        self.sim.step();
                    }
                    if ui.button("Reset").clicked() {
                        self.sim.reset(self.seed, &self.genome);
                        self.tex = None;
                    }
                });
                ui.add_space(8.0);
                ui.label("Pheromone channel");
                ui.horizontal(|ui| {
                    if ui.radio_value(&mut self.channel, Channel::Trail, "Trail").changed() {
                        self.tex = None;
                    }
                    if ui.radio_value(&mut self.channel, Channel::Home, "Home").changed() {
                        self.tex = None;
                    }
                    if ui.radio_value(&mut self.channel, Channel::Alarm, "Alarm").changed() {
                        self.tex = None;
                    }
                    if ui.radio_value(&mut self.channel, Channel::Recruitment, "Recruit").changed()
                    {
                        self.tex = None;
                    }
                });
                ui.add_space(8.0);
                ui.label("Display gain");
                ui.add(egui::Slider::new(&mut self.scale, 1.0..=50.0).logarithmic(true));
                ui.checkbox(&mut self.auto_gain, "Auto gain (normalize to max)");
                ui.label("Ticks / frame");
                ui.add(egui::Slider::new(&mut self.ticks_per_frame, 1..=50));
                ui.add_space(8.0);
                ui.label("Colony size");
                if ui
                    .add(egui::Slider::new(&mut self.colony_size, 1usize..=2000).logarithmic(true))
                    .changed()
                {
                    self.sim.set_colony_size(self.colony_size, &self.genome);
                }
                ui.add_space(8.0);
                ui.label("Scenario");
                ui.horizontal(|ui| {
                    if ui.button("Single food").clicked() {
                        self.sim.scenario_single();
                        self.tex = None;
                    }
                    if ui.button("Two-branch").clicked() {
                        self.sim.scenario_two_branch();
                        self.tex = None;
                    }
                });
                if ui.button("Spawn enemy near nest").clicked() {
                    self.sim.spawn_enemy_near_nest();
                }
                ui.add_space(4.0);
                ui.label("Canvas click mode");
                ui.horizontal(|ui| {
                    for m in ["none", "food", "enemy", "wall", "erase"] {
                        ui.radio_value(&mut self.place_mode, m.to_string(), m);
                    }
                });
                ui.add_space(4.0);
                ui.label("Ant overlay (T19.4)");
                ui.horizontal(|ui| {
                    for m in ["none", "dopamine", "carrying"] {
                        ui.radio_value(&mut self.overlay, m.to_string(), m);
                    }
                });
                ui.add_space(10.0);
                ui.heading("Brain & live metrics");
                ui.label("Decision layer");
                egui::ComboBox::from_id_salt("brain_combo")
                    .selected_text(&self.brain)
                    .show_ui(ui, |ui| {
                        for b in ["fsm", "ann", "cppn", "mb", "cx"] {
                            ui.selectable_value(&mut self.brain, b.to_string(), b);
                        }
                    });
                // sparkline of collected over time
                let sp = ui.available_rect_before_wrap();
                let sh = egui::vec2(sp.width(), 40.0);
                let (rect, _) = ui.allocate_exact_size(sh, egui::Sense::hover());
                let p = ui.painter_at(rect);
                p.rect_filled(rect, 0.0, egui::Color32::from_rgb(20, 20, 30));
                let h = &self.collected_hist;
                if h.len() > 1 {
                    let mx = h.iter().cloned().fold(1.0f32, f32::max);
                    let n = h.len() as f32;
                    let mut pts: Vec<egui::Pos2> = Vec::with_capacity(h.len());
                    for (i, &v) in h.iter().enumerate() {
                        let x = rect.left() + (i as f32 / n) * rect.width();
                        let y = rect.bottom() - (v / mx) * rect.height() * 0.9;
                        pts.push(egui::pos2(x, y));
                    }
                    if pts.len() >= 2 {
                        p.add(egui::Shape::line(pts, egui::Stroke::new(1.5, egui::Color32::from_rgb(120, 220, 160))));
                    }
                }
                ui.label(format!("collected={:.0} ants={} brood={:.0}", self.sim.collected, self.sim.ants.len(), self.sim.world.brood));
                // T18.4: MB activity panel (KC sparsity sparkline + dopamine bars)
                if self.sim.brain_mb {
                    ui.label("MB activity (KC sparsity + dopamine)");
                    let spw = ui.available_rect_before_wrap().width();
                    // KC-sparsity sparkline
                    let (rect, _) =
                        ui.allocate_exact_size(egui::vec2(spw, 30.0), egui::Sense::hover());
                    let p = ui.painter_at(rect);
                    p.rect_filled(rect, 0.0, egui::Color32::from_rgb(20, 20, 30));
                    let mh = &self.mb_sparsity_hist;
                    if mh.len() > 1 {
                        let mx = mh.iter().cloned().fold(0.1f32, f32::max);
                        let n = mh.len() as f32;
                        let mut pts: Vec<egui::Pos2> = Vec::with_capacity(mh.len());
                        for (i, &v) in mh.iter().enumerate() {
                            let x = rect.left() + (i as f32 / n) * rect.width();
                            let y = rect.bottom() - (v / mx) * rect.height() * 0.9;
                            pts.push(egui::pos2(x, y));
                        }
                        if pts.len() >= 2 {
                            p.add(egui::Shape::line(
                                pts,
                                egui::Stroke::new(1.5, egui::Color32::from_rgb(220, 180, 80)),
                            ));
                        }
                    }
                    // mean dopamine reward / punish bars
                    let dm = crate::genome::DOPAMINE_MAX.max(1e-9);
                    let (mr, mp) = if self.sim.ants.is_empty() {
                        (0.0, 0.0)
                    } else {
                        let n = self.sim.ants.len() as f32;
                        let r: f32 = self.sim.ants.iter().map(|a| a.dopamine_reward).sum::<f32>() / n;
                        let p: f32 = self.sim.ants.iter().map(|a| a.dopamine_punish).sum::<f32>() / n;
                        (r, p)
                    };
                    for (label, val, col) in [
                        ("reward", mr, egui::Color32::from_rgb(120, 200, 120)),
                        ("punish", mp, egui::Color32::from_rgb(220, 100, 100)),
                    ] {
                        let (r2, _) =
                            ui.allocate_exact_size(egui::vec2(spw, 12.0), egui::Sense::hover());
                        let p = ui.painter_at(r2);
                        p.rect_filled(r2, 0.0, egui::Color32::from_rgb(20, 20, 30));
                        let w = r2.width() * (val / dm).min(1.0);
                        p.rect_filled(
                            egui::Rect::from_min_size(r2.left_top(), egui::vec2(w, r2.height())),
                            0.0,
                            col,
                        );
                        p.text(
                            r2.left_top() + egui::vec2(2.0, 2.0),
                            egui::Align2::LEFT_TOP,
                            format!("{} {:.2}", label, val),
                            egui::FontId::proportional(9.0),
                            egui::Color32::WHITE,
                        );
                    }
                }
                // T19.4: CX ring-attractor bump visualization (brain=cx)
                if self.sim.brain_cx {
                    ui.label("CX ring attractor (bump + heading)");
                    let spw = ui.available_rect_before_wrap().width();
                    let size = spw.min(96.0);
                    let (rect, _) =
                        ui.allocate_exact_size(egui::vec2(size, size), egui::Sense::hover());
                    let p = ui.painter_at(rect);
                    p.rect_filled(rect, 0.0, egui::Color32::from_rgb(20, 20, 30));
                    let c = rect.center();
                    let r = rect.width() * 0.4;
                    if let Some(a) = self.sim.ants.first() {
                        let n = crate::genome::CX_N;
                        let mx = a.cx_bump.iter().cloned().fold(1e-6f32, f32::max);
                        for i in 0..n {
                            let th = std::f32::consts::TAU * i as f32 / n as f32;
                            let pos = egui::pos2(c.x + th.cos() * r, c.y + th.sin() * r);
                            let v = (a.cx_bump[i] / mx).clamp(0.0, 1.0);
                            let col = egui::Color32::from_rgb(
                                (220.0 * v) as u8,
                                (180.0 * v) as u8,
                                (80.0 * v) as u8,
                            );
                            p.circle_filled(pos, 2.5 + 3.5 * v, col);
                        }
                        let h = crate::ant::brain::cx_heading(a);
                        let tip = egui::pos2(c.x + h.cos() * r * 1.15, c.y + h.sin() * r * 1.15);
                        p.line_segment(
                            [c, tip],
                            egui::Stroke::new(1.5, egui::Color32::from_rgb(120, 220, 160)),
                        );
                    }
                }
                // T19.4: evolution-history replay (best/mean/worst over gens)
                if !self.history.is_empty() {
                    ui.label("Evolution history (best/mean/worst)");
                    let spw = ui.available_rect_before_wrap().width();
                    let (rect, _) =
                        ui.allocate_exact_size(egui::vec2(spw, 50.0), egui::Sense::hover());
                    let p = ui.painter_at(rect);
                    p.rect_filled(rect, 0.0, egui::Color32::from_rgb(20, 20, 30));
                    let n = self.history.len() as f32;
                    let mx = self
                        .history
                        .iter()
                        .map(|(b, _m, _w)| *b)
                        .fold(1.0f32, f32::max);
                    for (col, idx) in [
                        (egui::Color32::from_rgb(120, 220, 160), 0usize),
                        (egui::Color32::from_rgb(120, 160, 220), 1),
                        (egui::Color32::from_rgb(160, 160, 160), 2),
                    ] {
                        let mut pts: Vec<egui::Pos2> = Vec::with_capacity(self.history.len());
                        for (i, h) in self.history.iter().enumerate() {
                            let v = match idx {
                                0 => h.0,
                                1 => h.1,
                                _ => h.2,
                            };
                            let x = rect.left() + (i as f32 / n) * rect.width();
                            let y = rect.bottom() - (v / mx) * rect.height() * 0.9;
                            pts.push(egui::pos2(x, y));
                        }
                        if pts.len() >= 2 {
                            p.add(egui::Shape::line(pts, egui::Stroke::new(1.5, col)));
                        }
                    }
                }
                // 4-channel thumbnails (2×2)
                ui.label("All channels (Trail/Home/Alarm/Recruit)");
                let thumb = 56.0;
                let names = ["Trail", "Home", "Alarm", "Recr"];
                for row in 0..2 {
                    ui.horizontal(|ui| {
                        for col in 0..2 {
                            let i = row * 2 + col;
                            let (rect, _) = ui.allocate_exact_size(egui::vec2(thumb, thumb), egui::Sense::hover());
                            if let Some(t) = &self.tex_all[i] {
                                ui.painter_at(rect).image(
                                    t.id(),
                                    rect,
                                    egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                                    egui::Color32::WHITE,
                                );
                            }
                            ui.painter_at(rect).text(
                                rect.left_top() + egui::vec2(2.0, 2.0),
                                egui::Align2::LEFT_TOP,
                                names[i],
                                egui::FontId::proportional(9.0),
                                egui::Color32::WHITE,
                            );
                        }
                    });
                }

                ui.add_space(10.0);
                ui.heading("Environment & genome");
                ui.label("Environment preset");
                egui::ComboBox::from_id_salt("env_combo")
                    .selected_text(&self.env_name)
                    .show_ui(ui, |ui| {
                        for (name, _desc) in crate::environment::Environment::presets() {
                            ui.selectable_value(&mut self.env_name, name.to_string(), name);
                        }
                    });
                if ui.button("Apply env").clicked() {
                    let env = crate::environment::Environment::build(
                        &self.env_name,
                        self.sim.world.nest,
                        self.sim.world.width as f32,
                        self.sim.world.height as f32,
                    );
                    self.sim.apply_environment(&env);
                    self.tex = None;
                }
                if ui.button("Load evolved champion").clicked() {
                    let path = format!("results/evolved_{}.toml", self.env_name);
                    if let Ok(c) = crate::config::Config::load(&path) {
                        self.genome = c.genome.clone();
                        self.sim.set_colony_size(self.colony_size, &self.genome);
                        let env = crate::environment::Environment::build(
                            &self.env_name,
                            self.sim.world.nest,
                            self.sim.world.width as f32,
                            self.sim.world.height as f32,
                        );
                        self.sim.apply_environment(&env);
                        self.tex = None;
                        // T19.4: also load the matching evolution-history CSV
                        // (gen,best,mean,worst) for the replay sparkline.
                        let csv = format!("results/evolved_{}_history.csv", self.env_name);
                        if let Ok(txt) = std::fs::read_to_string(&csv) {
                            self.history = txt
                                .lines()
                                .skip(1)
                                .filter_map(|l| {
                                    let f: Vec<&str> = l.split(',').collect();
                                    if f.len() >= 4 {
                                        Some((
                                            f[1].trim().parse().ok()?,
                                            f[2].trim().parse().ok()?,
                                            f[3].trim().parse().ok()?,
                                        ))
                                    } else {
                                        None
                                    }
                                })
                                .collect();
                        }
                    }
                }
                ui.add_space(4.0);
                let g = &self.genome;
                ui.label(
                    egui::RichText::new(format!(
                        "genome: fs={:.2} explore={:.2} follow={:.2} aggr={:.2} task_bias={:+.2}",
                        g.follow_strength, g.explore_rate, g.follow_strength, g.aggression, g.task_bias
                    ))
                    .small(),
                );
                ui.add_space(8.0);
                ui.label(format!("collected: {:.0}", self.sim.collected));
                ui.label("Click on canvas to deposit Trail pheromone.");
            });

        egui::containers::CentralPanel::default().show(ui, |ui| {
            let available = ui.available_size();
            let (gw, gh) = (self.sim.world.width as f32, self.sim.world.height as f32);
            let aspect = gw / gh;
            let mut size = available;
            if size.x / size.y > aspect {
                size.x = size.y * aspect;
            } else {
                size.y = size.x / aspect;
            }
            let (rect, response) = ui.allocate_exact_size(size, egui::Sense::click_and_drag());
            let painter = ui.painter_at(rect);

            if let Some(tex) = &self.tex {
                painter.image(
                    tex.id(),
                    rect,
                    egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                    egui::Color32::WHITE,
                );
            }

            // walls
            for w in &self.sim.world.walls {
                let r = egui::Rect::from_min_max(
                    to_screen(rect, w.x0, w.y0, gw, gh),
                    to_screen(rect, w.x1, w.y1, gw, gh),
                );
                painter.rect_filled(r, 0.0, egui::Color32::from_rgb(70, 70, 80));
            }

            let nest = &self.sim.world.nest;
            painter.circle_filled(
                to_screen(rect, nest.x, nest.y, gw, gh),
                6.0,
                egui::Color32::from_rgb(180, 120, 40),
            );

            for f in &self.sim.world.food {
                painter.circle_stroke(
                    to_screen(rect, f.pos.x, f.pos.y, gw, gh),
                    f.radius,
                    egui::Stroke::new(2.0_f32, egui::Color32::from_rgb(80, 200, 80)),
                );
            }

            for e in &self.sim.world.enemies {
                painter.circle_filled(
                    to_screen(rect, e.pos.x, e.pos.y, gw, gh),
                    3.0,
                    egui::Color32::from_rgb(220, 40, 40),
                );
            }

            for a in &self.sim.ants {
                let p = to_screen(rect, a.pos.x, a.pos.y, gw, gh);
                // T19.4: ant color by overlay mode (default carrying).
                let col = match self.overlay.as_str() {
                    "dopamine" => {
                        let dm = crate::genome::DOPAMINE_MAX.max(1e-9);
                        let r = (a.dopamine_reward / dm).clamp(0.0, 1.0);
                        let pu = (a.dopamine_punish / dm).clamp(0.0, 1.0);
                        egui::Color32::from_rgb(
                            (120.0 + 135.0 * pu) as u8,
                            (120.0 + 135.0 * r) as u8,
                            120,
                        )
                    }
                    _ => {
                        if a.carrying {
                            egui::Color32::from_rgb(255, 200, 0)
                        } else {
                            egui::Color32::from_rgb(240, 240, 240)
                        }
                    }
                };
                painter.circle_filled(p, 1.5, col);
            }

            if response.clicked() {
                if let Some(pos) = response.interact_pointer_pos() {
                    let (gx, gy) = from_screen(rect, pos, gw, gh);
                    let ix = gx as i32;
                    let iy = gy as i32;
                    match self.place_mode.as_str() {
                        "food" => {
                            self.sim.world.food.push(crate::world::FoodSource {
                                pos: crate::world::Vec2::new(gx, gy),
                                radius: 4.0,
                                amount: 1_000_000.0,
                            });
                        }
                        "enemy" => {
                            self.sim.world.enemies.push(crate::world::Enemy {
                                pos: crate::world::Vec2::new(gx, gy),
                                radius: 2.0,
                                health: 10.0,
                            });
                        }
                        "wall" => {
                            self.sim.world.walls.push(crate::world::Wall {
                                x0: gx - 2.0,
                                y0: gy - 2.0,
                                x1: gx + 2.0,
                                y1: gy + 2.0,
                            });
                        }
                        "erase" => {
                            // remove nearest food/enemy/wall to click
                            
                            if let Some(i) = self.sim.world.food.iter().enumerate()
                                .min_by(|a, b| {
                                    let da = (a.1.pos.x - gx).hypot(a.1.pos.y - gy);
                                    let db = (b.1.pos.x - gx).hypot(b.1.pos.y - gy);
                                    da.partial_cmp(&db).unwrap()
                                }).map(|(i, _)| i)
                            {
                                if (self.sim.world.food[i].pos.x - gx).hypot(self.sim.world.food[i].pos.y - gy) < 10.0 {
                                    self.sim.world.food.remove(i);
                                }
                            }
                            self.sim.world.enemies.retain(|e| (e.pos.x - gx).hypot(e.pos.y - gy) > 10.0);
                            self.sim.world.walls.retain(|w| {
                                let cx = (w.x0 + w.x1) * 0.5;
                                let cy = (w.y0 + w.y1) * 0.5;
                                (cx - gx).hypot(cy - gy) > 10.0
                            });
                        }
                        _ => {
                            // default: deposit Trail pheromone
                            for dy in -2..=2 {
                                for dx in -2..=2 {
                                    let d = ((dx * dx + dy * dy) as f32).sqrt();
                                    let amt = 2.0 * (1.0 - d / 3.0).max(0.0);
                                    self.sim.world.deposit(Channel::Trail, ix + dx, iy + dy, amt);
                                }
                            }
                        }
                    }
                }
            }
        });
    }
}

fn to_screen(rect: egui::Rect, gx: f32, gy: f32, gw: f32, gh: f32) -> egui::Pos2 {
    let x = rect.left() + (gx / gw) * rect.width();
    let y = rect.top() + (gy / gh) * rect.height();
    egui::pos2(x, y)
}

fn from_screen(rect: egui::Rect, p: egui::Pos2, gw: f32, gh: f32) -> (f32, f32) {
    let gx = ((p.x - rect.left()) / rect.width() * gw).clamp(0.0, gw - 1.0);
    let gy = ((p.y - rect.top()) / rect.height() * gh).clamp(0.0, gh - 1.0);
    (gx, gy)
}
