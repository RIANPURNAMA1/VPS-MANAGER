# VPS Manager

Aplikasi desktop Windows untuk mengelola VPS Ubuntu Server 24.04 lewat SSH/SFTP — tanpa agen di server, tanpa backend publik, tanpa Docker.

**Stack:** Tauri 2 · Rust · React 18 · TypeScript (strict) · Vite · Tailwind CSS 4 · SQLite (metadata lokal) · ssh2/libssh2 · Windows Credential Manager · Ollama (opsional).

Desain: monokrom hitam-putih (Light / Dark / System), tanpa warna aksen. Status dibedakan lewat ikon, label, dan gaya garis (solid / putus-putus / titik-titik).

## Fitur

| Halaman | Isi |
|---|---|
| Overview | Jumlah server, status SSH, CPU, RAM, disk, uptime, load, jaringan, grafik, aktivitas terbaru. Data nyata dari `/proc` & `df`; tanpa data dummy; ada indikator *stale*. |
| Servers | Tambah/ubah/hapus profil, SSH key + passphrase, password (opsional), Test Connection, Connect/Disconnect, verifikasi host key. |
| Websites | Domain & root dari `nginx -T` (hanya-baca), cek HTTP/HTTPS, masa berlaku SSL, `nginx -t`, reload Nginx (setelah validasi + konfirmasi). |
| Services | Deteksi Nginx, MySQL/MariaDB, PHP-FPM (nama service sesuai versi), SSH, PM2; daftar proses; log `journalctl`; restart/reload dengan konfirmasi dan verifikasi `is-active`. |
| Terminal | PTY interaktif (xterm.js), scrollback, copy/paste, clear, reconnect, konfirmasi sebelum perintah destruktif. |
| File Manager | SFTP: browse, upload/download dengan progress, rename, mkdir, delete (file / folder kosong), edit teks (≤ 512 KB), izin file. |
| Databases | Daftar DB MySQL, backup (`mysqldump` + gzip), verifikasi (`gzip -t`, penanda *Dump completed*, sha256), unduh ke Windows, riwayat, restore dengan konfirmasi mengetik nama DB. |
| Deployments | Proyek Git: status, fetch, deploy fast-forward, langkah build, restart service, verifikasi URL, riwayat + log, rollback. |
| AI Assistant | Ollama lokal: analisis log (disanitasi), saran perintah dengan klasifikasi risiko. AI tidak mengeksekusi apa pun sendiri. |
| Activity Logs | Waktu, server, aksi, status, exit code, durasi, ringkasan (disamarkan). Retensi dapat diatur. |
| Settings | Tema, interval monitoring, timeout SSH, folder unduhan, batas transfer, Ollama, retensi log, host terpercaya. |

## Prasyarat (Windows 10/11 64-bit)

1. **Node.js 20+** dan npm.
2. **Rust ≥ 1.90** (`rustup default stable`) dengan target `x86_64-pc-windows-msvc`.
3. **Visual Studio Build Tools 2022** → workload *Desktop development with C++*.
4. **WebView2 Runtime** (sudah ada di Windows 11).
5. **Perl** di `PATH` (mis. Strawberry Perl, atau `C:\Program Files\Git\usr\bin` dari Git for Windows). Dibutuhkan karena di Windows `ssh2` dibangun dengan OpenSSL *vendored* agar kunci `ed25519` / format OpenSSH didukung (backend WinCNG bawaan libssh2 tidak mendukungnya). Build pertama beberapa menit lebih lama.

## Menjalankan

```powershell
npm install
npm run tauri dev        # mode development
npm run tauri build      # build release + installer (NSIS .exe dan MSI) di src-tauri\target\release\bundle\
```

Pemeriksaan kualitas:

```powershell
npm run typecheck && npm run lint && npm test      # frontend
npm run check:rust && npm run test:rust            # backend
```

Data lokal (SQLite) ada di `%APPDATA%\id.vpsmanager.app\vps-manager.sqlite3`.

## Menyiapkan server

* **Autentikasi:** SSH key disarankan. Pasang public key di `~/.ssh/authorized_keys` user Anda.
* **Operasi administratif** (restart service, `nginx -T`, `journalctl` unit lain, mysqldump via socket): aktifkan opsi *sudo* pada profil. Aplikasi hanya memakai `sudo -n` (tidak pernah meminta password), jadi user perlu `NOPASSWD` untuk perintah yang dibutuhkan. Contoh minimal di `/etc/sudoers.d/vps-manager`:
  ```
  ubuntu ALL=(root) NOPASSWD: /usr/bin/systemctl restart *, /usr/bin/systemctl reload *, /usr/sbin/nginx, /usr/bin/journalctl, /usr/bin/openssl, /usr/bin/mysql, /usr/bin/mysqldump, /usr/bin/bash
  ```
  (`bash` dibutuhkan karena skrip dibungkus `bash -c`; persempit sesuai kebijakan Anda, atau jalankan sebagai root.)
* **MySQL/MariaDB:** tanpa password di command line. Pakai socket auth lewat sudo, atau isi *File defaults MySQL* di profil (mis. `/home/ubuntu/.my.cnf` berisi `[client] user=… password=…`, `chmod 600`).
* **Deployment:** repository harus sudah di-clone di server dengan akses `git fetch` (deploy key / agent) untuk user SSH.

## Keamanan

* Verifikasi host key **tidak bisa dimatikan**: host baru butuh persetujuan fingerprint; host key yang berubah memblokir koneksi.
* Password/passphrase di Windows Credential Manager; tidak pernah di SQLite, log, atau dikirim balik ke UI. Private key hanya dibaca dari path pilihan Anda.
* Semua input divalidasi di Rust; argumen shell di-quote; nama service/DB/branch/commit memakai allowlist regex; path remote dinormalisasi (absolut, tanpa `..`), path sistem tidak bisa dihapus/rename.
* Operasi destruktif (hapus, timpa, restore, restart, deploy, rollback) mewajibkan konfirmasi yang juga diperiksa ulang di backend (`confirmed=true`).
* Output perintah dibatasi (2 MiB), transfer dibatasi (default 1 GiB), semua operasi punya timeout.
* Log aktivitas dan konteks AI disamarkan (password, token, private key, kredensial URL).
* AI: hanya Ollama di loopback; perintah usulan diklasifikasi `safe` / `caution` / `blocked`; `blocked` tidak pernah dijalankan.
* Tauri capability minimal: `core:default` + dialog open/save. CSP ketat.
* Tidak ada pembuatan akun root, perubahan firewall, atau konfigurasi VPS otomatis. Deploy tidak memakai `git reset --hard`.

## Struktur

```
src/                    React + TypeScript (features/ per halaman, components/, lib/, hooks/, types/)
src-tauri/src/
  commands/             lapisan perintah Tauri (tipis; validasi + audit log)
  ssh/                  sesi SSH, host key, exec, SFTP
  monitoring/ services/ website/ files/ terminal/ backup/ deployment/ ai/
  security/             validasi, redaksi, klasifikasi risiko, credential store
  storage/              SQLite + migrasi, profil, pengaturan, log
tests/                  vitest (format, tema, aturan monokrom)
```

## Batasan yang diketahui (jujur)

* Aplikasi ini **dikompilasi dan diuji unit di Linux**; belum pernah dijalankan di Windows, belum diuji terhadap VPS sungguhan, dan belum ada integration test SSH/SFTP end-to-end. Parser (metrik, service, Nginx, Git), validasi, redaksi, klasifikasi risiko, SQLite, dan helper UI punya unit test. Alur SSH/SFTP/terminal/backup/deploy **perlu Anda uji pada VPS non-produksi** sebelum dipakai di produksi.
* Windows Credential Manager (`keyring`, `windows-native`) belum diuji di Windows.
* Konfirmasi perintah destruktif di terminal bersifat *best-effort* (membaca baris yang diketik; riwayat panah/tab-completion tidak terlihat). Itu pengaman tambahan, bukan sandbox.
* Pembatalan (cancel) operasi panjang belum ada; hanya timeout. Unduh folder dan hapus folder berisi (rekursif) belum didukung.
* Network traffic ditampilkan sebagai laju sesaat (selisih 1 detik) dan total counter; bukan riwayat jangka panjang.
* Ikon aplikasi dibuat otomatis (placeholder monokrom); ganti dengan `npx tauri icon <logo.png>`.
