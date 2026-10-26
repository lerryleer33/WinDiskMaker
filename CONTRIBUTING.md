# Panduan Berkontribusi pada WinDisk Maker

Terima kasih telah meluangkan waktu untuk berkontribusi! Proyek ini terbuka untuk siapa saja yang ingin membantu meningkatkan fungsionalitas, keamanan, dan estetika **WinDisk Maker**.

---

## 🛠️ Alur Kontribusi

1. **Fork Repositori**: Buat salinan repositori ini ke akun GitHub Anda.
2. **Buat Branch Fitur**:
   ```bash
   git checkout -b feature/nama-fitur-baru
   ```
   atau untuk perbaikan bug:
   ```bash
   git checkout -b fix/deskripsi-masalah
   ```
3. **Lakukan Perubahan**:
   - Tulis kode yang rapi dan patuhi standar keamanan Rust.
   - Hindari penggunaan perintah berbahaya yang dapat memengaruhi disk internal pengguna.
4. **Validasi Kompilasi**:
   Pastikan tidak ada error atau warning sebelum commit:
   ```bash
   cargo check
   cargo build --release
   ```
5. **Commit Perubahan**:
   Gunakan pesan commit yang jelas dan deskriptif, contoh:
   ```bash
   git commit -m "feat: tambahkan dukungan driver tambahan untuk chip Intel generasi baru"
   ```
6. **Push dan Buka Pull Request**:
   Push branch ke fork Anda dan buat Pull Request ke branch `main` repositori resmi.

---

## 💡 Melaporkan Masalah (Bug Reports)

Jika Anda menemukan bug:
- Buka [GitHub Issues](https://github.com/lerryleer33/WinDiskMaker/issues).
- Jelaskan versi macOS yang digunakan (Intel atau Apple Silicon).
- Sertakan log keluaran dari panel log konsol aplikasi jika terjadi kegagalan.

Terima kasih atas kontribusi Anda dalam membuat WinDisk Maker semakin baik!
