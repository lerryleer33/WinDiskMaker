//! Screen composition for WinDisk Maker.

use eframe::egui::{
    self, Align, Align2, Color32, Id, Layout, Margin, Order, RichText, ScrollArea, Sense, Stroke,
    Vec2,
};

use crate::theme::{body, display, mono, Palette};
use crate::widgets;
use crate::{DiskInfo, StatusLevel, WinDiskApp};

const STEP_LABELS: [&str; 6] = ["Format", "ISO", "Salin", "WIM", "Patch", "Selesai"];

pub fn app(app: &mut WinDiskApp, ctx: &egui::Context) {
    if app.show_confirm_dialog && ctx.input(|input| input.key_pressed(egui::Key::Escape)) {
        app.show_confirm_dialog = false;
    }

    let p = Palette::for_mode(app.dark);
    header(app, ctx, &p);
    footer(app, ctx, &p);
    central(app, ctx, &p);
    modal(app, ctx, &p);
}

fn header(app: &WinDiskApp, ctx: &egui::Context, p: &Palette) {
    egui::TopBottomPanel::top("wdm_header")
        .exact_height(66.0)
        .frame(egui::Frame::none().fill(p.window))
        .show(ctx, |ui| {
            ui.add_space(14.0);
            ui.horizontal(|ui| {
                ui.add_space(20.0);
                widgets::app_mark(ui, p);
                ui.add_space(10.0);
                ui.vertical(|ui| {
                    ui.spacing_mut().item_spacing.y = 1.0;
                    ui.label(
                        RichText::new("WinDisk Maker")
                            .font(display(19.0))
                            .color(p.text),
                    );
                    ui.label(
                        RichText::new("Pembuat USB bootable Windows untuk macOS")
                            .font(body(11.5))
                            .color(p.text_secondary),
                    );
                });

                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ui.add_space(20.0);
                    let (color, label) = header_status(app, p);
                    widgets::status(ui, p, color, label);
                    ui.add_space(10.0);
                    ui.label(
                        RichText::new("v1.2")
                            .font(body(11.0))
                            .color(p.text_tertiary),
                    );
                });
            });
        });
}

fn header_status(app: &WinDiskApp, p: &Palette) -> (Color32, &'static str) {
    if app.is_creating {
        (p.accent, "Memproses")
    } else if app.status_level == StatusLevel::Error {
        (p.danger, "Perlu perhatian")
    } else if !app.iso_path.is_empty() && !app.selected_disk.is_empty() {
        (p.success, "Siap")
    } else {
        (p.text_tertiary, "Menunggu")
    }
}

fn footer(app: &mut WinDiskApp, ctx: &egui::Context, p: &Palette) {
    egui::TopBottomPanel::bottom("wdm_footer")
        .exact_height(54.0)
        .frame(egui::Frame::none().fill(p.window))
        .show(ctx, |ui| {
            let rect = ui.max_rect();
            ui.painter().hline(
                rect.left()..=rect.right(),
                rect.top() + 0.5,
                Stroke::new(1.0, p.separator),
            );
            ui.add_space(11.0);

            ui.horizontal(|ui| {
                ui.add_space(20.0);
                let status_color = if app.is_creating {
                    p.text_secondary
                } else {
                    match app.status_level {
                        StatusLevel::Info => p.text_secondary,
                        StatusLevel::Success => p.success,
                        StatusLevel::Error => p.danger,
                    }
                };
                let status_width = (ui.available_width() - 350.0).max(160.0);
                ui.allocate_ui_with_layout(
                    Vec2::new(status_width, 20.0),
                    Layout::left_to_right(Align::Center),
                    |ui| {
                        ui.add(
                            egui::Label::new(
                                RichText::new(&app.status)
                                    .font(body(12.0))
                                    .color(status_color),
                            )
                            .truncate(true),
                        );
                    },
                );

                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ui.add_space(20.0);
                    if app.is_creating {
                        ui.label(
                            RichText::new("Jangan cabut drive USB")
                                .font(body(11.5))
                                .color(p.warning),
                        );
                    } else {
                        let ready = !app.iso_path.is_empty() && !app.selected_disk.is_empty();
                        if widgets::primary_button(ui, p, "Buat USB Bootable", ready).clicked() {
                            app.show_confirm_dialog = true;
                        }
                        ui.add_space(8.0);
                        let label = if app.show_log_panel {
                            "Sembunyikan Log"
                        } else {
                            "Lihat Log"
                        };
                        if widgets::secondary_button(ui, p, label, true).clicked() {
                            app.show_log_panel = !app.show_log_panel;
                        }
                    }
                });
            });
        });
}

fn central(app: &mut WinDiskApp, ctx: &egui::Context, p: &Palette) {
    egui::CentralPanel::default()
        .frame(egui::Frame::none().fill(p.window).inner_margin(Margin {
            left: 20.0,
            right: 20.0,
            top: 4.0,
            bottom: 10.0,
        }))
        .show(ctx, |ui| {
            ScrollArea::vertical()
                .id_source("wdm_main_scroll")
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    if app.is_creating {
                        creation_view(app, ui, p);
                    } else {
                        form_view(app, ui, p);
                    }
                });
        });
}

fn form_view(app: &mut WinDiskApp, ui: &mut egui::Ui, p: &Palette) {
    widgets::section_label(ui, p, "Sumber instalasi");
    widgets::group(ui, p, |ui| {
        egui::Frame::none()
            .inner_margin(Margin {
                left: 16.0,
                right: 16.0,
                top: 10.0,
                bottom: 10.0,
            })
            .show(ui, |ui| {
                ui.set_min_height(44.0);
                iso_row(app, ui, p);
            });
    });

    widgets::section_label(ui, p, "Tujuan");
    widgets::group(ui, p, |ui| {
        if app.available_disks.is_empty() {
            empty_drives(ui, p);
        } else {
            let mut clicked: Option<String> = None;
            for (index, disk) in app.available_disks.iter().enumerate() {
                if index > 0 {
                    widgets::separator(ui, p);
                }
                let (title, meta) = drive_text(disk);
                let (tag, tag_fg, tag_bg) = if disk.is_partition {
                    ("Aman", p.success, p.success_soft)
                } else {
                    ("Hapus semua", p.danger, p.danger_soft)
                };
                let response = widgets::drive_row(
                    ui,
                    p,
                    title,
                    &meta,
                    tag,
                    tag_fg,
                    tag_bg,
                    app.selected_disk == disk.device,
                );
                if response.clicked() {
                    clicked = Some(disk.device.clone());
                }
            }
            if let Some(device) = clicked {
                app.selected_disk = device;
                app.status_level = StatusLevel::Info;
            }

            if let Some(info) = app
                .available_disks
                .iter()
                .find(|disk| disk.device == app.selected_disk)
            {
                widgets::separator(ui, p);
                target_details(ui, p, info);
            }
        }
    });

    widgets::section_label(ui, p, "Opsi");
    widgets::group(ui, p, |ui| {
        widgets::option_row(
            ui,
            p,
            "Driver NVMe / Intel VMD",
            "iaStorVD dimuat otomatis agar SSD NVMe terdeteksi.",
            &mut app.patch_vmd,
        );
        widgets::separator(ui, p);
        widgets::option_row(
            ui,
            p,
            "Bypass persyaratan Windows 11",
            "TPM 2.0, Secure Boot, RAM, dan CPU check dilewati.",
            &mut app.bypass_tpm,
        );
    });

    if app.show_log_panel {
        ui.add_space(20.0);
        log_group(app, ui, p);
    }

    ui.add_space(8.0);
}

fn drive_text(disk: &DiskInfo) -> (&str, String) {
    if disk.is_partition {
        let meta = format!("{} · {} · {}", disk.media_name, disk.size, disk.device);
        (&disk.volume_name, meta)
    } else {
        let meta = format!("{} · {} · {}", disk.volume_name, disk.size, disk.device);
        (&disk.media_name, meta)
    }
}

fn iso_row(app: &mut WinDiskApp, ui: &mut egui::Ui, p: &Palette) {
    ui.horizontal(|ui| {
        widgets::file_icon(
            ui,
            if app.iso_path.is_empty() {
                p.text_tertiary
            } else {
                p.accent
            },
        );
        ui.add_space(10.0);

        let button_width = 92.0;
        let text_width = (ui.available_width() - button_width - 12.0).max(120.0);
        ui.allocate_ui_with_layout(
            Vec2::new(text_width, 40.0),
            Layout::top_down(Align::Min),
            |ui| {
                ui.spacing_mut().item_spacing.y = 1.0;
                if app.iso_path.is_empty() {
                    ui.add(
                        egui::Label::new(
                            RichText::new("Belum ada ISO dipilih")
                                .font(display(13.0))
                                .color(p.text),
                        )
                        .truncate(true),
                    );
                    ui.add_space(2.0);
                    ui.add(
                        egui::Label::new(
                            RichText::new("Format .iso · Windows 10, 11, atau Server")
                                .font(body(11.5))
                                .color(p.text_secondary),
                        )
                        .truncate(true),
                    );
                } else {
                    ui.add(
                        egui::Label::new(
                            RichText::new(&app.iso_name)
                                .font(display(13.0))
                                .color(p.text),
                        )
                        .truncate(true),
                    );
                    ui.add_space(2.0);
                    let meta = format!("{} · {}", app.iso_size, app.iso_path);
                    ui.add(
                        egui::Label::new(
                            RichText::new(meta).font(body(11.5)).color(p.text_secondary),
                        )
                        .truncate(true),
                    );
                }
            },
        );

        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            let label = if app.iso_path.is_empty() {
                "Pilih ISO…"
            } else {
                "Ganti"
            };
            if widgets::secondary_button(ui, p, label, true).clicked() {
                app.pick_iso();
            }
        });
    });
}

fn empty_drives(ui: &mut egui::Ui, p: &Palette) {
    egui::Frame::none()
        .inner_margin(Margin {
            left: 16.0,
            right: 16.0,
            top: 14.0,
            bottom: 14.0,
        })
        .show(ui, |ui| {
            ui.spacing_mut().item_spacing.y = 2.0;
            ui.label(
                RichText::new("Tidak ada drive eksternal terdeteksi")
                    .font(display(13.0))
                    .color(p.text),
            );
            ui.label(
                RichText::new("Colokkan flashdisk atau SSD. Daftar diperbarui otomatis.")
                    .font(body(11.5))
                    .color(p.text_secondary),
            );
        });
}

fn target_details(ui: &mut egui::Ui, p: &Palette, info: &DiskInfo) {
    egui::Frame::none()
        .inner_margin(Margin {
            left: 16.0,
            right: 16.0,
            top: 9.0,
            bottom: 12.0,
        })
        .show(ui, |ui| {
            let rows: [(&str, &str, Color32); 3] = [
                (
                    "Mode",
                    if info.is_partition {
                        "Hanya partisi ini"
                    } else {
                        "Seluruh disk"
                    },
                    if info.is_partition {
                        p.success
                    } else {
                        p.danger
                    },
                ),
                ("Sistem berkas", "FAT32 · WIM dipecah otomatis", p.text),
                ("Label", "WIN11", p.text),
            ];

            for (index, (label, value, color)) in rows.iter().enumerate() {
                if index > 0 {
                    ui.add_space(5.0);
                }
                ui.horizontal(|ui| {
                    let (label_rect, _) =
                        ui.allocate_exact_size(Vec2::new(88.0, 18.0), Sense::hover());
                    ui.painter().with_clip_rect(label_rect).text(
                        label_rect.left_center(),
                        Align2::LEFT_CENTER,
                        *label,
                        body(11.5),
                        p.text_secondary,
                    );
                    ui.label(RichText::new(*value).font(body(12.0)).color(*color));
                });
            }
        });
}

fn creation_view(app: &mut WinDiskApp, ui: &mut egui::Ui, p: &Palette) {
    ui.add_space(8.0);
    ui.label(
        RichText::new("Membuat media bootable")
            .font(display(20.0))
            .color(p.text),
    );
    ui.add_space(4.0);
    ui.label(
        RichText::new(&app.status)
            .font(body(12.5))
            .color(p.text_secondary),
    );
    ui.add_space(18.0);

    widgets::progress_bar(ui, p, progress_fraction(app.current_step));
    ui.add_space(10.0);
    widgets::stepper(ui, p, app.current_step, &STEP_LABELS);
    ui.add_space(18.0);

    ui.label(
        RichText::new(format!("{} → {}", app.iso_name, target_summary(app)))
            .font(body(11.5))
            .color(p.text_secondary),
    );
    ui.add_space(8.0);
    ui.horizontal(|ui| {
        let (dot, _) = ui.allocate_exact_size(Vec2::splat(12.0), Sense::hover());
        ui.painter().circle_filled(dot.center(), 3.0, p.warning);
        ui.label(
            RichText::new("Jangan cabut drive selama proses berlangsung.")
                .font(body(11.5))
                .color(p.warning),
        );
    });
    ui.add_space(22.0);
    log_group(app, ui, p);
    ui.add_space(8.0);
}

fn progress_fraction(step: usize) -> f32 {
    match step {
        1 => 0.15,
        2 => 0.25,
        3 => 0.60,
        4 => 0.80,
        5 => 0.92,
        6 => 1.0,
        _ => 0.05,
    }
}

fn target_summary(app: &WinDiskApp) -> String {
    match app
        .available_disks
        .iter()
        .find(|disk| disk.device == app.selected_disk)
    {
        Some(info) if info.is_partition => {
            format!("{} · {} · {}", info.volume_name, info.size, info.device)
        }
        Some(info) => format!("{} · {} · {}", info.media_name, info.size, info.device),
        None => app.selected_disk.clone(),
    }
}

fn log_group(app: &WinDiskApp, ui: &mut egui::Ui, p: &Palette) {
    widgets::group(ui, p, |ui| {
        egui::Frame::none()
            .inner_margin(Margin {
                left: 16.0,
                right: 16.0,
                top: 10.0,
                bottom: 10.0,
            })
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new("Log sistem")
                            .font(display(13.0))
                            .color(p.text),
                    );
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if widgets::plain_button(ui, p, "Salin", true).clicked() {
                            ui.ctx().copy_text(app.log.clone());
                        }
                    });
                });
            });

        widgets::separator(ui, p);

        egui::Frame::none()
            .inner_margin(Margin {
                left: 12.0,
                right: 12.0,
                top: 0.0,
                bottom: 12.0,
            })
            .show(ui, |ui| {
                egui::Frame::none()
                    .fill(p.inset)
                    .rounding(8.0)
                    .inner_margin(Margin::same(10.0))
                    .show(ui, |ui| {
                        ScrollArea::both()
                            .id_source("wdm_log_scroll")
                            .max_height(190.0)
                            .auto_shrink([false, false])
                            .stick_to_bottom(true)
                            .show(ui, |ui| {
                                if app.log.is_empty() {
                                    ui.label(
                                        RichText::new("Belum ada aktivitas.")
                                            .font(body(11.5))
                                            .color(p.text_tertiary),
                                    );
                                } else {
                                    ui.add(
                                        egui::Label::new(
                                            RichText::new(&app.log)
                                                .font(mono(11.5))
                                                .color(p.text_secondary),
                                        )
                                        .wrap(false),
                                    );
                                }
                            });
                    });
            });
    });
}

fn modal(app: &mut WinDiskApp, ctx: &egui::Context, p: &Palette) {
    if !app.show_confirm_dialog {
        return;
    }

    let screen = ctx.screen_rect();
    egui::Area::new(Id::new("wdm_scrim"))
        .order(Order::Middle)
        .fixed_pos(screen.min)
        .show(ctx, |ui| {
            let (rect, _) = ui.allocate_exact_size(screen.size(), Sense::click());
            ui.painter().rect_filled(rect, 0.0, p.scrim);
        });

    let info = app
        .available_disks
        .iter()
        .find(|disk| disk.device == app.selected_disk)
        .cloned();
    let is_partition = info.as_ref().map(|disk| disk.is_partition).unwrap_or(false);
    let title = if is_partition {
        "Format partisi ini?"
    } else {
        "Format seluruh disk?"
    };
    let description = if is_partition {
        "Hanya partisi target yang akan diformat menjadi FAT32. Partisi lain pada drive ini tidak disentuh."
    } else {
        "Semua partisi pada drive ini akan diformat ulang. Data di dalamnya tidak dapat dipulihkan."
    };
    let target_name = info
        .as_ref()
        .map(|disk| {
            if disk.is_partition {
                disk.volume_name.clone()
            } else {
                disk.media_name.clone()
            }
        })
        .unwrap_or_else(|| app.selected_disk.clone());
    let target_meta = info
        .as_ref()
        .map(|disk| format!("{} · {}", disk.size, disk.protocol))
        .unwrap_or_default();
    let target_device = app.selected_disk.clone();

    egui::Area::new(Id::new("wdm_modal"))
        .order(Order::Foreground)
        .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
        .show(ctx, |ui| {
            egui::Frame::none()
                .fill(p.surface)
                .stroke(Stroke::new(1.0, p.group_stroke))
                .rounding(12.0)
                .shadow(egui::epaint::Shadow {
                    offset: Vec2::new(0.0, 12.0),
                    blur: 32.0,
                    spread: 0.0,
                    color: p.shadow,
                })
                .inner_margin(Margin::same(20.0))
                .show(ui, |ui| {
                    ui.set_width(400.0);

                    ui.horizontal(|ui| {
                        widgets::alert_badge(
                            ui,
                            if is_partition { p.accent } else { p.danger },
                            if is_partition {
                                p.accent_soft
                            } else {
                                p.danger_soft
                            },
                        );
                        ui.add_space(10.0);
                        ui.label(RichText::new(title).font(display(15.0)).color(p.text));
                    });

                    ui.add_space(10.0);
                    ui.label(
                        RichText::new(description)
                            .font(body(12.5))
                            .color(p.text_secondary),
                    );
                    ui.add_space(12.0);

                    egui::Frame::none()
                        .fill(p.inset)
                        .rounding(8.0)
                        .inner_margin(Margin::same(12.0))
                        .show(ui, |ui| {
                            ui.set_width(ui.available_width());
                            ui.spacing_mut().item_spacing.y = 2.0;
                            ui.add(
                                egui::Label::new(
                                    RichText::new(&target_name)
                                        .font(display(13.0))
                                        .color(p.text),
                                )
                                .truncate(true),
                            );
                            ui.add(
                                egui::Label::new(
                                    RichText::new(&target_meta)
                                        .font(body(11.5))
                                        .color(p.text_secondary),
                                )
                                .truncate(true),
                            );
                            ui.label(
                                RichText::new(&target_device)
                                    .font(mono(11.0))
                                    .color(p.text_tertiary),
                            );
                        });

                    if !is_partition {
                        ui.add_space(10.0);
                        ui.label(
                            RichText::new(
                                "Semua partisi akan dihapus permanen. Pastikan tidak ada data penting.",
                            )
                            .font(body(11.5))
                            .color(p.danger),
                        );
                    }

                    ui.add_space(18.0);
                    ui.allocate_ui_with_layout(
                        Vec2::new(ui.available_width(), 32.0),
                        Layout::right_to_left(Align::Center),
                        |ui| {
                            let confirm_label = if is_partition {
                                "Format Partisi"
                            } else {
                                "Hapus & Format"
                            };
                            let confirmed = if is_partition {
                                widgets::primary_button(ui, p, confirm_label, true).clicked()
                            } else {
                                widgets::danger_button(ui, p, confirm_label).clicked()
                            };
                            if confirmed {
                                app.show_confirm_dialog = false;
                                app.start_worker_process();
                            }
                            ui.add_space(8.0);
                            if widgets::secondary_button(ui, p, "Batal", true).clicked() {
                                app.show_confirm_dialog = false;
                            }
                        },
                    );
                });
        });
}
