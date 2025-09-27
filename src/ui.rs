use crate::SampleDuckApp;
use egui::{Color32, RichText, Sense, Shape, Stroke, Ui, pos2, vec2};
use egui_extras::{Column, TableBuilder};

impl eframe::App for SampleDuckApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            //keymap
            if ui.input(|i| i.key_pressed(egui::Key::J) || i.key_pressed(egui::Key::ArrowDown)) {
                self.select_next_sample();
            }
            if ui.input(|i| i.key_pressed(egui::Key::K) || i.key_pressed(egui::Key::ArrowUp)) {
                self.select_prev_sample();
            }
            if ui.input(|i| i.key_pressed(egui::Key::Space)) {
                self.audio_player.toggle_play_state();
            }

            ui.heading("Sample Duck");
            ui.separator();

            ui.vertical(|ui| {
                self.details_view(ui);
                self.app_ui(ui);
                egui::ScrollArea::both().show(ui, |ui| {
                    self.sample_list(ui);
                });
            });
        });
    }
}

impl SampleDuckApp {
    fn app_ui(&mut self, ui: &mut Ui) {
        if ui.button("Import Dir").clicked() {
            if let Some(dir) = rfd::FileDialog::new().pick_folder() {
                self.import_samples_from_dir(&dir);
                self.query_samples();
            }
        }
    }

    fn sample_list(&mut self, ui: &mut Ui) {
        let available_height = ui.available_height();

        let mut table = TableBuilder::new(ui)
            .striped(true)
            .resizable(true)
            .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
            .column(Column::auto())
            .column(Column::auto())
            .column(Column::auto())
            .column(Column::auto())
            .column(Column::auto())
            .column(Column::auto())
            .min_scrolled_height(0.0)
            .max_scroll_height(available_height);

        table = table.sense(egui::Sense::click());

        table
            .header(20.0, |mut header| {
                header.col(|ui| {
                    ui.strong("Name");
                });
                header.col(|ui| {
                    ui.strong("Path");
                });
                header.col(|ui| {
                    ui.strong("Rating");
                });
                header.col(|ui| {
                    ui.strong("Format");
                });
                header.col(|ui| {
                    ui.strong("Sample Rate");
                });
                header.col(|ui| {
                    ui.strong("Size");
                });
            })
            .body(|mut body| {
                for (idx, sample) in self.samples.clone().iter().enumerate() {
                    let row_height = 18.0;
                    body.row(row_height, |mut row| {
                        if let Some(selected_sample) = &self.selected_sample {
                            row.set_selected(selected_sample.id == sample.id);
                        }
                        row.col(|ui| {
                            ui.label(sample.name.clone());
                        });
                        row.col(|ui| {
                            ui.label(sample.path.clone());
                        });
                        row.col(|ui| {
                            self.star_rating(ui, sample.id, sample.rating);
                        });
                        row.col(|ui| {
                            ui.label(sample.format.to_string());
                        });
                        row.col(|ui| {
                            ui.label(sample.sample_rate.to_string());
                        });
                        row.col(|ui| {
                            ui.label(sample.size.to_string());
                        });

                        self.click_sample(idx, &row.response());
                    });
                }
            });
    }

    fn star_rating(&mut self, ui: &mut egui::Ui, sample_id: isize, rating: u8) {
        ui.horizontal(|ui| {
            for i in 1..=5 {
                let is_filled = i <= rating;

                let star_text = if is_filled { "★" } else { "☆" };
                let color = if is_filled {
                    Color32::from_rgb(255, 215, 0)
                } else {
                    Color32::GRAY
                };

                let response = ui.add(
                    egui::Label::new(RichText::new(star_text).size(16.0).color(color))
                        .sense(Sense::click()),
                );

                if response.hovered() {
                    ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                }

                if response.clicked() {
                    self.db.update_rating(sample_id, i);
                    //todo: shouldn't always requery samples on updates
                    self.query_samples();
                }
            }

            // Clear button
            if ui.small_button("Clear").clicked() {
                self.db.update_rating(sample_id, 0);
                //todo: shouldn't always requery samples on updates
                self.query_samples();
            }
        });
    }

    fn details_view(&mut self, ui: &mut Ui) {
        if let Some(selected_sample) = &self.selected_sample {
            ui.ctx()
                .request_repaint_after(std::time::Duration::from_millis(16));

            ui.label(selected_sample.name.clone());
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    ui.label(selected_sample.notes.clone());
                });

                let (rect, response) = ui.allocate_exact_size(
                    vec2(ui.available_width(), 90.0),
                    Sense::click_and_drag(),
                );

                // Handle clicks on waveform
                if response.clicked() || response.dragged() {
                    if let Some(pos) = response.hover_pos() {
                        println!("pos: {}", pos.x);
                        println!("rect.width: {}", rect.width());
                        println!("rect.min.x: {}", rect.min.x);
                        let relative_x = (pos.x - rect.min.x) / rect.width();
                        println!("relative_x: {}", relative_x);
                        self.audio_player.seek_to_position_percentage(relative_x);
                    }
                }
                let available_width = rect.width();
                let available_height = rect.height();

                let to_screen = |x: f32, y: f32| {
                    let px = rect.min.x + x * available_width;
                    let py = rect.center().y - y * (available_height / 2.0);
                    pos2(px, py)
                };

                // Draw waveform
                let points: Vec<Shape> = self
                    .audio_player
                    .peak_samples
                    .iter()
                    .enumerate()
                    .map(|(i, &(min, max))| {
                        let x = i as f32 / self.audio_player.peak_samples.len() as f32;
                        let min_pos = to_screen(x, min);
                        let max_pos = to_screen(x, max);

                        // Different color before/after playhead
                        let color = if x <= self.audio_player.get_position_percentage() {
                            Color32::from_rgb(100, 200, 255) // Bright blue for played part
                        } else {
                            Color32::WHITE // White for unplayed part
                        };

                        Shape::line_segment([min_pos, max_pos], Stroke::new(1.0, color))
                    })
                    .collect();

                ui.painter().extend(points);

                // Draw position marker
                let playhead_x =
                    rect.min.x + (self.audio_player.get_position_percentage() * available_width);
                let playhead_top = rect.min.y;
                let playhead_bottom = rect.max.y;
                ui.painter().line_segment(
                    [
                        pos2(playhead_x, playhead_top),
                        pos2(playhead_x, playhead_bottom),
                    ],
                    Stroke::new(2.0, Color32::from_rgb(255, 100, 100)), // Red playhead
                );
            });
        }
    }

    fn select_sample(&mut self, sample_idx: usize) {
        if self.samples.len() > sample_idx {
            self.selected_sample_idx = Option::Some(sample_idx);
            self.selected_sample = Option::Some(self.samples[sample_idx].clone());
            if let Some(selected_sample) = &self.selected_sample {
                match self.audio_player.load(&selected_sample.path) {
                    Ok(_) => {
                        self.audio_player.play();
                    }
                    Err(error) => {
                        println!("Error: {}", error);
                    }
                }
            }
        }
    }

    fn click_sample(&mut self, sample_idx: usize, row_response: &egui::Response) {
        if row_response.clicked() {
            self.select_sample(sample_idx);
        }
    }

    fn select_next_sample(&mut self) {
        let next_sample_idx = match self.selected_sample_idx {
            Some(selected_sample_idx) => selected_sample_idx + 1,
            None => 0,
        };

        self.select_sample(next_sample_idx);
    }

    fn select_prev_sample(&mut self) {
        let prev_sample_idx = match self.selected_sample_idx {
            Some(selected_sample_idx) if selected_sample_idx > 0 => selected_sample_idx - 1,
            _ => 0,
        };
        self.select_sample(prev_sample_idx);
    }
}
