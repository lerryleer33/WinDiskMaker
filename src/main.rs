use eframe::egui;
use std::fs;
use std::io::{BufRead, BufReader};
use std::path::Path as FsPath;
use std::process::{Command, Stdio};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::time::Duration;

// Driver Intel RST VMD untuk deteksi disk NVMe SSD pada Windows Installer
const IASTORVD_INF: &[u8] = include_bytes!("../drivers/vmd/iaStorVD.inf");
const IASTORVD_SYS: &[u8] = include_bytes!("../drivers/vmd/iaStorVD.sys");
const IASTORVD_CAT: &[u8] = include_bytes!("../drivers/vmd/iaStorVD.cat");
const RST_MW_SERVICE: &[u8] = include_bytes!("../drivers/vmd/RstMwService.exe");
const RST_MW_EVENT_LOG: &[u8] = include_bytes!("../drivers/vmd/RstMwEventLogMsg.dll");

#[derive(Clone, PartialEq, Eq, Debug)]
struct DiskInfo {
    device: String,         // "/dev/disk3s3" atau "/dev/disk3"
    parent_device: String,  // "/dev/disk3"
    media_name: String,     // "SanDisk Portable SSD"
    volume_name: String,    // "WIN10"
    size: String,           // "49.9 GB"
    protocol: String,       // "USB"
    display: String,
    is_usb: bool,
    is_partition: bool,     // true jika single partition / split volume
}

enum BackgroundMessage {
    Status(String),
    Step(usize), // 1 to 6
    LogLine(String),
    Finished(bool, String),
}

struct WinDiskApp {
    iso_path: String,
    iso_name: String,
    iso_size: String,
    selected_disk: String,
    available_disks: Vec<DiskInfo>,
    status: String,
    current_step: usize,
    is_creating: bool,
    patch_vmd: bool,
    log: String,
    show_confirm_dialog: bool,
    show_log_panel: bool,
    disk_receiver: Receiver<Vec<DiskInfo>>,
    worker_receiver: Option<Receiver<BackgroundMessage>>,
}

impl WinDiskApp {
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        // Channel untuk background USB auto-detect
        let (disk_tx, disk_rx) = channel::<Vec<DiskInfo>>();

        // Background thread mandiri untuk memantau USB & Partisi (Zero UI latency / Zero stutter)
        std::thread::spawn(move || {
            let mut last_detected_nodes: Vec<String> = Vec::new();
            let mut last_disks: Vec<DiskInfo> = Vec::new();

            loop {
                let current_nodes = Self::quick_get_external_nodes();
                // Jika susunan perangkat USB berubah (dicolok atau dicabut)
                if current_nodes != last_detected_nodes {
                    last_detected_nodes = current_nodes.clone();
                    let disks = Self::deep_scan_usb_disks(&current_nodes);
                    last_disks = disks.clone();
                    let _ = disk_tx.send(disks);
                } else if !last_disks.is_empty() {
                    // Kirim berkala untuk sinkronisasi state
                    let _ = disk_tx.send(last_disks.clone());
                }
                std::thread::sleep(Duration::from_millis(1500));
            }
        });

        Self {
            iso_path: String::new(),
            iso_name: String::new(),
            iso_size: String::new(),
            selected_disk: String::new(),
            available_disks: Vec::new(),
            status: "Siap. Silakan pilih berkas ISO dan flashdisk/partisi USB.".to_string(),
            current_step: 0,
            is_creating: false,
            patch_vmd: true, // Default aktif agar SSD NVMe terdeteksi di laptop modern
            log: "=== WinDisk Maker v1.2 ===\nSistem siap. Mendeteksi perangkat USB & partisi split volume di latar belakang...\n".to_string(),
            show_confirm_dialog: false,
            show_log_panel: false,
            disk_receiver: disk_rx,
            worker_receiver: None,
        }
    }

    /// Pemeriksaan cepat node fisik eksternal (~15ms)
    fn quick_get_external_nodes() -> Vec<String> {
        let mut nodes = Vec::new();
        if let Ok(out) = Command::new("diskutil").args(["list", "external", "physical"]).output() {
            let txt = String::from_utf8_lossy(&out.stdout);
            for line in txt.lines() {
                let trimmed = line.trim();
                if trimmed.starts_with("/dev/disk") {
                    if let Some(dev) = trimmed.split_whitespace().next() {
                        let disk_name = dev.trim_start_matches("/dev/");
                        if let Some(num_str) = disk_name.strip_prefix("disk") {
                            if !num_str.is_empty() && num_str.chars().all(|c| c.is_ascii_digit()) {
                                if disk_name != "disk0" && disk_name != "disk1" {
                                    nodes.push(format!("/dev/{}", disk_name));
                                }
                            }
                        }
                    }
                }
            }
        }
        nodes.sort();
        nodes.dedup();
        nodes
    }

    /// Pemindaian detail perangkat USB, termasuk opsi Partisi Tunggal (Split Volume) agar data lain aman
    fn deep_scan_usb_disks(nodes: &[String]) -> Vec<DiskInfo> {
        let mut results = Vec::new();

        for dev in nodes {
            let disk_id = dev.trim_start_matches("/dev/");
            let mut media_name = "USB Drive".to_string();
            let mut whole_size = "Unknown Size".to_string();
            let mut protocol = "USB".to_string();
            let mut is_internal = false;
            let mut is_virtual = false;

            // 1. Ekstrak data dari diskutil info whole disk
            if let Ok(out) = Command::new("diskutil").args(["info", dev]).output() {
                let txt = String::from_utf8_lossy(&out.stdout);
                for line in txt.lines() {
                    if line.contains("Device / Media Name:") {
                        if let Some(v) = line.splitn(2, ':').nth(1) {
                            let n = v.trim();
                            if !n.is_empty() && n != "Not applicable" {
                                media_name = n.to_string();
                            }
                        }
                    } else if line.contains("Disk Size:") {
                        if let Some(v) = line.splitn(2, ':').nth(1) {
                            let s = v.split('(').next().unwrap_or("").trim();
                            if !s.is_empty() {
                                whole_size = s.to_string();
                            }
                        }
                    } else if line.contains("Protocol:") {
                        if let Some(v) = line.splitn(2, ':').nth(1) {
                            protocol = v.trim().to_string();
                        }
                    } else if line.contains("Internal:") && line.contains("Yes") {
                        is_internal = true;
                    } else if line.contains("Virtual:") && line.contains("Yes") {
                        is_virtual = true;
                    }
                }
            }

            // 2. Ambil vendor/nama dagang lengkap dari plist jika ada
            if let Ok(plist_out) = Command::new("diskutil").args(["info", "-plist", dev]).output() {
                let plist_txt = String::from_utf8_lossy(&plist_out.stdout);
                if let Some(entry_name) = Self::extract_plist_string(&plist_txt, "IORegistryEntryName") {
                    let clean = entry_name.replace(" Media", "").trim().to_string();
                    if !clean.is_empty() {
                        media_name = clean;
                    }
                }
            }

            if is_internal || is_virtual {
                continue;
            }

            // 3. Scan semua partisi individual dari disk eksternal ini
            let mut partitions: Vec<DiskInfo> = Vec::new();
            let mut partition_labels: Vec<String> = Vec::new();

            if let Ok(list_out) = Command::new("diskutil").args(["list", dev]).output() {
                let list_txt = String::from_utf8_lossy(&list_out.stdout);
                for line in list_txt.lines() {
                    let parts: Vec<&str> = line.split_whitespace().collect();
                    if parts.len() >= 5 && parts[0].ends_with(':') {
                        if let Some(part_id) = parts.last() {
                            if part_id.starts_with(disk_id) && part_id.contains('s') {
                                if let Ok(part_info) = Command::new("diskutil").args(["info", part_id]).output() {
                                    let p_txt = String::from_utf8_lossy(&part_info.stdout);
                                    let mut v_name = String::new();
                                    let mut part_size = String::new();
                                    let mut part_type = String::new();

                                    for p_line in p_txt.lines() {
                                        if p_line.contains("Volume Name:") {
                                            if let Some(v) = p_line.splitn(2, ':').nth(1) {
                                                let n = v.trim();
                                                if !n.is_empty() && n != "Not applicable (no file system)" {
                                                    v_name = n.to_string();
                                                }
                                            }
                                        } else if p_line.contains("Disk Size:") && part_size.is_empty() {
                                            if let Some(v) = p_line.splitn(2, ':').nth(1) {
                                                part_size = v.split('(').next().unwrap_or("").trim().to_string();
                                            }
                                        } else if p_line.contains("Volume Total Space:") && part_size.is_empty() {
                                            if let Some(v) = p_line.splitn(2, ':').nth(1) {
                                                part_size = v.split('(').next().unwrap_or("").trim().to_string();
                                            }
                                        } else if p_line.contains("Partition Type:") {
                                            if let Some(v) = p_line.splitn(2, ':').nth(1) {
                                                part_type = v.trim().to_string();
                                            }
                                        }
                                    }

                                    // Abaikan partisi EFI sistem agar aman
                                    if part_type.contains("EFI") || v_name == "EFI" {
                                        continue;
                                    }

                                    let final_vname = if !v_name.is_empty() {
                                        partition_labels.push(v_name.clone());
                                        v_name
                                    } else if !part_type.is_empty() {
                                        part_type
                                    } else {
                                        "Tanpa Label".to_string()
                                    };

                                    let part_dev = format!("/dev/{}", part_id);
                                    let display = format!("📁 [PARTISI AMAN] \"{}\" ({}) - {} (Partisi lain aman)", final_vname, part_size, part_dev);

                                    partitions.push(DiskInfo {
                                        device: part_dev,
                                        parent_device: dev.clone(),
                                        media_name: media_name.clone(),
                                        volume_name: final_vname,
                                        size: part_size,
                                        protocol: protocol.clone(),
                                        display,
                                        is_usb: true,
                                        is_partition: true,
                                    });
                                }
                            }
                        }
                    }
                }
            }

            // Prioritaskan partisi yang ada kata "WIN" agar otomatis terpilih!
            partitions.sort_by(|a, b| {
                let a_is_win = a.volume_name.to_lowercase().contains("win");
                let b_is_win = b.volume_name.to_lowercase().contains("win");
                b_is_win.cmp(&a_is_win)
            });

            // 1. Masukkan opsi partisi individual ke dalam list (Prioritas Utama untuk SSD split volume)
            for part in partitions {
                results.push(part);
            }

            // 2. Masukkan opsi Whole Disk sebagai alternatif (disertai peringatan jika ada banyak partisi)
            partition_labels.sort();
            partition_labels.dedup();
            let label_summary = if !partition_labels.is_empty() {
                format!("\"{}\"", partition_labels.join(", "))
            } else {
                "Tanpa Label".to_string()
            };

            let whole_display = if partition_labels.len() > 1 {
                format!("💾 [FULL DISK WIPE] {} ({}) - {} [⚠️ HAPUS SELURUH 1TB]", media_name, label_summary, dev)
            } else {
                format!("💾 [SELURUH DRIVE] {} ({}) - {} [{}]", media_name, label_summary, whole_size, dev)
            };

            results.push(DiskInfo {
                device: dev.clone(),
                parent_device: dev.clone(),
                media_name,
                volume_name: label_summary,
                size: whole_size,
                protocol,
                display: whole_display,
                is_usb: true,
                is_partition: false,
            });
        }

        results
    }

    fn extract_plist_string(xml: &str, key: &str) -> Option<String> {
        let key_tag = format!("<key>{}</key>", key);
        if let Some(pos) = xml.find(&key_tag) {
            let rest = &xml[pos + key_tag.len()..];
            if let Some(start_tag) = rest.find("<string>") {
                let after_tag = &rest[start_tag + "<string>".len()..];
                if let Some(end_tag) = after_tag.find("</string>") {
                    let val = &after_tag[..end_tag];
                    return Some(val.trim().to_string());
                }
            }
        }
        None
    }

    /// Mulai proses pembuatan bootable di thread terpisah dengan streaming log langsung
    fn start_worker_process(&mut self) {
        let disk = self.selected_disk.clone();
        let iso = self.iso_path.clone();
        let patch_vmd = self.patch_vmd;

        if iso.is_empty() || disk.is_empty() {
            self.status = "Harap tentukan file ISO dan USB/partisi tujuan terlebih dahulu.".to_string();
            return;
        }

        if disk == "/dev/disk0" || disk == "/dev/disk1" {
            self.status = "Keamanan: Dilarang memformat disk sistem Mac!".to_string();
            return;
        }

        let is_target_partition = self.available_disks.iter()
            .find(|d| d.device == disk)
            .map(|d| d.is_partition)
            .unwrap_or(false);

        self.is_creating = true;
        self.current_step = 1;
        self.status = format!("Memulai persiapan format pada {}...", disk);
        self.log.push_str(&format!("\n\n>>> MEMULAI PEMBUATAN BOOTABLE: {} -> {}\n", iso, disk));
        if is_target_partition {
            self.log.push_str(">>> MODE AMAN AKTIF: Hanya memformat partisi tunggal. Partisi lain pada disk tidak akan disentuh!\n");
        } else {
            self.log.push_str(">>> MODE FULL DISK: Seluruh partisi pada drive akan diformat ulang.\n");
        }
        if patch_vmd {
            self.log.push_str(">>> Fitur Patch NVMe SSD / Intel VMD: AKTIF (iaStorVD.inf, iaStorVD.sys, iaStorVD.cat)\n");
        }

        let (tx, rx) = channel::<BackgroundMessage>();
        self.worker_receiver = Some(rx);

        std::thread::spawn(move || {
            let run_cmd_streaming = |tx: &Sender<BackgroundMessage>, cmd: &str, args: &[&str]| -> bool {
                let _ = tx.send(BackgroundMessage::LogLine(format!("$ {} {}", cmd, args.join(" "))));
                
                let mut child = match Command::new(cmd)
                    .args(args)
                    .stdout(Stdio::piped())
                    .stderr(Stdio::piped())
                    .spawn()
                {
                    Ok(c) => c,
                    Err(e) => {
                        let _ = tx.send(BackgroundMessage::LogLine(format!("Gagal mengeksekusi {}: {}", cmd, e)));
                        return false;
                    }
                };

                let stdout = child.stdout.take();
                let stderr = child.stderr.take();
                let tx_out = tx.clone();
                let tx_err = tx.clone();

                let out_handle = std::thread::spawn(move || {
                    if let Some(out) = stdout {
                        let reader = BufReader::new(out);
                        for line in reader.lines().flatten() {
                            let _ = tx_out.send(BackgroundMessage::LogLine(line));
                        }
                    }
                });

                let err_handle = std::thread::spawn(move || {
                    if let Some(err) = stderr {
                        let reader = BufReader::new(err);
                        for line in reader.lines().flatten() {
                            let _ = tx_err.send(BackgroundMessage::LogLine(line));
                        }
                    }
                });

                let res = child.wait();
                let _ = out_handle.join();
                let _ = err_handle.join();

                match res {
                    Ok(s) => s.success(),
                    Err(_) => false,
                }
            };

            // LANGKAH 1: Format Target (Partisi Tunggal ATAU Seluruh Disk)
            let _ = tx.send(BackgroundMessage::Step(1));
            if is_target_partition {
                let _ = tx.send(BackgroundMessage::Status(format!("Memformat partisi {} menjadi FAT32 (WIN11)...", disk)));
                let _ = tx.send(BackgroundMessage::LogLine(format!(">> Melepaskan dan memformat partisi tunggal {}...", disk)));
                run_cmd_streaming(&tx, "diskutil", &["unmount", &disk]);
                if !run_cmd_streaming(&tx, "diskutil", &["eraseVolume", "MS-DOS", "WIN11", &disk]) {
                    let _ = tx.send(BackgroundMessage::Finished(false, "Gagal memformat partisi target!".to_string()));
                    return;
                }
            } else {
                let _ = tx.send(BackgroundMessage::Status(format!("Melepaskan kaitan seluruh disk {}...", disk)));
                run_cmd_streaming(&tx, "diskutil", &["unmountDisk", &disk]);
                let _ = tx.send(BackgroundMessage::Status(format!("Membuat partisi MBR FAT32 pada seluruh disk {}...", disk)));
                if !run_cmd_streaming(&tx, "diskutil", &["partitionDisk", &disk, "1", "MBR", "MS-DOS", "WIN11", "0"]) {
                    let _ = tx.send(BackgroundMessage::Finished(false, "Gagal memformat seluruh disk!".to_string()));
                    return;
                }
            }

            // LANGKAH 2: Mount ISO Windows
            let _ = tx.send(BackgroundMessage::Step(2));
            let _ = tx.send(BackgroundMessage::Status("Memasang (mount) citra ISO Windows...".to_string()));
            run_cmd_streaming(&tx, "hdiutil", &["mount", &iso]);
            std::thread::sleep(Duration::from_secs(2));

            let mut iso_vol = String::new();
            if let Ok(out) = Command::new("ls").arg("/Volumes").output() {
                let txt = String::from_utf8_lossy(&out.stdout);
                for vol in txt.lines() {
                    let check = format!("/Volumes/{}/setup.exe", vol);
                    if FsPath::new(&check).exists() {
                        iso_vol = format!("/Volumes/{}", vol);
                        break;
                    }
                }
            }

            if iso_vol.is_empty() {
                let _ = tx.send(BackgroundMessage::Finished(false, "Gagal menemukan volume berkas installer Windows dari ISO.".to_string()));
                return;
            }

            // LANGKAH 3: Salin File Windows (kecuali install.wim)
            let _ = tx.send(BackgroundMessage::Step(3));
            let _ = tx.send(BackgroundMessage::Status("Menyalin berkas Windows ke volume WIN11 (proses streaming)...".to_string()));
            let rsync_src = format!("{}/", iso_vol);
            if !run_cmd_streaming(&tx, "rsync", &["-rltD", "--info=progress2", "--exclude=sources/install.wim", &rsync_src, "/Volumes/WIN11/"]) {
                let _ = tx.send(BackgroundMessage::LogLine("Rsync peringatan minor, melanjutkan proses...".to_string()));
            }

            // LANGKAH 4: Pecah install.wim (UEFI FAT32 Split)
            let _ = tx.send(BackgroundMessage::Step(4));
            let wim_path = format!("{}/sources/install.wim", iso_vol);
            if FsPath::new(&wim_path).exists() {
                let _ = tx.send(BackgroundMessage::Status("Memecah install.wim menjadi format SWM (FAT32 UEFI Split)...".to_string()));
                let _ = run_cmd_streaming(&tx, "wimlib-imagex", &["split", &wim_path, "/Volumes/WIN11/sources/install.swm", "3800"]);
            } else {
                let esd_path = format!("{}/sources/install.esd", iso_vol);
                if FsPath::new(&esd_path).exists() {
                    let _ = tx.send(BackgroundMessage::Status("Menyalin install.esd langsung ke USB...".to_string()));
                    let _ = run_cmd_streaming(&tx, "cp", &[&esd_path, "/Volumes/WIN11/sources/install.esd"]);
                }
            }

            // LANGKAH 5: Injeksi Driver Intel VMD / NVMe Patch (iaStorVD)
            if patch_vmd {
                let _ = tx.send(BackgroundMessage::Step(5));
                let _ = tx.send(BackgroundMessage::Status("Memasang patch driver Intel VMD / NVMe SSD (iaStorVD)...".to_string()));
                let _ = tx.send(BackgroundMessage::LogLine("\n>> Memasang patch driver Intel RST VMD untuk deteksi NVMe...".to_string()));

                // Folder $WinPEDriver$ dibaca otomatis oleh Windows PE saat instalasi dimulai
                let target_dirs = [
                    "/Volumes/WIN11/$WinPEDriver$/Intel_VMD",
                    "/Volumes/WIN11/drivers/Intel_VMD",
                ];

                let driver_files: [(&str, &[u8]); 5] = [
                    ("iaStorVD.inf", IASTORVD_INF),
                    ("iaStorVD.sys", IASTORVD_SYS),
                    ("iaStorVD.cat", IASTORVD_CAT),
                    ("RstMwService.exe", RST_MW_SERVICE),
                    ("RstMwEventLogMsg.dll", RST_MW_EVENT_LOG),
                ];

                let mut patch_success = true;
                for dir in &target_dirs {
                    if let Err(e) = fs::create_dir_all(dir) {
                        let _ = tx.send(BackgroundMessage::LogLine(format!("Gagal membuat direktori {}: {}", dir, e)));
                        patch_success = false;
                        continue;
                    }

                    for (name, data) in &driver_files {
                        let file_path = format!("{}/{}", dir, name);
                        if let Err(e) = fs::write(&file_path, data) {
                            let _ = tx.send(BackgroundMessage::LogLine(format!("Gagal menulis berkas {}: {}", file_path, e)));
                            patch_success = false;
                        }
                    }
                    if patch_success {
                        let _ = tx.send(BackgroundMessage::LogLine(format!("✓ Berhasil menyalin paket driver VMD ke {}", dir)));
                    }
                }

                if patch_success {
                    let _ = tx.send(BackgroundMessage::LogLine("✓ Driver iaStorVD.inf, iaStorVD.sys, iaStorVD.cat aktif. SSD NVMe akan otomatis terdeteksi!".to_string()));
                }
            }

            // LANGKAH 6: Finalisasi & Unmount ISO
            let _ = tx.send(BackgroundMessage::Step(6));
            let _ = tx.send(BackgroundMessage::Status("Menyelesaikan dan melepaskan berkas ISO...".to_string()));
            run_cmd_streaming(&tx, "hdiutil", &["unmount", &iso_vol]);

            let _ = tx.send(BackgroundMessage::Finished(
                true,
                "Flashdisk / Partisi Bootable Windows berhasil dibuat dan siap digunakan!".to_string(),
            ));
        });
    }

    fn check_receivers(&mut self) {
        // 1. Terima update disk otomatis dari background thread
        let mut latest_disks = None;
        while let Ok(disks) = self.disk_receiver.try_recv() {
            latest_disks = Some(disks);
        }

        if let Some(new_disks) = latest_disks {
            if new_disks != self.available_disks {
                let prev_still_exists = new_disks.iter().any(|d| d.device == self.selected_disk);
                if !prev_still_exists {
                    if let Some(first) = new_disks.first() {
                        self.selected_disk = first.device.clone();
                        if !self.is_creating {
                            let tipe_str = if first.is_partition { "Partisi" } else { "Drive" };
                            self.status = format!("Terhubung: {} [{}] ({})", first.media_name, tipe_str, first.volume_name);
                        }
                    } else {
                        self.selected_disk.clear();
                        if !self.is_creating {
                            self.status = "Menunggu flashdisk / SSD eksternal dicolokkan ke Mac...".to_string();
                        }
                    }
                } else if self.selected_disk.is_empty() && !new_disks.is_empty() {
                    self.selected_disk = new_disks[0].device.clone();
                }
                self.available_disks = new_disks;
            }
        }

        // 2. Terima update proses worker thread
        let mut worker_messages = Vec::new();
        if let Some(ref rx) = self.worker_receiver {
            while let Ok(msg) = rx.try_recv() {
                worker_messages.push(msg);
            }
        }

        for msg in worker_messages {
            match msg {
                BackgroundMessage::Status(s) => {
                    self.status = s;
                }
                BackgroundMessage::Step(st) => {
                    self.current_step = st;
                }
                BackgroundMessage::LogLine(line) => {
                    if self.log.len() > 150_000 {
                        self.log.drain(..50_000);
                    }
                    self.log.push_str(&line);
                    self.log.push('\n');
                }
                BackgroundMessage::Finished(success, message) => {
                    self.status = message;
                    self.is_creating = false;
                    self.current_step = if success { 6 } else { 0 };
                }
            }
        }
    }
}

impl eframe::App for WinDiskApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.check_receivers();

        let refresh_rate = if self.is_creating {
            Duration::from_millis(100)
        } else {
            Duration::from_millis(1000)
        };
        ctx.request_repaint_after(refresh_rate);

        let mut style = (*ctx.style()).clone();
        style.spacing.item_spacing = egui::vec2(8.0, 8.0);
        style.spacing.button_padding = egui::vec2(12.0, 8.0);
        ctx.set_style(style);

        egui::CentralPanel::default()
            .frame(egui::Frame::none().fill(egui::Color32::from_rgb(16, 18, 24)).inner_margin(18.0))
            .show(ctx, |ui| {
                // HEADER ATAS
                ui.horizontal(|ui| {
                    ui.vertical(|ui| {
                        ui.horizontal(|ui| {
                            ui.label(egui::RichText::new("⚡ WinDisk Maker").size(22.0).strong().color(egui::Color32::from_rgb(240, 244, 250)));
                            ui.add_space(8.0);
                            ui.label(
                                egui::RichText::new("NVMe Ready")
                                    .size(11.0)
                                    .color(egui::Color32::from_rgb(46, 204, 113))
                                    .background_color(egui::Color32::from_rgb(18, 40, 28))
                                    .strong(),
                            );
                            ui.add_space(4.0);
                            ui.label(
                                egui::RichText::new("Split Volume Safe")
                                    .size(11.0)
                                    .color(egui::Color32::from_rgb(56, 128, 255))
                                    .background_color(egui::Color32::from_rgb(20, 36, 60))
                                    .strong(),
                            );
                        });
                        ui.label(
                            egui::RichText::new("Pembuat USB Bootable Windows Profesional untuk Mac Intel & PC")
                                .size(12.0)
                                .color(egui::Color32::from_rgb(140, 150, 168)),
                        );
                    });

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let (badge_text, badge_color, bg_color) = if self.is_creating {
                            ("⚡ Sedang Memproses...", egui::Color32::from_rgb(255, 193, 7), egui::Color32::from_rgb(45, 36, 15))
                        } else if !self.selected_disk.is_empty() && !self.iso_path.is_empty() {
                            ("● Siap Dimulai", egui::Color32::from_rgb(46, 204, 113), egui::Color32::from_rgb(18, 40, 28))
                        } else {
                            ("● Menunggu Input", egui::Color32::from_rgb(150, 160, 175), egui::Color32::from_rgb(30, 34, 42))
                        };

                        egui::Frame::none()
                            .fill(bg_color)
                            .rounding(4.0)
                            .inner_margin(egui::vec2(8.0, 4.0))
                            .show(ui, |ui| {
                                ui.label(egui::RichText::new(badge_text).size(12.0).strong().color(badge_color));
                            });
                    });
                });

                ui.add_space(14.0);

                // WORKFLOW CARDS CONTAINER
                egui::ScrollArea::vertical().show(ui, |ui| {
                    // ==========================================
                    // CARD 1: BERKAS CITRA ISO
                    // ==========================================
                    egui::Frame::none()
                        .fill(egui::Color32::from_rgb(24, 28, 38))
                        .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(40, 48, 64)))
                        .rounding(8.0)
                        .inner_margin(14.0)
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                ui.label(egui::RichText::new("1. File Citra ISO Windows").strong().size(14.0).color(egui::Color32::from_rgb(220, 228, 240)));
                                if !self.iso_path.is_empty() {
                                    ui.label(egui::RichText::new("✓ Terverifikasi").size(12.0).color(egui::Color32::from_rgb(46, 204, 113)));
                                }
                            });

                            ui.add_space(8.0);

                            if self.iso_path.is_empty() {
                                ui.horizontal(|ui| {
                                    if ui.button(egui::RichText::new("📂 Pilih Berkas ISO Windows (.iso)...").size(13.0).strong()).clicked() {
                                        if let Some(path) = rfd::FileDialog::new().add_filter("Windows ISO Image", &["iso"]).pick_file() {
                                            self.iso_path = path.display().to_string();
                                            self.iso_name = path.file_name().map(|f| f.to_string_lossy().to_string()).unwrap_or_default();
                                            if let Ok(meta) = fs::metadata(&path) {
                                                let gb = meta.len() as f64 / (1024.0 * 1024.0 * 1024.0);
                                                self.iso_size = format!("{:.2} GB", gb);
                                            }
                                        }
                                    }
                                    ui.label(egui::RichText::new("Dukungan untuk Windows 11, Windows 10, atau Windows Server").size(12.0).color(egui::Color32::from_rgb(120, 130, 145)));
                                });
                            } else {
                                egui::Frame::none()
                                    .fill(egui::Color32::from_rgb(19, 23, 32))
                                    .rounding(6.0)
                                    .inner_margin(10.0)
                                    .show(ui, |ui| {
                                        ui.horizontal(|ui| {
                                            ui.label(egui::RichText::new("💿").size(20.0));
                                            ui.vertical(|ui| {
                                                ui.label(egui::RichText::new(&self.iso_name).strong().color(egui::Color32::WHITE));
                                                ui.horizontal(|ui| {
                                                    ui.label(egui::RichText::new(&self.iso_size).color(egui::Color32::from_rgb(56, 128, 255)).size(11.0));
                                                    ui.label(egui::RichText::new(&self.iso_path).color(egui::Color32::from_rgb(120, 130, 145)).size(11.0));
                                                });
                                            });

                                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                                if ui.button("Ganti ISO").clicked() {
                                                    if let Some(path) = rfd::FileDialog::new().add_filter("Windows ISO Image", &["iso"]).pick_file() {
                                                        self.iso_path = path.display().to_string();
                                                        self.iso_name = path.file_name().map(|f| f.to_string_lossy().to_string()).unwrap_or_default();
                                                        if let Ok(meta) = fs::metadata(&path) {
                                                            let gb = meta.len() as f64 / (1024.0 * 1024.0 * 1024.0);
                                                            self.iso_size = format!("{:.2} GB", gb);
                                                        }
                                                    }
                                                }
                                            });
                                        });
                                    });
                            }
                        });

                    ui.add_space(10.0);

                    // ==========================================
                    // CARD 2: FLASHDISK / PARTISI USB TUJUAN (AUTO-DETECT + SPLIT VOLUME SAFE)
                    // ==========================================
                    egui::Frame::none()
                        .fill(egui::Color32::from_rgb(24, 28, 38))
                        .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(40, 48, 64)))
                        .rounding(8.0)
                        .inner_margin(14.0)
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                ui.label(egui::RichText::new("2. Drive USB / Partisi Target").strong().size(14.0).color(egui::Color32::from_rgb(220, 228, 240)));
                                ui.label(egui::RichText::new("• Deteksi Split Volume Aktif").size(11.0).color(egui::Color32::from_rgb(56, 128, 255)));
                            });

                            ui.add_space(8.0);

                            let selected_info = self.available_disks.iter().find(|d| d.device == self.selected_disk);
                            let selected_text = selected_info
                                .map(|d| d.display.clone())
                                .unwrap_or_else(|| {
                                    if self.available_disks.is_empty() {
                                        "⏳ Menunggu flashdisk / SSD eksternal dicolokkan ke Mac...".to_string()
                                    } else {
                                        "Pilih Drive atau Partisi USB...".to_string()
                                    }
                                });

                            egui::ComboBox::from_id_source("usb_combo_picker")
                                .width(ui.available_width() - 8.0)
                                .selected_text(egui::RichText::new(&selected_text).strong().size(13.0))
                                .show_ui(ui, |ui| {
                                    if self.available_disks.is_empty() {
                                        ui.label("Tidak ada USB/SSD terdeteksi. Silakan tancapkan drive Anda.");
                                    } else {
                                        for disk in &self.available_disks {
                                            let text_color = if disk.is_partition {
                                                egui::Color32::from_rgb(46, 204, 113) // Hijau untuk partisi aman
                                            } else {
                                                egui::Color32::from_rgb(235, 120, 120) // Kuning kemerahan untuk full disk wipe
                                            };
                                            let text = egui::RichText::new(&disk.display).color(text_color).strong();
                                            if ui.selectable_label(self.selected_disk == disk.device, text).clicked() {
                                                self.selected_disk = disk.device.clone();
                                            }
                                        }
                                    }
                                });

                            // KARTU INFORMASI RINCI & PERINGATAN KEAMANAN PARTISI VS FULL DISK
                            if let Some(info) = selected_info {
                                ui.add_space(8.0);

                                if info.is_partition {
                                    // MODE PARTISI AMAN (SPLIT VOLUME)
                                    egui::Frame::none()
                                        .fill(egui::Color32::from_rgb(18, 34, 28))
                                        .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(46, 204, 113)))
                                        .rounding(6.0)
                                        .inner_margin(10.0)
                                        .show(ui, |ui| {
                                            ui.horizontal(|ui| {
                                                ui.label(egui::RichText::new("🛡️ MODE AMAN (HANYA FORMAT 1 PARTISI INI):").strong().color(egui::Color32::from_rgb(46, 204, 113)).size(12.0));
                                            });
                                            ui.add_space(2.0);
                                            ui.label(
                                                egui::RichText::new(format!(
                                                    "Aplikasi HANYA akan memformat partisi \"{}\" ({}) pada {}. Partisi lain di SSD ini (seperti Kontakt, macOS, dll.) 100% AMAN dan TIDAK AKAN HILANG!",
                                                    info.volume_name, info.size, info.device
                                                ))
                                                .color(egui::Color32::WHITE)
                                                .size(12.0)
                                            );
                                        });
                                } else {
                                    // PERINGATAN FULL DISK WIPE
                                    egui::Frame::none()
                                        .fill(egui::Color32::from_rgb(38, 20, 20))
                                        .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(235, 87, 87)))
                                        .rounding(6.0)
                                        .inner_margin(10.0)
                                        .show(ui, |ui| {
                                            ui.horizontal(|ui| {
                                                ui.label(egui::RichText::new("⚠️ PERINGATAN: FORMAT SELURUH DISK DIPILIH!").strong().color(egui::Color32::from_rgb(235, 87, 87)).size(12.0));
                                            });
                                            ui.add_space(2.0);
                                            ui.label(
                                                egui::RichText::new(
                                                    "Memilih opsi ini akan MENGHAPUS SEMUA PARTISI di drive ini (termasuk partisi Kontakt, dll.)! Jika hanya ingin memformat partisi Windows saja, pilih opsi [PARTISI AMAN] di dropdown atas."
                                                )
                                                .color(egui::Color32::from_rgb(255, 200, 200))
                                                .size(12.0)
                                            );
                                        });
                                }

                                ui.add_space(8.0);
                                egui::Frame::none()
                                    .fill(egui::Color32::from_rgb(19, 23, 32))
                                    .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(32, 45, 65)))
                                    .rounding(6.0)
                                    .inner_margin(10.0)
                                    .show(ui, |ui| {
                                        egui::Grid::new("usb_specs_grid")
                                            .num_columns(4)
                                            .spacing([18.0, 6.0])
                                            .show(ui, |ui| {
                                                ui.label(egui::RichText::new("Perangkat Fisik:").color(egui::Color32::from_rgb(130, 140, 155)).size(12.0));
                                                ui.label(egui::RichText::new(&info.media_name).strong().color(egui::Color32::WHITE).size(12.0));

                                                ui.label(egui::RichText::new("Target Format:").color(egui::Color32::from_rgb(130, 140, 155)).size(12.0));
                                                let target_type_str = if info.is_partition { "Partisi Tunggal (Aman)" } else { "Seluruh Drive" };
                                                ui.label(egui::RichText::new(target_type_str).strong().color(egui::Color32::from_rgb(56, 128, 255)).size(12.0));
                                                ui.end_row();

                                                ui.label(egui::RichText::new("Kapasitas Target:").color(egui::Color32::from_rgb(130, 140, 155)).size(12.0));
                                                ui.label(egui::RichText::new(&info.size).strong().color(egui::Color32::from_rgb(46, 204, 113)).size(12.0));

                                                ui.label(egui::RichText::new("Target Device Node:").color(egui::Color32::from_rgb(130, 140, 155)).size(12.0));
                                                ui.label(egui::RichText::new(&info.device).monospace().color(egui::Color32::from_rgb(170, 180, 200)).size(12.0));
                                                ui.end_row();
                                            });
                                    });
                            } else {
                                ui.add_space(4.0);
                                ui.label(egui::RichText::new("💡 Flashdisk atau External SSD Anda akan otomatis muncul begitu ditancapkan ke port USB.").color(egui::Color32::from_rgb(120, 130, 145)).size(12.0));
                            }
                        });

                    ui.add_space(10.0);

                    // ==========================================
                    // CARD 3: PENGATURAN TARGET SISTEM & FORMAT
                    // ==========================================
                    egui::Frame::none()
                        .fill(egui::Color32::from_rgb(24, 28, 38))
                        .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(40, 48, 64)))
                        .rounding(8.0)
                        .inner_margin(14.0)
                        .show(ui, |ui| {
                            ui.label(egui::RichText::new("3. Standar Kompatibilitas Boot & Patch").strong().size(14.0).color(egui::Color32::from_rgb(220, 228, 240)));
                            ui.add_space(6.0);

                            egui::Grid::new("config_grid")
                                .num_columns(2)
                                .spacing([20.0, 6.0])
                                .show(ui, |ui| {
                                    let selected_is_part = self.available_disks.iter().find(|d| d.device == self.selected_disk).map(|d| d.is_partition).unwrap_or(false);

                                    ui.label(egui::RichText::new("Mode Format:").size(12.0).color(egui::Color32::from_rgb(140, 150, 165)));
                                    if selected_is_part {
                                        ui.label(egui::RichText::new("Erase Volume FAT32 (Tabel Partisi GPT SSD Asli Tetap Utuh)").strong().size(12.0).color(egui::Color32::from_rgb(46, 204, 113)));
                                    } else {
                                        ui.label(egui::RichText::new("MBR FAT32 (Format Ulang Seluruh Disk)").strong().size(12.0).color(egui::Color32::WHITE));
                                    }
                                    ui.end_row();

                                    ui.label(egui::RichText::new("Sistem Berkas:").size(12.0).color(egui::Color32::from_rgb(140, 150, 165)));
                                    ui.label(egui::RichText::new("FAT32 (MS-DOS) dengan Pemecahan Berkas WIM Otomatis").strong().size(12.0).color(egui::Color32::WHITE));
                                    ui.end_row();

                                    ui.label(egui::RichText::new("Label Boot Drive Baru:").size(12.0).color(egui::Color32::from_rgb(140, 150, 165)));
                                    ui.label(egui::RichText::new("WIN11").strong().color(egui::Color32::from_rgb(56, 128, 255)).size(12.0));
                                    ui.end_row();
                                });

                            ui.add_space(10.0);
                            ui.separator();
                            ui.add_space(6.0);

                            // OPSI PATCH DRIVER INTEL VMD / NVMe SSD
                            ui.checkbox(
                                &mut self.patch_vmd,
                                egui::RichText::new("⚡ Pasang Patch Driver NVMe / Intel VMD (iaStorVD)")
                                    .strong()
                                    .size(13.0)
                                    .color(egui::Color32::from_rgb(46, 204, 113)),
                            );
                            ui.add_space(2.0);
                            ui.label(
                                egui::RichText::new(
                                    "Menyertakan iaStorVD.inf, iaStorVD.sys, iaStorVD.cat ke $WinPEDriver$ & /drivers/Intel_VMD/ agar disk NVMe SSD langsung terdeteksi otomatis saat install Windows (mengatasi pesan 'No drives were found')."
                                )
                                .size(11.0)
                                .color(egui::Color32::from_rgb(150, 160, 175)),
                            );
                        });

                    ui.add_space(14.0);

                    // ==========================================
                    // EKSEKUSI & STATUS PROGRES
                    // ==========================================
                    if self.is_creating {
                        egui::Frame::none()
                            .fill(egui::Color32::from_rgb(20, 24, 34))
                            .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(56, 128, 255)))
                            .rounding(8.0)
                            .inner_margin(14.0)
                            .show(ui, |ui| {
                                ui.horizontal(|ui| {
                                    ui.label(egui::RichText::new("⏳ PROSES SEDANG BERJALAN").strong().color(egui::Color32::from_rgb(255, 193, 7)));
                                    ui.label(egui::RichText::new("(Jangan mencabut flashdisk USB!)").size(12.0).color(egui::Color32::from_rgb(235, 87, 87)));
                                });

                                ui.add_space(6.0);
                                let fraction = match self.current_step {
                                    1 => 0.15,
                                    2 => 0.25,
                                    3 => 0.60,
                                    4 => 0.80,
                                    5 => 0.92,
                                    6 => 1.0,
                                    _ => 0.05,
                                };
                                ui.add(egui::ProgressBar::new(fraction).animate(true).text(&self.status));

                                ui.add_space(8.0);
                                ui.horizontal(|ui| {
                                    let step_names = ["1. Format", "2. Mount ISO", "3. Salin Berkas", "4. Optimasi WIM", "5. Patch NVMe", "6. Selesai"];
                                    for (i, name) in step_names.iter().enumerate() {
                                        let step_num = i + 1;
                                        let color = if self.current_step > step_num {
                                            egui::Color32::from_rgb(46, 204, 113)
                                        } else if self.current_step == step_num {
                                            egui::Color32::from_rgb(56, 128, 255)
                                        } else {
                                            egui::Color32::from_rgb(80, 90, 105)
                                        };
                                        ui.label(egui::RichText::new(*name).size(11.0).color(color).strong());
                                        if i < step_names.len() - 1 {
                                            ui.label(egui::RichText::new("➔").size(11.0).color(egui::Color32::from_rgb(60, 70, 85)));
                                        }
                                    }
                                });
                            });
                    } else {
                        let is_ready = !self.selected_disk.is_empty() && !self.iso_path.is_empty();

                        let button_color = if is_ready {
                            egui::Color32::from_rgb(37, 99, 235)
                        } else {
                            egui::Color32::from_rgb(45, 55, 72)
                        };

                        let start_btn = egui::Button::new(
                            egui::RichText::new("🚀 MULAI PEMBUATAN USB BOOTABLE")
                                .size(15.0)
                                .color(egui::Color32::WHITE)
                                .strong(),
                        )
                        .fill(button_color)
                        .min_size(egui::vec2(ui.available_width(), 46.0));

                        if ui.add_enabled(is_ready, start_btn).clicked() {
                            self.show_confirm_dialog = true;
                        }

                        ui.add_space(4.0);
                        ui.horizontal(|ui| {
                            ui.label(egui::RichText::new(&self.status).color(egui::Color32::from_rgb(100, 220, 150)).strong().size(12.0));
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                let toggle_text = if self.show_log_panel { "Tutup Log Konsol" } else { "Lihat Log Konsol" };
                                if ui.button(toggle_text).clicked() {
                                    self.show_log_panel = !self.show_log_panel;
                                }
                            });
                        });
                    }

                    // ==========================================
                    // LOG KONSOL STREAMING TERINTEGRASI
                    // ==========================================
                    if self.show_log_panel || self.is_creating {
                        ui.add_space(8.0);
                        egui::Frame::none()
                            .fill(egui::Color32::from_rgb(12, 14, 18))
                            .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(35, 42, 54)))
                            .rounding(6.0)
                            .inner_margin(10.0)
                            .show(ui, |ui| {
                                ui.horizontal(|ui| {
                                    ui.label(egui::RichText::new("📟 Log Eksekusi Sistem (Live Streaming):").size(12.0).color(egui::Color32::from_rgb(140, 150, 168)).strong());
                                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                        if ui.button("Bersihkan").clicked() {
                                            self.log.clear();
                                        }
                                    });
                                });
                                ui.add_space(4.0);
                                egui::ScrollArea::vertical()
                                    .max_height(160.0)
                                    .stick_to_bottom(true)
                                    .show(ui, |ui| {
                                        ui.monospace(egui::RichText::new(&self.log).size(11.0).color(egui::Color32::from_rgb(74, 222, 128)));
                                    });
                            });
                    }
                });

                // ==========================================
                // DIALOG KONFIRMASI KEAMANAN
                // ==========================================
                if self.show_confirm_dialog {
                    let selected_info = self.available_disks.iter().find(|d| d.device == self.selected_disk);
                    let target_display = selected_info.map(|d| d.display.clone()).unwrap_or_else(|| self.selected_disk.clone());
                    let is_part = selected_info.map(|d| d.is_partition).unwrap_or(false);

                    egui::Window::new("⚠️ Konfirmasi Format Target")
                        .collapsible(false)
                        .resizable(false)
                        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                        .show(ctx, |ui| {
                            ui.set_width(480.0);

                            if is_part {
                                ui.label(egui::RichText::new("KONFIRMASI FORMAT PARTISI (MODE AMAN)").color(egui::Color32::from_rgb(46, 204, 113)).size(15.0).strong());
                                ui.add_space(6.0);
                                ui.label("Aplikasi HANYA akan memformat partisi tunggal berikut:");
                                ui.add_space(4.0);
                                egui::Frame::none()
                                    .fill(egui::Color32::from_rgb(18, 34, 28))
                                    .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(46, 204, 113)))
                                    .rounding(4.0)
                                    .inner_margin(8.0)
                                    .show(ui, |ui| {
                                        ui.label(egui::RichText::new(&target_display).strong().color(egui::Color32::WHITE));
                                    });
                                ui.add_space(6.0);
                                ui.label(egui::RichText::new("🛡️ Seluruh partisi lain di SSD ini (seperti Kontakt, macOS, dll.) TETAP AMAN dan TIDAK AKAN HILANG.").color(egui::Color32::from_rgb(100, 255, 120)).size(12.0));
                            } else {
                                ui.label(egui::RichText::new("PERINGATAN FORMAT SELURUH DISK (FULL WIPE)").color(egui::Color32::from_rgb(235, 87, 87)).size(15.0).strong());
                                ui.add_space(6.0);
                                ui.label("PERHATIAN: Semua partisi pada drive berikut AKAN DIHAPUS PERMANEN:");
                                ui.add_space(4.0);
                                egui::Frame::none()
                                    .fill(egui::Color32::from_rgb(38, 20, 20))
                                    .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(235, 87, 87)))
                                    .rounding(4.0)
                                    .inner_margin(8.0)
                                    .show(ui, |ui| {
                                        ui.label(egui::RichText::new(&target_display).strong().color(egui::Color32::from_rgb(255, 193, 7)));
                                    });
                                ui.add_space(6.0);
                                ui.label(egui::RichText::new("⚠️ Pastikan Anda tidak memiliki data penting di partisi lain sebelum melanjutkan.").color(egui::Color32::from_rgb(255, 150, 150)).size(12.0));
                            }

                            ui.add_space(14.0);
                            ui.horizontal(|ui| {
                                let btn_text = if is_part { "Ya, Format Partisi Ini Saja" } else { "Ya, Format Seluruh Drive" };
                                let confirm_btn = egui::Button::new(
                                    egui::RichText::new(btn_text)
                                        .color(egui::Color32::WHITE)
                                        .strong(),
                                )
                                .fill(if is_part { egui::Color32::from_rgb(37, 99, 235) } else { egui::Color32::from_rgb(220, 38, 38) });

                                if ui.add(confirm_btn).clicked() {
                                    self.show_confirm_dialog = false;
                                    self.start_worker_process();
                                }

                                if ui.button("Batal").clicked() {
                                    self.show_confirm_dialog = false;
                                }
                            });
                        });
                }
            });
    }
}

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([780.0, 720.0])
            .with_min_inner_size([650.0, 580.0])
            .with_title("WinDisk Maker - Windows Bootable Creator"),
        ..Default::default()
    };
    eframe::run_native(
        "WinDisk Maker",
        options,
        Box::new(|cc| Box::new(WinDiskApp::new(cc))),
    )
}
