//! Pembangkit kode LLVM IR untuk bahasa Nusa.
//!
//! Semua nilai disimpan di alloca (gaya mem2reg), sehingga LLVM dapat
//! mengangkatnya menjadi register pada tahap optimisasi.

use crate::ast::*;
use crate::diag::{Diag, Hasil};
use crate::types::Info;
use std::collections::HashMap;
use std::fmt::Write as _;

pub struct Opsi {
    /// Periksa luapan aritmetika bilangan bulat saat berjalan.
    pub aman: bool,
    /// Sertakan pemeriksaan `syarat`/`jamin`/`tegaskan` saat berjalan.
    pub kontrak: bool,
}

pub struct Keluaran {
    pub ir: String,
}

pub const RUNTIME_C: &str = include_str!("runtime/nusa_rt.c");

const DEKLARASI_RUNTIME: &str = r#"
declare void @nusa_tahan(ptr)
declare void @nusa_lepas_teks(ptr)
declare void @nusa_lepas_daftar(ptr)
declare void @nusa_panik(ptr, i64)
declare ptr @nusa_teks_gabung(ptr, ptr)
declare i8 @nusa_teks_sama(ptr, ptr)
declare i64 @nusa_teks_banding(ptr, ptr)
declare i64 @nusa_teks_panjang(ptr)
declare void @nusa_cetak_teks(ptr)
declare void @nusa_cetak_bil(i64)
declare void @nusa_cetak_des(double)
declare void @nusa_cetak_log(i8)
declare ptr @nusa_daftar_baru(i64, i64)
declare ptr @nusa_daftar_tambah(ptr, i64)
declare ptr @nusa_daftar_isi(ptr, i64, i64)
declare i64 @nusa_daftar_ambil(ptr, i64)
declare i64 @nusa_daftar_panjang(ptr)
declare i64 @nusa_bagi(i64, i64)
declare i64 @nusa_mod(i64, i64)
declare i64 @nusa_mutlak_bil(i64)
declare { i64, i1 } @llvm.sadd.with.overflow.i64(i64, i64)
declare { i64, i1 } @llvm.ssub.with.overflow.i64(i64, i64)
declare { i64, i1 } @llvm.smul.with.overflow.i64(i64, i64)
declare double @llvm.fabs.f64(double)
"#;

pub fn bangkitkan(program: &Program, info: &Info, opsi: Opsi) -> Hasil<Keluaran> {
    let mut g = Gen {
        info,
        opsi,
        globals: Vec::new(),
        intern: HashMap::new(),
        intern_pesan: HashMap::new(),
        n_str: 0,
        n_pesan: 0,
        fungsi: Vec::new(),
    };
    g.globals
        .push(format!("; Nusa v0.1 -- {} fungsi", program.fungsi.len()));
    for f in &program.fungsi {
        let ir = g.fungsi(f)?;
        g.fungsi.push(ir);
    }
    g.fungsi
        .push("define i32 @main() {\nentry:\n  call void @utama()\n  ret i32 0\n}\n".to_string());

    let mut ir = String::new();
    ir.push_str("; Modul LLVM IR yang dihasilkan kompiler Nusa (.ns)\n");
    ir.push_str("; Aman memori: tanpa pointer pengguna, batas daftar selalu diperiksa.\n\n");
    ir.push_str(DEKLARASI_RUNTIME);
    ir.push('\n');
    for g_ in &g.globals {
        ir.push_str(g_);
        ir.push('\n');
    }
    ir.push('\n');
    for f in &g.fungsi {
        ir.push_str(f);
        ir.push('\n');
    }
    Ok(Keluaran { ir })
}

fn tipe_ir(t: &Tipe) -> &'static str {
    match t {
        Tipe::Bil => "i64",
        Tipe::Des => "double",
        Tipe::Log => "i1",
        Tipe::Teks => "ptr",
        Tipe::Daftar(_) => "ptr",
        Tipe::Kosong => "void",
    }
}

fn jenis_elemen(t: &Tipe) -> i64 {
    match t {
        Tipe::Teks => 1,
        Tipe::Daftar(_) => 2,
        _ => 0,
    }
}

fn fmt_des(n: f64) -> String {
    if n.is_nan() {
        return "0x7FF8000000000000".into();
    }
    if n.is_infinite() {
        return if n > 0.0 {
            "0x7FF0000000000000"
        } else {
            "0xFFF0000000000000"
        }
        .into();
    }
    let s = format!("{:?}", n);
    if let Some(pos) = s.find(['e', 'E']) {
        if !s[..pos].contains('.') {
            return format!("{}.0{}", &s[..pos], &s[pos..]);
        }
        return s;
    }
    if s.contains('.') {
        s
    } else {
        format!("{}.0", s)
    }
}

fn escape_ir_bytes(bytes: &[u8]) -> String {
    let mut s = String::new();
    for &b in bytes {
        match b {
            b'"' => s.push_str("\\\""),
            b'\\' => s.push_str("\\\\"),
            0x20..=0x7e => s.push(b as char),
            _ => {
                let _ = write!(s, "\\{:02X}", b);
            }
        }
    }
    s
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Milik {
    Pinjam,
    Milik,
}

#[derive(Clone)]
struct Nilai {
    repr: String,
    tipe: Tipe,
    milik: Milik,
}

impl Nilai {
    fn skalar(repr: String, tipe: Tipe) -> Self {
        Nilai {
            repr,
            tipe,
            milik: Milik::Pinjam,
        }
    }
    fn kosong() -> Self {
        Nilai {
            repr: String::new(),
            tipe: Tipe::Kosong,
            milik: Milik::Pinjam,
        }
    }
}

#[derive(Clone)]
struct Slot {
    ir: String,
    tipe: Tipe,
}

struct LoopInfo {
    dasar: usize,
    lanjut: String,
    keluar: String,
}

struct Gen<'a> {
    info: &'a Info,
    opsi: Opsi,
    globals: Vec<String>,
    intern: HashMap<String, String>,
    intern_pesan: HashMap<String, String>,
    n_str: usize,
    n_pesan: usize,
    fungsi: Vec<String>,
}

impl<'a> Gen<'a> {
    fn intern(&mut self, isi: &str) -> String {
        if let Some(n) = self.intern.get(isi) {
            return n.clone();
        }
        let nama = format!("@.nusa.str.{}", self.n_str);
        self.n_str += 1;
        let bytes = isi.as_bytes();
        let isi_ir = escape_ir_bytes(bytes);
        let inisialisasi = if bytes.is_empty() {
            "[0 x i8] zeroinitializer".to_string()
        } else {
            format!("[{} x i8] c\"{}\"", bytes.len(), isi_ir)
        };
        self.globals.push(format!(
            "{} = private unnamed_addr constant {{ i64, i64, [{} x i8] }} {{ i64 {}, i64 -1, {} }}",
            nama,
            bytes.len(),
            bytes.len(),
            inisialisasi
        ));
        self.intern.insert(isi.to_string(), nama.clone());
        nama
    }

    fn fungsi(&mut self, f: &Fungsi) -> Hasil<String> {
        let mut fg = FnGen {
            g: self,
            teks: String::new(),
            awal: Vec::new(),
            terminal: false,
            n_label: 1,
            mulai: "%L0".to_string(),
            n_temp: 0,
            n_alloca: 0,
            lingkup: vec![HashMap::new()],
            heap_slots: Vec::new(),
            loops: Vec::new(),
            temps: Vec::new(),
            hasil_slot: None,
            akhir: "%selesai".to_string(),
        };
        fg.badan(f)?;
        Ok(fg.rakit(f))
    }
}

struct FnGen<'g, 'a> {
    g: &'g mut Gen<'a>,
    teks: String,
    awal: Vec<String>,
    terminal: bool,
    n_label: usize,
    n_temp: usize,
    n_alloca: usize,
    mulai: String,
    lingkup: Vec<HashMap<String, Slot>>,
    heap_slots: Vec<(String, Tipe)>,
    loops: Vec<LoopInfo>,
    temps: Vec<(String, Tipe)>,
    hasil_slot: Option<String>,
    akhir: String,
}

impl<'g, 'a> FnGen<'g, 'a> {
    fn t(&mut self) -> String {
        let s = format!("%t{}", self.n_temp);
        self.n_temp += 1;
        s
    }

    fn label(&mut self) -> String {
        let s = format!("%L{}", self.n_label);
        self.n_label += 1;
        s
    }

    fn masukkan(&mut self, baris: impl AsRef<str>) {
        if self.terminal {
            return;
        }
        let _ = writeln!(self.teks, "  {}", baris.as_ref());
    }

    fn br(&mut self, label: &str) {
        self.masukkan(format!("br label {}", label));
        self.terminal = true;
    }

    fn br_kondisi(&mut self, kondisi: &str, benar: &str, salah: &str) {
        self.masukkan(format!(
            "br i1 {}, label {}, label {}",
            kondisi, benar, salah
        ));
        self.terminal = true;
    }

    fn mulai(&mut self, label: &str) {
        let definisi = label.strip_prefix('%').unwrap_or(label);
        let _ = writeln!(self.teks, "\n{}:", definisi);
        self.terminal = false;
    }

    fn tutup_ke(&mut self, label: &str) {
        if !self.terminal {
            self.br(label);
        }
    }

    fn alokasi(&mut self, tipe: &Tipe) -> String {
        let nama = format!("%a{}", self.n_alloca);
        self.n_alloca += 1;
        self.awal
            .push(format!("  {} = alloca {}", nama, tipe_ir(tipe)));
        nama
    }

    fn muat(&mut self, slot: &str, tipe: &Tipe) -> String {
        let t = self.t();
        self.masukkan(format!("{} = load {}, ptr {}", t, tipe_ir(tipe), slot));
        t
    }

    fn simpan(&mut self, nilai: &str, slot: &str, tipe: &Tipe) {
        self.masukkan(format!("store {} {}, ptr {}", tipe_ir(tipe), nilai, slot));
    }

    fn deklarasikan(&mut self, nama: &str, slot: String, tipe: Tipe) {
        self.lingkup
            .last_mut()
            .unwrap()
            .insert(nama.to_string(), Slot { ir: slot, tipe });
    }

    fn cari_slot(&self, nama: &str) -> Hasil<Slot> {
        self.lingkup
            .iter()
            .rev()
            .find_map(|l| l.get(nama).cloned())
            .ok_or_else(|| {
                Diag::baru(
                    "NUSA-C001",
                    format!("kesalahan internal: slot `{}` tidak ditemukan", nama),
                )
            })
    }

    fn daftarkan_heap(&mut self, slot: &str, tipe: &Tipe) {
        self.awal.push(format!("  store ptr null, ptr {}", slot));
        self.heap_slots.push((slot.to_string(), tipe.clone()));
    }

    fn lepas_slot(&mut self, slot: &str, tipe: &Tipe) {
        let v = self.muat(slot, tipe);
        let fungsi = match tipe {
            Tipe::Teks => "@nusa_lepas_teks",
            Tipe::Daftar(_) => "@nusa_lepas_daftar",
            _ => return,
        };
        self.masukkan(format!("call void {}(ptr {})", fungsi, v));
        self.simpan("null", slot, tipe);
    }

    fn lepas_temps(&mut self) {
        let temps = std::mem::take(&mut self.temps);
        for (nama, tipe) in temps {
            let fungsi = match tipe {
                Tipe::Teks => "@nusa_lepas_teks",
                Tipe::Daftar(_) => "@nusa_lepas_daftar",
                _ => continue,
            };
            self.masukkan(format!("call void {}(ptr {})", fungsi, nama));
        }
    }

    fn ambil_milik(&mut self, v: Nilai) -> String {
        if !v.tipe.adalah_heap() {
            return v.repr;
        }
        match v.milik {
            Milik::Milik => {
                if let Some(p) = self.temps.iter().rposition(|(n, _)| n == &v.repr) {
                    self.temps.remove(p);
                }
                v.repr
            }
            Milik::Pinjam => {
                self.masukkan(format!("call void @nusa_tahan(ptr {})", v.repr));
                v.repr
            }
        }
    }

    fn milik_baru(&mut self, repr: String, tipe: Tipe) -> Nilai {
        self.temps.push((repr.clone(), tipe.clone()));
        Nilai {
            repr,
            tipe,
            milik: Milik::Milik,
        }
    }

    fn pesan(&mut self, teks: &str) -> String {
        if let Some(n) = self.g.intern_pesan.get(teks) {
            let len = teks.len();
            return format!("ptr {}, i64 {}", n, len);
        }
        let nama = format!("@.nusa.pesan.{}", self.g.n_pesan);
        self.g.n_pesan += 1;
        let bytes = teks.as_bytes();
        let inisialisasi = if bytes.is_empty() {
            "zeroinitializer".to_string()
        } else {
            format!("c\"{}\"", escape_ir_bytes(bytes))
        };
        self.g.globals.push(format!(
            "{} = private unnamed_addr constant [{} x i8] {}",
            nama,
            bytes.len(),
            inisialisasi
        ));
        self.g.intern_pesan.insert(teks.to_string(), nama.clone());
        format!("ptr {}, i64 {}", nama, bytes.len())
    }

    fn panik_jika(&mut self, kondisi: &str, pesan: &str) {
        let p = self.label();
        let l = self.label();
        self.br_kondisi(kondisi, &p, &l);
        self.mulai(&p);
        let arg = self.pesan(pesan);
        self.masukkan(format!("call void @nusa_panik({})", arg));
        self.masukkan("unreachable");
        self.terminal = true;
        self.mulai(&l);
    }

    fn cek_benar(&mut self, kondisi: &str, pesan: &str) {
        let n = self.t();
        self.masukkan(format!("{} = xor i1 {}, true", n, kondisi));
        self.panik_jika(&n, pesan);
    }

    fn badan(&mut self, f: &Fungsi) -> Hasil<()> {
        for (i, p) in f.params.iter().enumerate() {
            let slot = self.alokasi(&p.tipe);
            let arg = format!("%p{}", i);
            self.awal.push(format!(
                "  store {} {}, ptr {}",
                tipe_ir(&p.tipe),
                arg,
                slot
            ));
            self.deklarasikan(&p.nama, slot.clone(), p.tipe.clone());
            if p.tipe.adalah_heap() {
                self.heap_slots.push((slot, p.tipe.clone()));
            }
        }
        if f.hasil != Tipe::Kosong {
            let slot = self.alokasi(&f.hasil);
            self.hasil_slot = Some(slot);
        }
        let l0 = self.mulai.clone();
        self.mulai(&l0);

        if self.g.opsi.kontrak {
            for s in &f.syarat {
                let induk = std::mem::take(&mut self.temps);
                let v = self.gen_expr(s)?;
                let p = format!("syarat fungsi `{}` gagal: {}", f.nama, cetak_ekspr(s));
                self.cek_benar(&v.repr, &p);
                self.lepas_temps();
                self.temps = induk;
            }
        }

        self.blok(&f.tubuh)?;
        let akhir = self.akhir.clone();
        self.tutup_ke(&akhir);
        self.mulai(&akhir);

        if self.g.opsi.kontrak && !f.jamin.is_empty() {
            let hasil_slot = self.hasil_slot.clone();
            if let Some(hs) = &hasil_slot {
                self.lingkup[0].insert(
                    "hasil".into(),
                    Slot {
                        ir: hs.clone(),
                        tipe: f.hasil.clone(),
                    },
                );
            }
            for j in &f.jamin {
                let induk = std::mem::take(&mut self.temps);
                let v = self.gen_expr(j)?;
                let p = format!("jamin fungsi `{}` gagal: {}", f.nama, cetak_ekspr(j));
                self.cek_benar(&v.repr, &p);
                self.lepas_temps();
                self.temps = induk;
            }
            self.lingkup[0].remove("hasil");
        }

        let heap = self.heap_slots.clone();
        for (slot, tipe) in &heap {
            self.lepas_slot(slot, tipe);
        }

        if f.hasil == Tipe::Kosong {
            self.masukkan("ret void");
        } else {
            let hs = self.hasil_slot.clone().unwrap();
            let v = self.muat(&hs, &f.hasil);
            self.masukkan(format!("ret {} {}", tipe_ir(&f.hasil), v));
        }
        self.terminal = true;
        Ok(())
    }

    fn rakit(&self, f: &Fungsi) -> String {
        let params: Vec<String> = f
            .params
            .iter()
            .enumerate()
            .map(|(i, p)| format!("{} %p{}", tipe_ir(&p.tipe), i))
            .collect();
        let mut s = String::new();
        let _ = writeln!(
            s,
            "define {} @{}({}) {{",
            tipe_ir(&f.hasil),
            f.nama,
            params.join(", ")
        );
        s.push_str("entry:\n");
        for baris in &self.awal {
            s.push_str(baris);
            s.push('\n');
        }
        let _ = writeln!(s, "  br label {}", self.mulai);
        s.push_str(&self.teks);
        s.push_str("}\n");
        s
    }

    fn blok(&mut self, stmts: &[Stmt]) -> Hasil<()> {
        self.lingkup.push(HashMap::new());
        for s in stmts {
            self.pernyataan(s)?;
        }
        self.tutup_lingkup();
        Ok(())
    }

    fn tutup_lingkup(&mut self) {
        if let Some(l) = self.lingkup.pop() {
            if !self.terminal {
                let isi: Vec<Slot> = l.into_values().collect();
                for slot in &isi {
                    if slot.tipe.adalah_heap() {
                        self.lepas_slot(&slot.ir, &slot.tipe);
                    }
                }
            }
        }
    }

    fn drop_dari(&mut self, dasar: usize) {
        if self.terminal {
            return;
        }
        let mut daftar = Vec::new();
        for l in &self.lingkup[dasar.min(self.lingkup.len())..] {
            for slot in l.values() {
                if slot.tipe.adalah_heap() {
                    daftar.push((slot.ir.clone(), slot.tipe.clone()));
                }
            }
        }
        for (slot, tipe) in daftar {
            self.lepas_slot(&slot, &tipe);
        }
    }

    fn pernyataan(&mut self, s: &Stmt) -> Hasil<()> {
        let induk = std::mem::take(&mut self.temps);
        let hasil = self.pernyataan_inti(s);
        if !self.terminal {
            self.lepas_temps();
        }
        self.temps = induk;
        hasil
    }

    fn pernyataan_inti(&mut self, s: &Stmt) -> Hasil<()> {
        match s {
            Stmt::Tetap { nama, nilai, .. } | Stmt::Var { nama, nilai, .. } => {
                let v = self.gen_expr(nilai)?;
                let tipe = v.tipe.clone();
                let repr = self.ambil_milik(v);
                let slot = self.alokasi(&tipe);
                if tipe.adalah_heap() {
                    self.daftarkan_heap(&slot, &tipe);
                }
                self.deklarasikan(nama, slot.clone(), tipe.clone());
                self.simpan(&repr, &slot, &tipe);
                Ok(())
            }
            Stmt::Tugas {
                target, op, nilai, ..
            } => match &target.kind {
                ExprKind::Nama(nama) => self.tugas_nama(nama, *op, nilai),
                ExprKind::Indeks(base, indeks) => {
                    let nama = match &base.kind {
                        ExprKind::Nama(n) => n.clone(),
                        _ => {
                            return Err(Diag::baru(
                                "NUSA-C002",
                                "kesalahan internal: penugasan indeks bukan variabel",
                            ))
                        }
                    };
                    self.tugas_indeks(&nama, indeks, *op, nilai)
                }
                _ => Err(Diag::baru(
                    "NUSA-C003",
                    "kesalahan internal: target penugasan tidak sah",
                )),
            },
            Stmt::Ekspresi(e) => {
                self.gen_expr(e)?;
                Ok(())
            }
            Stmt::Jika {
                kondisi,
                maka,
                lainnya,
                ..
            } => {
                let k = self.gen_expr(kondisi)?;
                let benar = self.label();
                let salah = self.label();
                let gabung = self.label();
                self.br_kondisi(&k.repr, &benar, &salah);
                self.mulai(&benar);
                self.blok(maka)?;
                self.tutup_ke(&gabung);
                self.mulai(&salah);
                if let Some(l) = lainnya {
                    self.blok(l)?;
                }
                self.tutup_ke(&gabung);
                self.mulai(&gabung);
                Ok(())
            }
            Stmt::Selama { kondisi, tubuh, .. } => {
                let cond = self.label();
                let body = self.label();
                let setelah = self.label();
                self.tutup_ke(&cond);
                self.mulai(&cond);
                let k = self.gen_expr(kondisi)?;
                self.br_kondisi(&k.repr, &body, &setelah);
                self.mulai(&body);
                self.loops.push(LoopInfo {
                    dasar: self.lingkup.len(),
                    lanjut: cond.clone(),
                    keluar: setelah.clone(),
                });
                self.blok(tubuh)?;
                self.loops.pop();
                self.tutup_ke(&cond);
                self.mulai(&setelah);
                Ok(())
            }
            Stmt::Untuk {
                nama,
                iterasi,
                tubuh,
                ..
            } => match iterasi {
                Iterasi::Rentang {
                    mulai,
                    akhir,
                    inklusif,
                } => self.untuk_rentang(nama, mulai, akhir, *inklusif, tubuh),
                Iterasi::Setiap(e) => self.untuk_setiap(nama, e, tubuh),
            },
            Stmt::Kembalikan { nilai, .. } => {
                let n = match nilai {
                    Some(e) => {
                        let v = self.gen_expr(e)?;
                        let t = v.tipe.clone();
                        Some((self.ambil_milik(v), t))
                    }
                    None => None,
                };
                if let Some((repr, tipe)) = n {
                    if let Some(hs) = self.hasil_slot.clone() {
                        self.simpan(&repr, &hs, &tipe);
                    }
                }
                let akhir = self.akhir.clone();
                self.br(&akhir);
                Ok(())
            }
            Stmt::Tegaskan { kondisi, .. } => {
                if self.g.opsi.kontrak {
                    let v = self.gen_expr(kondisi)?;
                    let p = format!("tegasan gagal: {}", cetak_ekspr(kondisi));
                    self.cek_benar(&v.repr, &p);
                }
                Ok(())
            }
            Stmt::Hentikan { .. } => {
                let info = self
                    .loops
                    .last()
                    .ok_or_else(|| Diag::baru("NUSA-C004", "`hentikan` di luar loop"))?;
                let dasar = info.dasar;
                let keluar = info.keluar.clone();
                self.drop_dari(dasar);
                self.br(&keluar);
                Ok(())
            }
            Stmt::Lanjutkan { .. } => {
                let info = self
                    .loops
                    .last()
                    .ok_or_else(|| Diag::baru("NUSA-C005", "`lanjutkan` di luar loop"))?;
                let dasar = info.dasar;
                let lanjut = info.lanjut.clone();
                self.drop_dari(dasar);
                self.br(&lanjut);
                Ok(())
            }
        }
    }

    fn ke_i64(&mut self, v: &Nilai) -> String {
        self.ke_i64_repr(&v.repr, &v.tipe)
    }

    fn ke_i64_repr(&mut self, repr: &str, tipe: &Tipe) -> String {
        match tipe {
            Tipe::Bil => repr.to_string(),
            Tipe::Log => {
                let t = self.t();
                self.masukkan(format!("{} = zext i1 {} to i64", t, repr));
                t
            }
            Tipe::Des => {
                let t = self.t();
                self.masukkan(format!("{} = bitcast double {} to i64", t, repr));
                t
            }
            Tipe::Teks | Tipe::Daftar(_) => {
                let t = self.t();
                self.masukkan(format!("{} = ptrtoint ptr {} to i64", t, repr));
                t
            }
            Tipe::Kosong => String::new(),
        }
    }

    fn dari_i64(&mut self, repr: &str, tipe: &Tipe) -> String {
        match tipe {
            Tipe::Bil => repr.to_string(),
            Tipe::Log => {
                let t = self.t();
                self.masukkan(format!("{} = trunc i64 {} to i1", t, repr));
                t
            }
            Tipe::Des => {
                let t = self.t();
                self.masukkan(format!("{} = bitcast i64 {} to double", t, repr));
                t
            }
            Tipe::Teks | Tipe::Daftar(_) => {
                let t = self.t();
                self.masukkan(format!("{} = inttoptr i64 {} to ptr", t, repr));
                t
            }
            Tipe::Kosong => String::new(),
        }
    }

    fn tugas_nama(&mut self, nama: &str, op: TugasOp, nilai: &Expr) -> Hasil<()> {
        let slot = self.cari_slot(nama)?;
        let tipe = slot.tipe.clone();
        if op == TugasOp::Isi {
            let v = self.gen_expr(nilai)?;
            let repr = self.ambil_milik(v);
            if tipe.adalah_heap() {
                self.lepas_slot(&slot.ir, &tipe);
            }
            self.simpan(&repr, &slot.ir, &tipe);
            return Ok(());
        }
        let lama = Nilai {
            repr: self.muat(&slot.ir, &tipe),
            tipe: tipe.clone(),
            milik: Milik::Pinjam,
        };
        let kanan = self.gen_expr(nilai)?;
        let hasil = self.hitung(tugas_ke_biner(op), &lama, &kanan, &tipe)?;
        let repr = self.ambil_milik(hasil);
        if tipe.adalah_heap() {
            self.lepas_slot(&slot.ir, &tipe);
        }
        self.simpan(&repr, &slot.ir, &tipe);
        Ok(())
    }

    fn tugas_indeks(&mut self, nama: &str, indeks: &Expr, op: TugasOp, nilai: &Expr) -> Hasil<()> {
        let base = self.cari_slot(nama)?;
        let (elemen, tipe_daftar) = match &base.tipe {
            Tipe::Daftar(t) => ((**t).clone(), base.tipe.clone()),
            _ => {
                return Err(Diag::baru(
                    "NUSA-C006",
                    "kesalahan internal: penugasan indeks pada bukan daftar",
                ))
            }
        };
        let list_ptr = self.muat(&base.ir, &tipe_daftar);
        let idx = self.gen_expr(indeks)?;
        let idx_i = self.ke_i64(&idx);
        let nilai_i = if op == TugasOp::Isi {
            let v = self.gen_expr(nilai)?;
            let repr = self.ambil_milik(v);
            self.ke_i64_repr(&repr, &elemen)
        } else {
            let raw = self.t();
            self.masukkan(format!(
                "{} = call i64 @nusa_daftar_ambil(ptr {}, i64 {})",
                raw, list_ptr, idx_i
            ));
            let lama_repr = self.dari_i64(&raw, &elemen);
            let lama = Nilai {
                repr: lama_repr,
                tipe: elemen.clone(),
                milik: Milik::Pinjam,
            };
            let kanan = self.gen_expr(nilai)?;
            let hasil = self.hitung(tugas_ke_biner(op), &lama, &kanan, &elemen)?;
            let repr = self.ambil_milik(hasil);
            self.ke_i64_repr(&repr, &elemen)
        };
        let baru = self.t();
        self.masukkan(format!(
            "{} = call ptr @nusa_daftar_isi(ptr {}, i64 {}, i64 {})",
            baru, list_ptr, idx_i, nilai_i
        ));
        self.simpan(&baru, &base.ir, &tipe_daftar);
        Ok(())
    }

    fn untuk_rentang(
        &mut self,
        nama: &str,
        mulai: &Expr,
        akhir: &Expr,
        inklusif: bool,
        tubuh: &[Stmt],
    ) -> Hasil<()> {
        let mv = self.gen_expr(mulai)?;
        let av = self.gen_expr(akhir)?;
        let s_slot = self.alokasi(&Tipe::Bil);
        let e_slot = self.alokasi(&Tipe::Bil);
        let i_slot = self.alokasi(&Tipe::Bil);
        self.simpan(&mv.repr, &s_slot, &Tipe::Bil);
        self.simpan(&av.repr, &e_slot, &Tipe::Bil);
        let m = self.muat(&s_slot, &Tipe::Bil);
        self.simpan(&m, &i_slot, &Tipe::Bil);

        self.lingkup.push(HashMap::new());
        self.deklarasikan(nama, i_slot.clone(), Tipe::Bil);

        let cond = self.label();
        let body = self.label();
        let inc = self.label();
        let after = self.label();
        self.br(&cond);
        self.mulai(&cond);
        let iv = self.muat(&i_slot, &Tipe::Bil);
        let ev = self.muat(&e_slot, &Tipe::Bil);
        let c = self.t();
        self.masukkan(format!(
            "{} = icmp {} i64 {}, {}",
            c,
            if inklusif { "sle" } else { "slt" },
            iv,
            ev
        ));
        self.br_kondisi(&c, &body, &after);
        self.mulai(&body);
        self.loops.push(LoopInfo {
            dasar: self.lingkup.len() - 1,
            lanjut: inc.clone(),
            keluar: after.clone(),
        });
        self.blok(tubuh)?;
        self.loops.pop();
        self.tutup_lingkup();
        self.tutup_ke(&inc);
        self.mulai(&inc);
        let iv2 = self.muat(&i_slot, &Tipe::Bil);
        let nv = if self.g.opsi.aman {
            self.tambah_cek(&iv2, "1", "luapan penghitung loop")
        } else {
            let t = self.t();
            self.masukkan(format!("{} = add i64 {}, 1", t, iv2));
            t
        };
        self.simpan(&nv, &i_slot, &Tipe::Bil);
        self.br(&cond);
        self.mulai(&after);
        Ok(())
    }

    fn untuk_setiap(&mut self, nama: &str, iter: &Expr, tubuh: &[Stmt]) -> Hasil<()> {
        let v = self.gen_expr(iter)?;
        let elemen = match &v.tipe {
            Tipe::Daftar(t) => (**t).clone(),
            _ => {
                return Err(Diag::baru(
                    "NUSA-C007",
                    "kesalahan internal: `untuk ... dalam` pada bukan daftar",
                ))
            }
        };
        let tipe_daftar = Tipe::Daftar(Box::new(elemen.clone()));
        let iter_repr = self.ambil_milik(v);
        let iter_slot = self.alokasi(&tipe_daftar);
        self.daftarkan_heap(&iter_slot, &tipe_daftar);
        self.simpan(&iter_repr, &iter_slot, &tipe_daftar);

        let len_slot = self.alokasi(&Tipe::Bil);
        let idx_slot = self.alokasi(&Tipe::Bil);
        let ip = self.muat(&iter_slot, &tipe_daftar);
        let len = self.t();
        self.masukkan(format!(
            "{} = call i64 @nusa_daftar_panjang(ptr {})",
            len, ip
        ));
        self.simpan(&len, &len_slot, &Tipe::Bil);
        self.simpan("0", &idx_slot, &Tipe::Bil);

        self.lingkup.push(HashMap::new());
        let var_slot = self.alokasi(&elemen);
        self.deklarasikan(nama, var_slot.clone(), elemen.clone());
        if elemen.adalah_heap() {
            self.daftarkan_heap(&var_slot, &elemen);
        }

        let cond = self.label();
        let body = self.label();
        let inc = self.label();
        let after = self.label();
        self.br(&cond);
        self.mulai(&cond);
        let iv = self.muat(&idx_slot, &Tipe::Bil);
        let lv = self.muat(&len_slot, &Tipe::Bil);
        let c = self.t();
        self.masukkan(format!("{} = icmp slt i64 {}, {}", c, iv, lv));
        self.br_kondisi(&c, &body, &after);
        self.mulai(&body);
        self.loops.push(LoopInfo {
            dasar: self.lingkup.len() - 1,
            lanjut: inc.clone(),
            keluar: after.clone(),
        });
        let ip2 = self.muat(&iter_slot, &tipe_daftar);
        let ix = self.muat(&idx_slot, &Tipe::Bil);
        let raw = self.t();
        self.masukkan(format!(
            "{} = call i64 @nusa_daftar_ambil(ptr {}, i64 {})",
            raw, ip2, ix
        ));
        let nilai = self.dari_i64(&raw, &elemen);
        if elemen.adalah_heap() {
            self.masukkan(format!("call void @nusa_tahan(ptr {})", nilai));
        }
        self.simpan(&nilai, &var_slot, &elemen);
        self.blok(tubuh)?;
        self.loops.pop();
        self.tutup_lingkup();
        self.tutup_ke(&inc);
        self.mulai(&inc);
        let ix2 = self.muat(&idx_slot, &Tipe::Bil);
        let nx = if self.g.opsi.aman {
            self.tambah_cek(&ix2, "1", "luapan penghitung loop")
        } else {
            let t = self.t();
            self.masukkan(format!("{} = add i64 {}, 1", t, ix2));
            t
        };
        self.simpan(&nx, &idx_slot, &Tipe::Bil);
        self.br(&cond);
        self.mulai(&after);
        self.lepas_slot(&iter_slot, &tipe_daftar);
        Ok(())
    }

    fn tambah_cek(&mut self, a: &str, b: &str, pesan: &str) -> String {
        let r = self.t();
        self.masukkan(format!(
            "{} = call {{ i64, i1 }} @llvm.sadd.with.overflow.i64(i64 {}, i64 {})",
            r, a, b
        ));
        let v = self.t();
        let o = self.t();
        self.masukkan(format!("{} = extractvalue {{ i64, i1 }} {}, 0", v, r));
        self.masukkan(format!("{} = extractvalue {{ i64, i1 }} {}, 1", o, r));
        self.panik_jika(&o, pesan);
        v
    }
    fn gen_expr(&mut self, e: &Expr) -> Hasil<Nilai> {
        match &e.kind {
            ExprKind::Bil(n) => {
                if matches!(self.g.info.tipe(e), Tipe::Des) {
                    Ok(Nilai::skalar(fmt_des(*n as f64), Tipe::Des))
                } else {
                    Ok(Nilai::skalar(n.to_string(), Tipe::Bil))
                }
            }
            ExprKind::Des(n) => Ok(Nilai::skalar(fmt_des(*n), Tipe::Des)),
            ExprKind::Teks(s) => {
                let n = self.g.intern(s);
                Ok(Nilai {
                    repr: n,
                    tipe: Tipe::Teks,
                    milik: Milik::Pinjam,
                })
            }
            ExprKind::Log(b) => Ok(Nilai::skalar(
                if *b { "true" } else { "false" }.to_string(),
                Tipe::Log,
            )),
            ExprKind::Nama(nama) => {
                let slot = self.cari_slot(nama)?;
                let v = self.muat(&slot.ir, &slot.tipe);
                Ok(Nilai {
                    repr: v,
                    tipe: slot.tipe,
                    milik: Milik::Pinjam,
                })
            }
            ExprKind::Daftar(isi) => self.ekspr_daftar(e, isi),
            ExprKind::Unary(op, a) => {
                let v = self.gen_expr(a)?;
                let tipe = self.g.info.tipe(e).clone();
                match op {
                    UnOp::Neg => match tipe {
                        Tipe::Bil => {
                            let r = if self.g.opsi.aman {
                                self.biner_cek("ssub", "0", &v.repr, "luapan negasi")
                            } else {
                                let t = self.t();
                                self.masukkan(format!("{} = sub i64 0, {}", t, v.repr));
                                t
                            };
                            Ok(Nilai::skalar(r, Tipe::Bil))
                        }
                        Tipe::Des => {
                            let t = self.t();
                            self.masukkan(format!("{} = fneg double {}", t, v.repr));
                            Ok(Nilai::skalar(t, Tipe::Des))
                        }
                        _ => Err(Diag::baru(
                            "NUSA-C010",
                            "kesalahan internal: negasi bukan angka",
                        )),
                    },
                    UnOp::Tidak => {
                        let t = self.t();
                        self.masukkan(format!("{} = xor i1 {}, true", t, v.repr));
                        Ok(Nilai::skalar(t, Tipe::Log))
                    }
                }
            }
            ExprKind::Biner(op, a, b) => {
                if matches!(op, BinOp::Dan | BinOp::Atau) {
                    return self.ekspr_logika(*op, a, b);
                }
                if op.adalah_perbandingan() {
                    return self.banding(*op, a, b);
                }
                let tipe = self.g.info.tipe(e).clone();
                let va = self.gen_expr(a)?;
                let vb = self.gen_expr(b)?;
                self.hitung(*op, &va, &vb, &tipe)
            }
            ExprKind::Panggil(nama, args) => self.ekspr_panggil(e, nama, args),
            ExprKind::Metode(res, nama, args) => self.ekspr_metode(res, nama, args),
            ExprKind::Indeks(base, indeks) => {
                let b = self.gen_expr(base)?;
                let i = self.gen_expr(indeks)?;
                let i_i = self.ke_i64(&i);
                let elemen = self.g.info.tipe(e).clone();
                let raw = self.t();
                self.masukkan(format!(
                    "{} = call i64 @nusa_daftar_ambil(ptr {}, i64 {})",
                    raw, b.repr, i_i
                ));
                let val = self.dari_i64(&raw, &elemen);
                Ok(Nilai {
                    repr: val,
                    tipe: elemen,
                    milik: Milik::Pinjam,
                })
            }
        }
    }

    fn ekspr_daftar(&mut self, e: &Expr, isi: &[Expr]) -> Hasil<Nilai> {
        let elemen = match self.g.info.tipe(e) {
            Tipe::Daftar(t) => (**t).clone(),
            _ => {
                return Err(Diag::baru(
                    "NUSA-C011",
                    "kesalahan internal: tipe daftar tidak diketahui",
                ))
            }
        };
        let jenis = jenis_elemen(&elemen);
        let cur0 = self.t();
        self.masukkan(format!(
            "{} = call ptr @nusa_daftar_baru(i64 {}, i64 {})",
            cur0,
            isi.len(),
            jenis
        ));
        let mut cur = cur0;
        for x in isi {
            let v = self.gen_expr(x)?;
            let repr = self.ambil_milik(v);
            let vi = self.ke_i64_repr(&repr, &elemen);
            let n = self.t();
            self.masukkan(format!(
                "{} = call ptr @nusa_daftar_tambah(ptr {}, i64 {})",
                n, cur, vi
            ));
            cur = n;
        }
        Ok(self.milik_baru(cur, Tipe::Daftar(Box::new(elemen))))
    }

    fn ekspr_logika(&mut self, op: BinOp, a: &Expr, b: &Expr) -> Hasil<Nilai> {
        let kiri = self.gen_expr(a)?;
        let slot = self.alokasi(&Tipe::Log);
        self.simpan(&kiri.repr, &slot, &Tipe::Log);
        let kanan = self.label();
        let gabung = self.label();
        if op == BinOp::Dan {
            self.br_kondisi(&kiri.repr, &kanan, &gabung);
        } else {
            self.br_kondisi(&kiri.repr, &gabung, &kanan);
        }
        self.mulai(&kanan);
        let v = self.gen_expr(b)?;
        self.simpan(&v.repr, &slot, &Tipe::Log);
        self.br(&gabung);
        self.mulai(&gabung);
        let hasil = self.muat(&slot, &Tipe::Log);
        Ok(Nilai::skalar(hasil, Tipe::Log))
    }

    fn banding(&mut self, op: BinOp, a: &Expr, b: &Expr) -> Hasil<Nilai> {
        let va = self.gen_expr(a)?;
        let vb = self.gen_expr(b)?;
        let ta = self.g.info.tipe(a).clone();
        let kode = match op {
            BinOp::KurangDari => "slt",
            BinOp::KurangSama => "sle",
            BinOp::LebihDari => "sgt",
            BinOp::LebihSama => "sge",
            BinOp::SamaSama => "eq",
            BinOp::TidakSama => "ne",
            _ => {
                return Err(Diag::baru(
                    "NUSA-C012",
                    "kesalahan internal: bukan perbandingan",
                ))
            }
        };
        match ta {
            Tipe::Bil => {
                let t = self.t();
                self.masukkan(format!(
                    "{} = icmp {} i64 {}, {}",
                    t, kode, va.repr, vb.repr
                ));
                Ok(Nilai::skalar(t, Tipe::Log))
            }
            Tipe::Log => {
                let t = self.t();
                self.masukkan(format!("{} = icmp {} i1 {}, {}", t, kode, va.repr, vb.repr));
                Ok(Nilai::skalar(t, Tipe::Log))
            }
            Tipe::Des => {
                let fkode = match op {
                    BinOp::KurangDari => "olt",
                    BinOp::KurangSama => "ole",
                    BinOp::LebihDari => "ogt",
                    BinOp::LebihSama => "oge",
                    BinOp::SamaSama => "oeq",
                    _ => "one",
                };
                let t = self.t();
                self.masukkan(format!(
                    "{} = fcmp {} double {}, {}",
                    t, fkode, va.repr, vb.repr
                ));
                Ok(Nilai::skalar(t, Tipe::Log))
            }
            Tipe::Teks => {
                if matches!(op, BinOp::SamaSama | BinOp::TidakSama) {
                    let r = self.t();
                    self.masukkan(format!(
                        "{} = call i8 @nusa_teks_sama(ptr {}, ptr {})",
                        r, va.repr, vb.repr
                    ));
                    let b = self.t();
                    self.masukkan(format!("{} = trunc i8 {} to i1", b, r));
                    if op == BinOp::TidakSama {
                        let n = self.t();
                        self.masukkan(format!("{} = xor i1 {}, true", n, b));
                        Ok(Nilai::skalar(n, Tipe::Log))
                    } else {
                        Ok(Nilai::skalar(b, Tipe::Log))
                    }
                } else {
                    let r = self.t();
                    self.masukkan(format!(
                        "{} = call i64 @nusa_teks_banding(ptr {}, ptr {})",
                        r, va.repr, vb.repr
                    ));
                    let t = self.t();
                    self.masukkan(format!("{} = icmp {} i64 {}, 0", t, kode, r));
                    Ok(Nilai::skalar(t, Tipe::Log))
                }
            }
            lain => Err(Diag::baru(
                "NUSA-C013",
                format!("kesalahan internal: perbandingan pada `{}`", lain.nama()),
            )),
        }
    }

    fn hitung(&mut self, op: BinOp, a: &Nilai, b: &Nilai, tipe: &Tipe) -> Hasil<Nilai> {
        match (op, tipe) {
            (BinOp::Tambah, Tipe::Teks) => {
                let t = self.t();
                self.masukkan(format!(
                    "{} = call ptr @nusa_teks_gabung(ptr {}, ptr {})",
                    t, a.repr, b.repr
                ));
                Ok(self.milik_baru(t, Tipe::Teks))
            }
            (_, Tipe::Bil) => {
                let r = match op {
                    BinOp::Tambah => {
                        if self.g.opsi.aman {
                            self.biner_cek("sadd", &a.repr, &b.repr, "luapan penjumlahan")
                        } else {
                            let t = self.t();
                            self.masukkan(format!("{} = add i64 {}, {}", t, a.repr, b.repr));
                            t
                        }
                    }
                    BinOp::Kurang => {
                        if self.g.opsi.aman {
                            self.biner_cek("ssub", &a.repr, &b.repr, "luapan pengurangan")
                        } else {
                            let t = self.t();
                            self.masukkan(format!("{} = sub i64 {}, {}", t, a.repr, b.repr));
                            t
                        }
                    }
                    BinOp::Kali => {
                        if self.g.opsi.aman {
                            self.biner_cek("smul", &a.repr, &b.repr, "luapan perkalian")
                        } else {
                            let t = self.t();
                            self.masukkan(format!("{} = mul i64 {}, {}", t, a.repr, b.repr));
                            t
                        }
                    }
                    BinOp::Bagi => {
                        let t = self.t();
                        self.masukkan(format!(
                            "{} = call i64 @nusa_bagi(i64 {}, i64 {})",
                            t, a.repr, b.repr
                        ));
                        t
                    }
                    BinOp::Mod => {
                        let t = self.t();
                        self.masukkan(format!(
                            "{} = call i64 @nusa_mod(i64 {}, i64 {})",
                            t, a.repr, b.repr
                        ));
                        t
                    }
                    _ => {
                        return Err(Diag::baru(
                            "NUSA-C014",
                            "kesalahan internal: operator aritmetika tidak sah",
                        ))
                    }
                };
                Ok(Nilai::skalar(r, Tipe::Bil))
            }
            (_, Tipe::Des) => {
                let kode = match op {
                    BinOp::Tambah => "fadd",
                    BinOp::Kurang => "fsub",
                    BinOp::Kali => "fmul",
                    BinOp::Bagi => "fdiv",
                    _ => {
                        return Err(Diag::baru(
                            "NUSA-C015",
                            "kesalahan internal: operator desimal tidak sah",
                        ))
                    }
                };
                let t = self.t();
                self.masukkan(format!("{} = {} double {}, {}", t, kode, a.repr, b.repr));
                Ok(Nilai::skalar(t, Tipe::Des))
            }
            _ => Err(Diag::baru(
                "NUSA-C016",
                format!("kesalahan internal: aritmetika pada `{}`", tipe.nama()),
            )),
        }
    }

    fn biner_cek(&mut self, intrinsic: &str, a: &str, b: &str, pesan: &str) -> String {
        let r = self.t();
        self.masukkan(format!(
            "{} = call {{ i64, i1 }} @llvm.{}.with.overflow.i64(i64 {}, i64 {})",
            r, intrinsic, a, b
        ));
        let v = self.t();
        let o = self.t();
        self.masukkan(format!("{} = extractvalue {{ i64, i1 }} {}, 0", v, r));
        self.masukkan(format!("{} = extractvalue {{ i64, i1 }} {}, 1", o, r));
        self.panik_jika(&o, pesan);
        v
    }

    fn ekspr_panggil(&mut self, e: &Expr, nama: &str, args: &[Expr]) -> Hasil<Nilai> {
        let _ = e;
        match nama {
            "cetak" | "cetakln" => {
                for a in args {
                    let v = self.gen_expr(a)?;
                    self.cetak_nilai(&v)?;
                }
                if nama == "cetakln" {
                    let baris = self.g.intern("\n");
                    self.masukkan(format!("call void @nusa_cetak_teks(ptr {})", baris));
                }
                Ok(Nilai::kosong())
            }
            "panjang" => {
                let v = self.gen_expr(&args[0])?;
                let t = self.t();
                let fungsi = match v.tipe {
                    Tipe::Teks => "@nusa_teks_panjang",
                    _ => "@nusa_daftar_panjang",
                };
                self.masukkan(format!("{} = call i64 {}(ptr {})", t, fungsi, v.repr));
                Ok(Nilai::skalar(t, Tipe::Bil))
            }
            "mutlak" => {
                let v = self.gen_expr(&args[0])?;
                match v.tipe {
                    Tipe::Bil => {
                        let t = self.t();
                        self.masukkan(format!("{} = call i64 @nusa_mutlak_bil(i64 {})", t, v.repr));
                        Ok(Nilai::skalar(t, Tipe::Bil))
                    }
                    _ => {
                        let t = self.t();
                        self.masukkan(format!(
                            "{} = call double @llvm.fabs.f64(double {})",
                            t, v.repr
                        ));
                        Ok(Nilai::skalar(t, Tipe::Des))
                    }
                }
            }
            _ => {
                let sig = self.g.info.tanda_tangan.get(nama).cloned().ok_or_else(|| {
                    Diag::baru(
                        "NUSA-C020",
                        format!("kesalahan internal: fungsi `{}` tidak ditemukan", nama),
                    )
                })?;
                let mut ir_args = Vec::new();
                for (a, p) in args.iter().zip(&sig.params) {
                    let v = self.gen_expr(a)?;
                    let repr = if p.adalah_heap() {
                        self.ambil_milik(v)
                    } else {
                        v.repr
                    };
                    ir_args.push(format!("{} {}", tipe_ir(p), repr));
                }
                let argumen = ir_args.join(", ");
                if sig.hasil == Tipe::Kosong {
                    self.masukkan(format!("call void @{}({})", nama, argumen));
                    Ok(Nilai::kosong())
                } else {
                    let t = self.t();
                    self.masukkan(format!(
                        "{} = call {} @{}({})",
                        t,
                        tipe_ir(&sig.hasil),
                        nama,
                        argumen
                    ));
                    if sig.hasil.adalah_heap() {
                        Ok(self.milik_baru(t, sig.hasil))
                    } else {
                        Ok(Nilai::skalar(t, sig.hasil))
                    }
                }
            }
        }
    }

    fn ekspr_metode(&mut self, res: &Expr, nama: &str, args: &[Expr]) -> Hasil<Nilai> {
        match nama {
            "tambah" => {
                let nama_res = match &res.kind {
                    ExprKind::Nama(n) => n.clone(),
                    _ => {
                        return Err(Diag::baru(
                            "NUSA-C021",
                            "kesalahan internal: penerima `tambah` bukan variabel",
                        ))
                    }
                };
                let slot = self.cari_slot(&nama_res)?;
                let (elemen, tipe_daftar) = match &slot.tipe {
                    Tipe::Daftar(t) => ((**t).clone(), slot.tipe.clone()),
                    _ => {
                        return Err(Diag::baru(
                            "NUSA-C022",
                            "kesalahan internal: `tambah` pada bukan daftar",
                        ))
                    }
                };
                let daftar = self.muat(&slot.ir, &tipe_daftar);
                let v = self.gen_expr(&args[0])?;
                let repr = self.ambil_milik(v);
                let vi = self.ke_i64_repr(&repr, &elemen);
                let baru = self.t();
                self.masukkan(format!(
                    "{} = call ptr @nusa_daftar_tambah(ptr {}, i64 {})",
                    baru, daftar, vi
                ));
                self.simpan(&baru, &slot.ir, &tipe_daftar);
                Ok(Nilai::kosong())
            }
            "panjang" => {
                let v = self.gen_expr(res)?;
                let t = self.t();
                let fungsi = match v.tipe {
                    Tipe::Teks => "@nusa_teks_panjang",
                    _ => "@nusa_daftar_panjang",
                };
                self.masukkan(format!("{} = call i64 {}(ptr {})", t, fungsi, v.repr));
                Ok(Nilai::skalar(t, Tipe::Bil))
            }
            _ => Err(Diag::baru(
                "NUSA-C023",
                format!("kesalahan internal: metode `{}` tidak dikenal", nama),
            )),
        }
    }

    fn cetak_nilai(&mut self, v: &Nilai) -> Hasil<()> {
        match &v.tipe {
            Tipe::Bil => self.masukkan(format!("call void @nusa_cetak_bil(i64 {})", v.repr)),
            Tipe::Des => self.masukkan(format!("call void @nusa_cetak_des(double {})", v.repr)),
            Tipe::Log => {
                let z = self.t();
                self.masukkan(format!("{} = zext i1 {} to i8", z, v.repr));
                self.masukkan(format!("call void @nusa_cetak_log(i8 {})", z));
            }
            Tipe::Teks => self.masukkan(format!("call void @nusa_cetak_teks(ptr {})", v.repr)),
            Tipe::Daftar(elemen) => self.cetak_daftar(&v.repr, elemen)?,
            Tipe::Kosong => {}
        }
        Ok(())
    }

    fn cetak_daftar(&mut self, daftar: &str, elemen: &Tipe) -> Hasil<()> {
        let buka = self.g.intern("[");
        self.masukkan(format!("call void @nusa_cetak_teks(ptr {})", buka));
        let idx_slot = self.alokasi(&Tipe::Bil);
        let len_slot = self.alokasi(&Tipe::Bil);
        let l = self.t();
        self.masukkan(format!(
            "{} = call i64 @nusa_daftar_panjang(ptr {})",
            l, daftar
        ));
        self.simpan(&l, &len_slot, &Tipe::Bil);
        self.simpan("0", &idx_slot, &Tipe::Bil);

        let cond = self.label();
        let body = self.label();
        let akhir = self.label();
        self.br(&cond);
        self.mulai(&cond);
        let i = self.muat(&idx_slot, &Tipe::Bil);
        let n = self.muat(&len_slot, &Tipe::Bil);
        let c = self.t();
        self.masukkan(format!("{} = icmp slt i64 {}, {}", c, i, n));
        self.br_kondisi(&c, &body, &akhir);
        self.mulai(&body);
        let i2 = self.muat(&idx_slot, &Tipe::Bil);
        let c2 = self.t();
        self.masukkan(format!("{} = icmp sgt i64 {}, 0", c2, i2));
        let ya = self.label();
        let tidak = self.label();
        self.br_kondisi(&c2, &ya, &tidak);
        self.mulai(&ya);
        let pisah = self.g.intern(", ");
        self.masukkan(format!("call void @nusa_cetak_teks(ptr {})", pisah));
        self.br(&tidak);
        self.mulai(&tidak);
        let raw = self.t();
        self.masukkan(format!(
            "{} = call i64 @nusa_daftar_ambil(ptr {}, i64 {})",
            raw, daftar, i2
        ));
        let val = self.dari_i64(&raw, elemen);
        self.cetak_nilai(&Nilai {
            repr: val,
            tipe: elemen.clone(),
            milik: Milik::Pinjam,
        })?;
        let inc = self.t();
        self.masukkan(format!("{} = add i64 {}, 1", inc, i2));
        self.simpan(&inc, &idx_slot, &Tipe::Bil);
        self.br(&cond);
        self.mulai(&akhir);
        let tutup = self.g.intern("]");
        self.masukkan(format!("call void @nusa_cetak_teks(ptr {})", tutup));
        Ok(())
    }
}

fn tugas_ke_biner(op: TugasOp) -> BinOp {
    match op {
        TugasOp::Tambah => BinOp::Tambah,
        TugasOp::Kurang => BinOp::Kurang,
        TugasOp::Kali => BinOp::Kali,
        TugasOp::Bagi => BinOp::Bagi,
        TugasOp::Mod => BinOp::Mod,
        TugasOp::Isi => BinOp::Tambah,
    }
}
