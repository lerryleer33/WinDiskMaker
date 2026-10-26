# WinDisk Maker — UI Redesign

**Tanggal:** 27 Oktober 2026
**Status:** Disetujui
**Ruang lingkup:** Lapisan presentasi egui. Logika pembuatan USB tidak berubah.

## Tujuan

Mengganti tampilan yang terasa seperti template AI (emoji, badge neon, banyak
aksen warna, kartu ber-border tebal) dengan antarmuka yang terasa seperti utilitas
native macOS: tenang, padat, satu aksen warna, dan hierarki lewat tipografi serta
separator.

Kriteria berhasil:

- Tidak ada emoji di UI.
- Maksimal satu aksen dekoratif (system blue); warna semantik hanya untuk status
  keamanan, peringatan, dan error.
- Seluruh alur lama tetap berfungsi: pilih ISO, deteksi USB, mode partisi aman,
  full disk wipe, progres 6 langkah, log streaming, konfirmasi sebelum format.
- Tema terang dan gelap mengikuti pengaturan macOS.

## Non-goals

- Tidak mengubah perintah `diskutil`, `rsync`, `wimlib-imagex`, atau alur worker.
- Tidak menambah sidebar, wizard multi-layar, atau custom titlebar.
- Tidak menambah dependency baru.
- Tidak menambah fitur pembatalan proses atau pemilihan filesystem.

## Sistem visual

### Warna

| Token | Light | Dark |
|---|---|---|
| window | `#F5F5F7` | `#1C1C1E` |
| surface | `#FFFFFF` | `#2C2C2E` |
| inset | `#F2F2F4` | `#232325` |
| separator | hitam 9% | putih 10% |
| text | `#1D1D1F` | `#F5F5F7` |
| text secondary | `#6E6E73` | `#98989D` |
| text tertiary | `#9A9AA0` | `#636366` |
| accent | `#007AFF` | `#0A84FF` |
| success | `#248A3D` | `#30D158` |
| warning | `#B25000` | `#FF9F0A` |
| danger | `#D70015` | `#FF453A` |

Accent dipakai untuk tombol utama, seleksi baris, progres, dan link. Success /
warning / danger hanya untuk chip keamanan, status, dan aksi destruktif.

### Tipografi

- Body: SF Pro dimuat runtime dari `/System/Library/Fonts/SFNS.ttf`.
- Judul/emphasis: Helvetica Neue Medium dari `HelveticaNeue.ttc` (index 10),
  karena egui 0.27 tidak memiliki bold sintetis dan SF Pro di sistem berupa
  variable font.
- Mono: SF Mono dari `/System/Library/Fonts/SFNSMono.ttf`.
- Fallback berurutan ke font sistem egui bila berkas tidak tersedia.
- Skala: 20 (judul app), 13 (judul grup/body), 11.5 (caption/meta), 11.5 mono (log).

### Spacing & bentuk

- Grid 4pt. Padding halaman 20, padding grup 16, jarak antar-grup 20.
- Tinggi baris 44–56. Kontrol 32. Radius: kontrol 6, grup 10, modal 12.
- Grup memakai satu permukaan `surface` dengan separator inset 16px; bukan kartu
  ber-border tebal per elemen.
- Tanpa gradient, glow, atau shadow dekoratif. Shadow hanya pada modal.

## Layout

Satu jendela 760×780 (minimum 680×620), tiga area:

1. **Header tetap** — app mark 30px, judul, subtitle, status dot + label, versi.
2. **Konten scroll** — tiga grup form:
   - `Sumber instalasi`: baris ISO (nama, ukuran, path, tombol Pilih/Ganti).
   - `Tujuan`: daftar drive/partisi sebagai baris radio (bukan tombol emoji +
     dropdown duplikat). Partisi aman di atas, whole disk di bawah dengan chip
     `Hapus semua`. Di bawahnya ringkasan mode, filesystem, dan label.
   - `Opsi`: dua baris switch (Driver NVMe/Intel VMD, Bypass Windows 11) dengan
     deskripsi satu baris.
3. **Action bar tetap** — status di kiri; tombol sekunder `Lihat Log` dan primer
   `Buat USB Bootable` di kanan. Saat proses berjalan, tombol diganti peringatan
   `Jangan cabut drive`.

### Mode proses

Saat `is_creating`, konten form diganti tampilan progres: judul, status, progress
bar 4px, stepper 6 segmen (Format, ISO, Salin, WIM, Patch, Selesai), ringkasan
sumber → target, dan log mono yang selalu terbuka. Form tidak dapat diklik.

### Modal konfirmasi

Scrim 40%, panel 440px di tengah. Judul & konsekuensi sesuai mode. Ringkasan
target di inset. Tombol `Batal` (sekunder) dan konfirmasi: biru untuk partisi,
merah untuk full disk. `Esc` menutup modal.

## Komponen

- `group`, `row`, `separator` — permukaan dan pemisah.
- `section_label`, `caption`, `status` — tipografi.
- `primary_button`, `secondary_button`, `plain_button`, `danger_button` — digambar
  manual agar state hover/pressed/disabled konsisten.
- `switch` — track 38×22, knob 16, animasi 120ms.
- `radio` + baris drive — seluruh baris dapat diklik, chip status di kanan.
- `progress_bar` + `stepper`.
- `modal` — scrim + panel.
- Ikon vector 1.5px: file, disk, chevron, check, warning. Tidak memakai emoji.

## Perilaku

- Semua state dan worker lama dipertahankan.
- Dropdown pemilihan drive dihapus karena baris radio sudah mencakup fungsi yang
  sama; `selected_disk` tetap diisi otomatis dengan perangkat pertama.
- Tambahan: tombol `Salin` untuk log, `Esc` untuk modal, dan `status_level`
  (Info/Success/Error) untuk pewarnaan status.
- Deteksi USB tetap berjalan di background setiap 1.5s.

## Arsitektur

- `src/theme.rs` — palet, font, style egui, konstanta spacing.
- `src/widgets.rs` — komponen visual murni.
- `src/ui.rs` — komposisi header, form, progres, footer, modal.
- `src/main.rs` — struct app, scanning, worker, dan entry point. Field yang
  dibutuhkan UI menjadi `pub(crate)`.

## Verifikasi

1. `cargo build --release` tanpa warning baru.
2. Jalankan app; periksa tema terang dan gelap.
3. Uji: tanpa drive, tanpa ISO, ISO terpilih, partisi terpilih, full disk terpilih,
   modal konfirmasi, dan tampilan progres (dengan target aman).
4. Pastikan dialog sistem (file picker) dan log streaming tetap berjalan.
