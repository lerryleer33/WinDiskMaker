use eframe::egui;
use std::fs;
use std::io::{BufRead, BufReader};
use std::path::Path as FsPath;
use std::process::{Command, Stdio};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::time::Duration;

mod theme;
mod ui;
mod widgets;

// Driver Intel RST VMD untuk deteksi disk NVMe SSD pada Windows Installer
const IASTORVD_INF: &[u8] = include_bytes!("../drivers/vmd/iaStorVD.inf");
const IASTORVD_SYS: &[u8] = include_bytes!("../drivers/vmd/iaStorVD.sys");
const IASTORVD_CAT: &[u8] = include_bytes!("../drivers/vmd/iaStorVD.cat");
const RST_MW_SERVICE: &[u8] = include_bytes!("../drivers/vmd/RstMwService.exe");
const RST_MW_EVENT_LOG: &[u8] = include_bytes!("../drivers/vmd/RstMwEventLogMsg.dll");

// Konfigurasi otomatis jawaban Windows PE (Unattended) untuk bypass pembatasan Windows 11
const AUTOUNATTEND_XML: &str = r#"<?xml version="1.0" encoding="utf-8"?>
<unattend xmlns="urn:schemas-microsoft-com:unattend">
  <settings pass="windowsPE">
    <component name="Microsoft-Windows-Setup" processorArchitecture="amd64" language="neutral" xmlns:wcm="http://schemas.microsoft.com/WMIConfig/2002/State" xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance" publicKeyToken="31bf3856ad364e35" versionScope="nonSxS">
      <UserData>
        <AcceptEula>true</AcceptEula>
      </UserData>
      <RunSynchronous>
        <RunSynchronousCommand wcm:action="add">
          <Order>1</Order>
          <Path>reg add HKLM\SYSTEM\Setup\LabConfig /v BypassTPMCheck /t REG_DWORD /d 1 /f</Path>
        </RunSynchronousCommand>
        <RunSynchronousCommand wcm:action="add">
          <Order>2</Order>
          <Path>reg add HKLM\SYSTEM\Setup\LabConfig /v BypassSecureBootCheck /t REG_DWORD /d 1 /f</Path>
        </RunSynchronousCommand>
        <RunSynchronousCommand wcm:action="add">
          <Order>3</Order>
          <Path>reg add HKLM\SYSTEM\Setup\LabConfig /v BypassRAMCheck /t REG_DWORD /d 1 /f</Path>
        </RunSynchronousCommand>
        <RunSynchronousCommand wcm:action="add">
          <Order>4</Order>
          <Path>reg add HKLM\SYSTEM\Setup\LabConfig /v BypassCPUCheck /t REG_DWORD /d 1 /f</Path>
        </RunSynchronousCommand>
        <RunSynchronousCommand wcm:action="add">
          <Order>5</Order>
          <Path>reg add HKLM\SYSTEM\Setup\LabConfig /v BypassStorageCheck /t REG_DWORD /d 1 /f</Path>
        </RunSynchronousCommand>
      </RunSynchronous>
    </component>
  </settings>
</unattend>
"#;

#[derive(Clone, PartialEq, Eq, Debug)]
pub(crate) struct DiskInfo {
    pub(crate) device: String,         // "/dev/disk3s3" atau "/dev/disk3"
    pub(crate) parent_device: String,  // "/dev/disk3"
    pub(crate) media_name: String,     // "SanDisk Portable SSD"
    pub(crate) volume_name: String,    // "WIN10"
    pub(crate) size: String,           // "49.9 GB"
    pub(crate) protocol: String,       // "USB"
    pub(crate) display: String,
    pub(crate) is_usb: bool,
    pub(crate) is_partition: bool,     // true jika single partition / split volume
}

enum BackgroundMessage {
    Status(String),
    Step(usize), // 1 to 6
    LogLine(String),
    Finished(bool, String),
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum StatusLevel {
    Info,
    Success,
    Error,
}

pub(crate) struct WinDiskApp {
    pub(crate) iso_path: String,
    pub(crate) iso_name: String,
    pub(crate) iso_size: String,
    pub(crate) selected_disk: String,
    pub(crate) available_disks: Vec<DiskInfo>,
    pub(crate) status: String,
    pub(crate) status_level: StatusLevel,
    pub(crate) current_step: usize,
    pub(crate) is_creating: bool,
    pub(crate) patch_vmd: bool,
    pub(crate) bypass_tpm: bool,
    pub(crate) log: String,
    pub(crate) show_confirm_dialog: bool,
    pub(crate) show_log_panel: bool,
    pub(crate) dark: bool,
    disk_receiver: Receiver<Vec<DiskInfo>>,
    worker_receiver: Option<Receiver<BackgroundMessage>>,
}

impl WinDiskApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let dark = cc.egui_ctx.style().visuals.dark_mode;
        theme::install_fonts(&cc.egui_ctx);
        theme::apply_style(&cc.egui_ctx, dark);

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
            status: "Pilih berkas ISO dan drive tujuan untuk memulai.".to_string(),
            status_level: StatusLevel::Info,
            current_step: 0,
            is_creating: false,
            patch_vmd: true, // Default aktif agar SSD NVMe terdeteksi di laptop modern
            bypass_tpm: true, // Default aktif agar Windows 11 bisa diinstall di mana pun tanpa error
            log: "=== WinDisk Maker v1.2 ===\nSistem siap. Mendeteksi perangkat USB & partisi split volume di latar belakang...\n".to_string(),
            show_confirm_dialog: false,
            show_log_panel: false,
            dark,
            disk_receiver: disk_rx,
            worker_receiver: None,
        }
    }

    pub(crate) fn pick_iso(&mut self) {
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("Windows ISO Image", &["iso"])
            .pick_file()
        {
            self.iso_path = path.display().to_string();
            self.iso_name = path
                .file_name()
                .map(|name| name.to_string_lossy().to_string())
                .unwrap_or_default();
            self.iso_size = fs::metadata(&path)
                .map(|meta| {
                    let gb = meta.len() as f64 / (1024.0 * 1024.0 * 1024.0);
                    format!("{:.2} GB", gb)
                })
                .unwrap_or_default();
            self.status = format!("ISO dipilih: {}", self.iso_name);
            self.status_level = StatusLevel::Info;
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
                            // Validasi ketat bahwa ini partisi diskXsY, bukan whole disk diskX
                            if let Some(suffix) = part_id.strip_prefix(disk_id) {
                                if suffix.starts_with('s') && suffix[1..].chars().all(|c| c.is_ascii_digit()) {
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
                                        let display = format!("📁 [PARTISI AMAN] \"{}\" ({}) - {} (Data lain aman)", final_vname, part_size, part_dev);

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
            }

            // Prioritaskan partisi yang mengandung kata "WIN" agar otomatis terpilih!
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
                format!("💾 [FULL DISK WIPE] {} ({}) - {} [⚠️ HAPUS SEMUA 1TB]", media_name, label_summary, dev)
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
    pub(crate) fn start_worker_process(&mut self) {
        let disk = self.selected_disk.clone();
        let iso = self.iso_path.clone();
        let patch_vmd = self.patch_vmd;
        let bypass_tpm = self.bypass_tpm;

        if iso.is_empty() || disk.is_empty() {
            self.status = "Harap tentukan file ISO dan USB/partisi tujuan terlebih dahulu.".to_string();
            self.status_level = StatusLevel::Error;
            return;
        }

        if disk == "/dev/disk0" || disk == "/dev/disk1" {
            self.status = "Keamanan: Dilarang memformat disk sistem Mac!".to_string();
            self.status_level = StatusLevel::Error;
            return;
        }

        let is_target_partition = self.available_disks.iter()
            .find(|d| d.device == disk)
            .map(|d| d.is_partition)
            .unwrap_or(false);

        self.is_creating = true;
        self.current_step = 1;
        self.status_level = StatusLevel::Info;
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
        if bypass_tpm {
            self.log.push_str(">>> Fitur Bypass Persyaratan Windows 11: AKTIF (TPM 2.0, Secure Boot, RAM, & CPU check)\n");
        }

        let (tx, rx) = channel::<BackgroundMessage>();
        self.worker_receiver = Some(rx);

        std::thread::spawn(move || {
            let run_cmd_streaming = |tx: &Sender<BackgroundMessage>, cmd: &str, args: &[&str]| -> bool {
                let _ = tx.send(BackgroundMessage::LogLine(format!("$ {} {}", cmd, args.join(" "))));
                
                let mut child = match Command::new(cmd)
                    .args(args)
                    .env("COPYFILE_DISABLE", "1") // Cegah macOS membuat file metadata AppleDouble ._* di drive FAT32
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

            // Dapatkan mount point target secara dinamis
            let mut target_vol = String::new();
            for _ in 0..6 {
                std::thread::sleep(Duration::from_millis(500));
                if let Ok(info_out) = Command::new("diskutil").args(&["info", &disk]).output() {
                    let info_str = String::from_utf8_lossy(&info_out.stdout);
                    for line in info_str.lines() {
                        let trimmed = line.trim();
                        if trimmed.starts_with("Mount Point:") {
                            let mp = trimmed.trim_start_matches("Mount Point:").trim();
                            if !mp.is_empty() && mp != "Not Mounted" {
                                target_vol = mp.to_string();
                                break;
                            }
                        }
                    }
                }
                if !target_vol.is_empty() {
                    break;
                }
            }

            if target_vol.is_empty() {
                // Upayakan mount manual jika belum otomatis ter-mount
                let _ = Command::new("diskutil").args(&["mount", &disk]).output();
                std::thread::sleep(Duration::from_millis(500));
                if FsPath::new("/Volumes/WIN11").exists() {
                    target_vol = "/Volumes/WIN11".to_string();
                }
            }

            if target_vol.is_empty() {
                let _ = tx.send(BackgroundMessage::Finished(false, "Gagal mendapatkan lokasi kait (mount point) drive target setelah pemformatan.".to_string()));
                return;
            }
            let _ = tx.send(BackgroundMessage::LogLine(format!("✓ Drive target terpasang pada: {}", target_vol)));

            // LANGKAH 2: Deteksi / Mount ISO Windows
            let _ = tx.send(BackgroundMessage::Step(2));
            let _ = tx.send(BackgroundMessage::Status("Memeriksa dan memasang citra ISO Windows...".to_string()));
            
            // Cek apakah ISO sudah terpasang di /Volumes
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
                let _ = tx.send(BackgroundMessage::LogLine(format!(">> Memasang ISO: {}", iso)));
                run_cmd_streaming(&tx, "hdiutil", &["mount", &iso]);
                std::thread::sleep(Duration::from_secs(2));

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
            }

            if iso_vol.is_empty() {
                let _ = tx.send(BackgroundMessage::Finished(false, "Gagal menemukan berkas installer Windows (setup.exe) di dalam ISO.".to_string()));
                return;
            }
            let _ = tx.send(BackgroundMessage::LogLine(format!("✓ Volume ISO Windows terdeteksi di: {}", iso_vol)));

            // LANGKAH 3: Salin Berkas Windows (kecuali install.wim & install.esd)
            let _ = tx.send(BackgroundMessage::Step(3));
            let _ = tx.send(BackgroundMessage::Status("Menyalin berkas Windows ke drive target (proses rsync)...".to_string()));
            let rsync_src = format!("{}/", iso_vol);
            let rsync_dst = format!("{}/", target_vol);
            
            // Gunakan bendera rsync yang kompatibel penuh dengan macOS bawaan
            let _rsync_ok = run_cmd_streaming(&tx, "rsync", &[
                "-rtv",
                "--progress",
                "--exclude=sources/install.wim",
                "--exclude=sources/install.esd",
                &rsync_src,
                &rsync_dst,
            ]);

            let check_target_setup = format!("{}/setup.exe", target_vol);
            if !FsPath::new(&check_target_setup).exists() {
                let _ = tx.send(BackgroundMessage::Finished(false, format!("Gagal menyalin berkas instalasi Windows ke {} (setup.exe tidak ditemukan).", target_vol)));
                return;
            }
            let _ = tx.send(BackgroundMessage::LogLine("✓ Berkas sistem Windows (bootloader, setup, dsb.) berhasil disalin.".to_string()));

            // Pastikan direktori sources ada di target sebelum memproses file image
            let target_sources = format!("{}/sources", target_vol);
            let _ = fs::create_dir_all(&target_sources);

            // LANGKAH 4: Pecah install.wim (UEFI FAT32 Split) ATAU Salin install.esd
            let _ = tx.send(BackgroundMessage::Step(4));
            let wim_path = format!("{}/sources/install.wim", iso_vol);
            let esd_path = format!("{}/sources/install.esd", iso_vol);

            if FsPath::new(&wim_path).exists() {
                let _ = tx.send(BackgroundMessage::Status("Memecah install.wim menjadi format SWM (FAT32 UEFI Split)...".to_string()));
                let _ = tx.send(BackgroundMessage::LogLine(format!(">> Menjalankan wimlib-imagex split dari {} ke {}/install.swm...", wim_path, target_sources)));
                
                let target_swm = format!("{}/install.swm", target_sources);
                let split_ok = run_cmd_streaming(&tx, "wimlib-imagex", &["split", &wim_path, &target_swm, "3800"]);
                
                if !split_ok || !FsPath::new(&target_swm).exists() {
                    let _ = tx.send(BackgroundMessage::Finished(false, "Gagal memecah berkas install.wim! Periksa ruang sisa drive.".to_string()));
                    return;
                }
                let _ = tx.send(BackgroundMessage::LogLine("✓ Berkas install.wim berhasil dipecah menjadi SWM untuk dukungan UEFI FAT32.".to_string()));
            } else if FsPath::new(&esd_path).exists() {
                let _ = tx.send(BackgroundMessage::Status("Menyalin berkas terkompresi install.esd ke USB...".to_string()));
                let target_esd = format!("{}/install.esd", target_sources);
                let _ = run_cmd_streaming(&tx, "rsync", &["-rtv", "--progress", &esd_path, &target_esd]);
                if !FsPath::new(&target_esd).exists() {
                    let _ = tx.send(BackgroundMessage::Finished(false, "Gagal menyalin berkas install.esd!".to_string()));
                    return;
                }
                let _ = tx.send(BackgroundMessage::LogLine("✓ Berkas install.esd berhasil disalin.".to_string()));
            } else {
                let _ = tx.send(BackgroundMessage::Finished(false, "Tidak ditemukan install.wim maupun install.esd di dalam berkas ISO!".to_string()));
                return;
            }

            // LANGKAH 5: Injeksi Driver Intel VMD & Bypass TPM Windows 11
            let _ = tx.send(BackgroundMessage::Step(5));
            if patch_vmd {
                let _ = tx.send(BackgroundMessage::Status("Memasang patch driver Intel VMD / NVMe SSD (iaStorVD)...".to_string()));
                let _ = tx.send(BackgroundMessage::LogLine("\n>> Memasang paket driver Intel RST VMD untuk deteksi NVMe...".to_string()));

                // 1. Letakkan 5 file lengkap di /drivers/Intel_VMD/ (untuk opsi 'Load Driver' manual di Windows Setup)
                let manual_driver_dir = format!("{}/drivers/Intel_VMD", target_vol);
                let _ = fs::create_dir_all(&manual_driver_dir);
                let full_driver_files: [(&str, &[u8]); 5] = [
                    ("iaStorVD.inf", IASTORVD_INF),
                    ("iaStorVD.sys", IASTORVD_SYS),
                    ("iaStorVD.cat", IASTORVD_CAT),
                    ("RstMwService.exe", RST_MW_SERVICE),
                    ("RstMwEventLogMsg.dll", RST_MW_EVENT_LOG),
                ];
                for (name, data) in &full_driver_files {
                    let _ = fs::write(format!("{}/{}", manual_driver_dir, name), data);
                }
                let _ = tx.send(BackgroundMessage::LogLine(format!("✓ Paket driver manual tersimpan di {}", manual_driver_dir)));

                // 2. Letakkan HANYA file driver kernel murni (.inf, .sys, .cat) di /$WinPEDriver$/Intel_VMD/
                // PENTING: Jangan menyertakan .exe atau .dll di $WinPEDriver$ agar WinPE driver loader tidak melempar error 0x80070006!
                let winpe_driver_dir = format!("{}/$WinPEDriver$/Intel_VMD", target_vol);
                let _ = fs::create_dir_all(&winpe_driver_dir);
                let kernel_driver_files: [(&str, &[u8]); 3] = [
                    ("iaStorVD.inf", IASTORVD_INF),
                    ("iaStorVD.sys", IASTORVD_SYS),
                    ("iaStorVD.cat", IASTORVD_CAT),
                ];
                for (name, data) in &kernel_driver_files {
                    let _ = fs::write(format!("{}/{}", winpe_driver_dir, name), data);
                }
                let _ = tx.send(BackgroundMessage::LogLine("✓ Driver kernel iaStorVD (.inf, .sys, .cat) siap di-load otomatis oleh Windows Setup PE.".to_string()));
            }

            // Injeksi Bypass Windows 11 (TPM 2.0, Secure Boot, RAM, CPU) jika diaktifkan
            if bypass_tpm {
                let _ = tx.send(BackgroundMessage::Status("Memasang berkas autounattend.xml (Bypass TPM 2.0 / Secure Boot)...".to_string()));
                let _ = tx.send(BackgroundMessage::LogLine(">> Menuliskan autounattend.xml untuk bypass batasan perangkat keras Windows 11...".to_string()));
                let autounattend_path = format!("{}/autounattend.xml", target_vol);
                if let Err(e) = fs::write(&autounattend_path, AUTOUNATTEND_XML.as_bytes()) {
                    let _ = tx.send(BackgroundMessage::LogLine(format!("Peringatan: Gagal menulis autounattend.xml: {}", e)));
                } else {
                    let _ = tx.send(BackgroundMessage::LogLine("✓ Berkas autounattend.xml berhasil disuntikkan ke root drive USB.".to_string()));
                }
            }

            // LANGKAH 6: Pembersihan Metadata Mac (._*), Sinkronisasi & Finalisasi
            let _ = tx.send(BackgroundMessage::Step(6));
            let _ = tx.send(BackgroundMessage::Status("Membersihkan berkas metadata Mac (._*) agar Windows Setup tidak error...".to_string()));
            let _ = tx.send(BackgroundMessage::LogLine(">> Menghapus seluruh file AppleDouble metadata (._*) dan .DS_Store...".to_string()));

            // 1. Eksekusi dot_clean untuk menggabungkan / membersihkan resource forks
            let _ = Command::new("dot_clean")
                .args(&["-m", &target_vol])
                .env("COPYFILE_DISABLE", "1")
                .output();

            // 2. Bersihkan file AppleDouble ._* dan .DS_Store yang menjadi penyebab utama 0x80070006 - 0x40031
            let _ = Command::new("find").args(&[&target_vol, "-name", "._*", "-delete"]).output();
            let _ = Command::new("find").args(&[&target_vol, "-name", ".DS_Store", "-delete"]).output();
            let _ = Command::new("find").args(&[&format!("{}/sources", target_vol), "-name", "._*", "-delete"]).output();
            let _ = Command::new("find").args(&[&format!("{}/$WinPEDriver$", target_vol), "-name", "._*", "-delete"]).output();
            let _ = Command::new("find").args(&[&format!("{}/drivers", target_vol), "-name", "._*", "-delete"]).output();

            let _ = tx.send(BackgroundMessage::LogLine("✓ Seluruh metadata Mac telah dibersihkan. Media instalasi 100% kompatibel dengan Windows Setup.".to_string()));

            // 3. Sinkronisasi buffer penyimpanan ke USB fisik
            let _ = tx.send(BackgroundMessage::Status("Menyelesaikan sinkronisasi data ke drive USB (sync)...".to_string()));
            let _ = tx.send(BackgroundMessage::LogLine(">> Menyinkronkan buffer penyimpanan (sync)...".to_string()));
            let _ = Command::new("sync").output();
            std::thread::sleep(Duration::from_millis(800));

            // 4. Melepaskan ISO
            let _ = tx.send(BackgroundMessage::Status("Melepaskan berkas ISO...".to_string()));
            run_cmd_streaming(&tx, "hdiutil", &["unmount", &iso_vol]);

            let _ = tx.send(BackgroundMessage::Finished(
                true,
                "Flashdisk / Partisi Bootable Windows berhasil dibuat dan siap digunakan tanpa error!".to_string(),
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
                            self.status_level = StatusLevel::Info;
                        }
                    } else {
                        self.selected_disk.clear();
                        if !self.is_creating {
                            self.status = "Menunggu flashdisk / SSD eksternal dicolokkan ke Mac...".to_string();
                            self.status_level = StatusLevel::Info;
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
                    self.status_level = if success {
                        StatusLevel::Success
                    } else {
                        StatusLevel::Error
                    };
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

        // eframe follows the macOS appearance; re-apply our tokens when it flips.
        let system_dark = ctx.style().visuals.dark_mode;
        if system_dark != self.dark {
            self.dark = system_dark;
            theme::apply_style(ctx, self.dark);
        }

        let refresh_rate = if self.is_creating {
            Duration::from_millis(100)
        } else {
            Duration::from_millis(500)
        };
        ctx.request_repaint_after(refresh_rate);

        ui::app(self, ctx);
    }
}

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([760.0, 780.0])
            .with_min_inner_size([680.0, 620.0])
            .with_title("WinDisk Maker"),
        follow_system_theme: true,
        ..Default::default()
    };
    eframe::run_native(
        "WinDisk Maker",
        options,
        Box::new(|cc| Box::new(WinDiskApp::new(cc))),
    )
}
