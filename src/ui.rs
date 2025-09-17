use crate::SampleDuckApp;
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

            ui.heading("Sample Manager");
            ui.separator();

            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    ui.set_min_height(500.0);
                    ui.set_max_width(ui.available_width() - 270.0); // Reserve space for right panel + margin
                    egui::ScrollArea::both().show(ui, |ui| {
                        ui.set_width(ui.available_width() * 0.5);
                        self.sample_list(ui);
                    });
                });

                ui.vertical(|ui| {
                    let size = egui::Vec2::new(250.0, ui.available_height());
                    ui.allocate_ui(size, |ui| {
                        ui.heading("Sample");
                        ui.label(self.selected_sample.name.clone());
                    });
                });
            });
        });
    }
}

impl SampleDuckApp {
    fn sample_list(&mut self, ui: &mut egui::Ui) {
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
                    ui.strong("ID");
                });
                header.col(|ui| {
                    ui.strong("Name");
                });
                header.col(|ui| {
                    ui.strong("Path");
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
                        row.set_selected(self.selected_sample.id == sample.id);
                        row.col(|ui| {
                            ui.label(sample.id.to_string());
                        });
                        row.col(|ui| {
                            ui.label(sample.name.clone());
                        });
                        row.col(|ui| {
                            ui.label(sample.path.clone());
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

    fn select_sample(&mut self, sample_idx: usize) {
        if self.samples.len() > sample_idx {
            self.selected_sample_idx = sample_idx;
            self.selected_sample = self.samples[sample_idx].clone();
            match self.audio_player.load(&self.selected_sample.path) {
                Ok(_) => {
                    self.audio_player.play();
                }
                Err(error) => {
                    println!("Error: {}", error);
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
        self.select_sample(self.selected_sample_idx + 1);
    }

    fn select_prev_sample(&mut self) {
        if self.selected_sample_idx > 0 {
            self.select_sample(self.selected_sample_idx - 1);
        }
    }
}
