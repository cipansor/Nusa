//! Analisis semantik: keamanan tipe, mutabilitas, kepemilikan (move),
//! dan peminjaman (borrow). Semua galat diberi kode dan lokasi.

use crate::ast::*;
use crate::diag::Diagnostik;
use crate::lex::KATA_KUNCI;
use std::collections::{HashMap, HashSet};

#[derive(Clone, Debug)]
pub struct Sig {
    pub params: Vec<Ty>,
    pub hasil: Ty,
}

#[derive(Clone, Debug)]
pub struct StrukInfo {
    pub medan: Vec<(String, Ty)>,
    pub indeks: HashMap<String, usize>,
}

#[derive(Clone, Debug)]
pub struct EnumInfo {
    pub varian: Vec<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Panggilan {
    Fungsi(String),
    Bawaan(String),
}

pub struct Sema {
    pub diag: Vec<Diagnostik>,
    pub tipe: HashMap<u32, Ty>,
    pub panggil: HashMap<u32, Panggilan>,
    pub elemen_untuk: HashMap<u32, Ty>,
    pub struk: HashMap<String, StrukInfo>,
    pub enumerasi: HashMap<String, EnumInfo>,
    pub sig: HashMap<String, Sig>,
    pub baatan: Vec<String>,
    /// Ekspresi yang memindahkan nilai (dipakai penafsir untuk menyamakan
    /// pengosongan sumber dengan hasil kompilasi).
    pub pindah: HashSet<u32>,
    /// Panggilan berbentuk metode (`d.tambah(x)`), bukan fungsi bawaan biasa.
    pub metode: HashSet<u32>,
}

#[derive(Clone, Debug)]
#[allow(dead_code)]
struct Var {
    id: u64,
    ty: Ty,
    ubah: bool,
    dipindah: bool,
    /// Jalur medan yang sudah dipindah, mis. `d` atau `d.x`.
    dipindah_medan: Vec<String>,
    kedalaman: usize,
}

#[derive(Clone, Debug)]
struct Pinjaman {
    ubah: bool,
    oleh: Option<String>,
    kedalaman: usize,
}

#[derive(Clone, Debug)]
struct Tempat {
    id: u64,
    ty: Ty,
    ubah: bool,
    akar: String,
    /// Jalur medan relatif terhadap `akar` (kosong = seluruh variabel).
    jalur: String,
}

struct FungsiCtx {
    hasil: Ty,
    lingkup: Vec<HashMap<String, Var>>,
    pinjaman: HashMap<u64, Vec<Pinjaman>>,
    id_berikut: u64,
    jangan_pindah: u32,
    kedalaman: usize,
    dalam_selama: u32,
    dalam_jamin: bool,
}

pub const BAWAAN: &[&str] = &[
    "cetak", "cetakln", "ctk", "tulis", "tls", "daftar", "tambah", "tmb", "panjang", "pjg",
    "ambil", "amb", "gagal", "ggl", "sisip", "ssp",
];

/// Nama kanonik untuk sinonim fungsi bawaan (gaya PR #1/#3).
pub fn bawaan_kanonik(nama: &str) -> &str {
    match nama {
        "cetakln" | "ctk" => "cetak",
        "tls" => "tulis",
        "tmb" => "tambah",
        "pjg" => "panjang",
        "amb" => "ambil",
        "ggl" => "gagal",
        "ssp" => "sisip",
        lain => lain,
    }
}

pub fn periksa(prog: &Prog, berkas: &str, boleh_takaman: bool) -> Sema {
    let mut s = Sema {
        diag: Vec::new(),
        tipe: HashMap::new(),
        panggil: HashMap::new(),
        elemen_untuk: HashMap::new(),
        struk: HashMap::new(),
        enumerasi: HashMap::new(),
        sig: HashMap::new(),
        baatan: BAWAAN.iter().map(|x| x.to_string()).collect(),
        pindah: HashSet::new(),
        metode: HashSet::new(),
    };
    s.daftar_deklarasi(prog, berkas);
    for f in &prog.fungsi {
        if f.luar {
            continue;
        }
        s.periksa_fungsi(f, berkas, boleh_takaman);
    }
    s
}

impl Sema {
    fn galat(&mut self, berkas: &str, span: Span, pesan: String, kode: &str) {
        self.diag.push(Diagnostik::galat(berkas, span, pesan, kode));
    }

    fn daftar_deklarasi(&mut self, prog: &Prog, berkas: &str) {
        for st in &prog.struk {
            if self.struk.contains_key(&st.nama) || self.enumerasi.contains_key(&st.nama) {
                self.galat(
                    berkas,
                    st.span,
                    format!("tipe `{}` dideklarasikan dua kali", st.nama),
                    "E0201",
                );
            }
            let mut medan = Vec::new();
            let mut indeks = HashMap::new();
            for (i, m) in st.medan.iter().enumerate() {
                if indeks.contains_key(&m.nama) {
                    self.galat(
                        berkas,
                        m.span,
                        format!("medan `{}` ganda pada struk `{}`", m.nama, st.nama),
                        "E0202",
                    );
                }
                if !self.tipe_sah(&m.ty) {
                    self.galat(
                        berkas,
                        m.span,
                        format!(
                            "tipe medan `{}` belum dikenal: `{}`",
                            m.nama,
                            m.ty.sebutan()
                        ),
                        "E0203",
                    );
                }
                if nama_tipe(&m.ty) == Some(st.nama.as_str()) {
                    self.galat(
                        berkas,
                        m.span,
                        format!(
                            "struk `{}` tidak boleh memuat dirinya sendiri (rekursi tak berujung)",
                            st.nama
                        ),
                        "E0204",
                    );
                }
                if mengandung_pinjaman(&m.ty) {
                    self.galat(
                        berkas,
                        m.span,
                        format!(
                            "medan `{}` tidak boleh menyimpan pinjaman pada Nusa v0.1",
                            m.nama
                        ),
                        "E0372",
                    );
                }
                if let Ty::Array(d, _) = &m.ty {
                    if !self.boleh_salin(d) {
                        self.galat(
                            berkas,
                            m.span,
                            format!("elemen larik `{}` harus bertipe yang dapat disalin", m.nama),
                            "E0373",
                        );
                    }
                }
                indeks.insert(m.nama.clone(), i);
                medan.push((m.nama.clone(), m.ty.clone()));
            }
            self.struk
                .insert(st.nama.clone(), StrukInfo { medan, indeks });
        }
        for e in &prog.enumerasi {
            if self.enumerasi.contains_key(&e.nama) || self.struk.contains_key(&e.nama) {
                self.galat(
                    berkas,
                    e.span,
                    format!("tipe `{}` dideklarasikan dua kali", e.nama),
                    "E0201",
                );
            }
            if e.varian.is_empty() {
                self.galat(
                    berkas,
                    e.span,
                    format!("enumerasi `{}` harus punya minimal satu varian", e.nama),
                    "E0205",
                );
            }
            let mut unik: Vec<String> = Vec::new();
            for v in &e.varian {
                if unik.contains(v) {
                    self.galat(
                        berkas,
                        e.span,
                        format!("varian `{}` ganda pada enumerasi `{}`", v, e.nama),
                        "E0206",
                    );
                }
                unik.push(v.clone());
            }
            self.enumerasi.insert(
                e.nama.clone(),
                EnumInfo {
                    varian: e.varian.clone(),
                },
            );
        }
        for f in &prog.fungsi {
            if KATA_KUNCI.iter().any(|(k, _)| k == &f.nama) {
                self.galat(
                    berkas,
                    f.span,
                    format!(
                        "`{}` adalah kata kunci, tidak boleh menjadi nama fungsi",
                        f.nama
                    ),
                    "E0207",
                );
            }
            if self.sig.contains_key(&f.nama) {
                self.galat(
                    berkas,
                    f.span,
                    format!("fungsi `{}` dideklarasikan dua kali", f.nama),
                    "E0208",
                );
            }
            if self.baatan.iter().any(|b| b == &f.nama) {
                self.galat(
                    berkas,
                    f.span,
                    format!("`{}` adalah fungsi bawaan; pilih nama lain", f.nama),
                    "E0209",
                );
            }
            for (i, p) in f.params.iter().enumerate() {
                if !self.tipe_sah(&p.ty) {
                    self.galat(
                        berkas,
                        p.span,
                        format!(
                            "tipe parameter `{}` belum dikenal: `{}`",
                            p.nama,
                            p.ty.sebutan()
                        ),
                        "E0203",
                    );
                }
                if f.params[..i].iter().any(|q| q.nama == p.nama) {
                    self.galat(
                        berkas,
                        p.span,
                        format!("parameter `{}` ganda", p.nama),
                        "E0210",
                    );
                }
            }
            if matches!(f.hasil, Ty::Ref { .. }) {
                self.galat(berkas, f.span, format!("fungsi `{}` tidak boleh mengembalikan pinjaman pada Nusa v0.1 (hindari referensi menggantung)", f.nama), "E0371");
            }
            if !self.tipe_sah(&f.hasil) {
                self.galat(
                    berkas,
                    f.span,
                    format!("tipe hasil belum dikenal: `{}`", f.hasil.sebutan()),
                    "E0203",
                );
            }
            if f.hasil != Ty::Kosong && !f.luar && !selalu_kembali(&f.badan) {
                self.galat(berkas, f.span, format!("fungsi `{}` bertipe hasil `{}` tetapi tidak semua jalur berakhir dengan `kembali`", f.nama, f.hasil.sebutan()), "E0211");
            }
            self.sig.insert(
                f.nama.clone(),
                Sig {
                    params: f.params.iter().map(|p| p.ty.clone()).collect(),
                    hasil: f.hasil.clone(),
                },
            );
        }
        if !self.sig.contains_key("utama") {
            self.diag.push(Diagnostik::galat(
                berkas,
                Span::baru(1, 1, 1),
                "program tidak memiliki `fungsi utama()` sebagai titik masuk".into(),
                "E0212",
            ));
        } else if let Some(s) = self.sig.get("utama") {
            if !s.params.is_empty() {
                self.diag.push(Diagnostik::galat(
                    berkas,
                    Span::baru(1, 1, 1),
                    "`utama` tidak boleh punya parameter".into(),
                    "E0213",
                ));
            }
            if !matches!(s.hasil, Ty::Kosong | Ty::Bil) {
                self.diag.push(Diagnostik::galat(
                    berkas,
                    Span::baru(1, 1, 1),
                    "`utama` harus mengembalikan `bil` (kode keluar) atau `kosong`".into(),
                    "E0214",
                ));
            }
        }
    }

    fn tipe_sah(&self, t: &Ty) -> bool {
        match t {
            Ty::Array(d, _) => self.tipe_sah(d) && self.boleh_salin(d) && !self.memuat_heap(d),
            Ty::Irisan(d) => self.tipe_sah(d) && !self.memuat_heap(d),
            Ty::Ref { dalam, .. } => self.tipe_sah(dalam),
            Ty::Daftar(d) => self.tipe_sah(d) && !self.memuat_heap(d),
            Ty::Nama(n) => self.struk.contains_key(n) || self.enumerasi.contains_key(n),
            _ => true,
        }
    }

    /// Tipe yang memiliki memori heap di dalamnya (butuh pembebasan).
    fn memuat_heap(&self, t: &Ty) -> bool {
        match t {
            Ty::Daftar(_) => true,
            Ty::Nama(n) => self
                .struk
                .get(n)
                .map(|s| s.medan.iter().any(|(_, m)| self.memuat_heap(m)))
                .unwrap_or(false),
            _ => false,
        }
    }

    fn boleh_salin(&self, t: &Ty) -> bool {
        match t {
            Ty::Nama(n) => {
                if let Some(st) = self.struk.get(n) {
                    st.medan.iter().all(|(_, m)| self.boleh_salin(m))
                } else {
                    true
                }
            }
            Ty::Array(d, _) => self.boleh_salin(d),
            _ => t.boleh_salin(),
        }
    }

    fn periksa_fungsi(&mut self, f: &Fungsi, berkas: &str, boleh_takaman: bool) {
        let mut ctx = FungsiCtx {
            hasil: f.hasil.clone(),
            lingkup: vec![HashMap::new()],
            pinjaman: HashMap::new(),
            id_berikut: 0,
            jangan_pindah: 0,
            kedalaman: 0,
            dalam_selama: 0,
            dalam_jamin: false,
        };
        for p in &f.params {
            ctx.id_berikut += 1;
            ctx.lingkup[0].insert(
                p.nama.clone(),
                Var {
                    id: ctx.id_berikut,
                    ty: p.ty.clone(),
                    ubah: p.ty.is_ubah_param(),
                    dipindah: false,
                    dipindah_medan: Vec::new(),
                    kedalaman: 0,
                },
            );
        }
        let mut cek = Pemeriksa {
            s: self,
            ctx: &mut ctx,
            berkas,
            fungsi: &f.nama,
            boleh_takaman,
            takaman: 0,
        };
        for e in f.perlu.iter() {
            let t = cek.eks(e, None);
            if t != Ty::Log {
                cek.s.galat(
                    berkas,
                    e.span,
                    format!("`perlu` harus bertipe `log`, bukan `{}`", t.sebutan()),
                    "E0300",
                );
            }
        }
        cek.ctx.dalam_jamin = true;
        for e in f.jamin.iter() {
            let t = cek.eks(e, Some(&f.hasil));
            if t != Ty::Log {
                cek.s.galat(
                    berkas,
                    e.span,
                    format!("`jamin` harus bertipe `log`, bukan `{}`", t.sebutan()),
                    "E0300",
                );
            }
        }
        cek.ctx.dalam_jamin = false;
        cek.blok(&f.badan);
        cek.akhir_pernyataan();
    }
}

trait UbahParam {
    fn is_ubah_param(&self) -> bool;
}

impl UbahParam for Ty {
    fn is_ubah_param(&self) -> bool {
        matches!(self, Ty::Ref { ubah: true, .. })
    }
}

fn nama_tipe(t: &Ty) -> Option<&str> {
    match t {
        Ty::Nama(n) => Some(n),
        _ => None,
    }
}

fn mengandung_pinjaman(t: &Ty) -> bool {
    match t {
        Ty::Ref { .. } => true,
        Ty::Array(d, _) | Ty::Irisan(d) | Ty::Daftar(d) => mengandung_pinjaman(d),
        _ => false,
    }
}

/// Apakah blok selalu berakhir dengan `kembali` (atau berhenti total)?
pub fn selalu_kembali(b: &Block) -> bool {
    b.stmts.iter().any(stmt_buntu)
}

fn stmt_buntu(s: &Stmt) -> bool {
    match s {
        Stmt::Kembali(..) => true,
        Stmt::Blok(b) | Stmt::Takaman(b) => selalu_kembali(b),
        Stmt::Jika {
            maka,
            lain: Some(l),
            ..
        } => selalu_kembali(maka) && selalu_kembali(l),
        Stmt::Cocok { kasus, lainnya, .. } => {
            kasus.iter().all(|(_, b)| selalu_kembali(b))
                && lainnya.as_ref().map(selalu_kembali).unwrap_or(true)
        }
        Stmt::Ekspresi(e) => match &e.kind {
            ExprKind::Panggil { nama, .. } => nama == "gagal",
            _ => false,
        },
        _ => false,
    }
}

struct Pemeriksa<'a> {
    s: &'a mut Sema,
    ctx: &'a mut FungsiCtx,
    berkas: &'a str,
    fungsi: &'a str,
    boleh_takaman: bool,
    takaman: u32,
}

impl<'a> Pemeriksa<'a> {
    fn galat(&mut self, span: Span, pesan: String, kode: &str) {
        self.s.galat(self.berkas, span, pesan, kode);
    }

    fn masuk(&mut self) {
        self.ctx.kedalaman += 1;
        self.ctx.lingkup.push(HashMap::new());
    }

    fn keluar(&mut self) {
        let k = self.ctx.kedalaman;
        self.ctx.lingkup.pop();
        self.ctx.kedalaman -= 1;
        for p in self.ctx.pinjaman.values_mut() {
            p.retain(|x| !(x.oleh.is_some() && x.kedalaman == k));
        }
    }

    fn cari(&self, nama: &str) -> Option<&Var> {
        for l in self.ctx.lingkup.iter().rev() {
            if let Some(v) = l.get(nama) {
                return Some(v);
            }
        }
        None
    }

    fn cari_mut(&mut self, nama: &str) -> Option<&mut Var> {
        for l in self.ctx.lingkup.iter_mut().rev() {
            if l.contains_key(nama) {
                return l.get_mut(nama);
            }
        }
        None
    }

    fn akhir_pernyataan(&mut self) {
        for p in self.ctx.pinjaman.values_mut() {
            p.retain(|x| x.oleh.is_some());
        }
    }

    fn id_var(&mut self) -> u64 {
        self.ctx.id_berikut += 1;
        self.ctx.id_berikut
    }

    fn pinjaman_aktif(&self, id: u64, akar: &str, ubah_saja: bool) -> Option<String> {
        if let Some(d) = self.ctx.pinjaman.get(&id) {
            for p in d {
                if !ubah_saja || p.ubah {
                    return Some(match (&p.oleh, p.ubah) {
                        (Some(v), true) => format!("`&ubah {}` yang disimpan di `{}`", akar, v),
                        (Some(v), false) => format!("`&{}` yang disimpan di `{}`", akar, v),
                        (None, true) => format!("`&ubah {}` sementara", akar),
                        (None, false) => format!("`&{}` sementara", akar),
                    });
                }
            }
        }
        None
    }

    fn tempat(&mut self, e: &Expr) -> Option<Tempat> {
        match &e.kind {
            ExprKind::Var(n) => {
                let v = self.cari(n)?.clone();
                self.s.tipe.insert(e.id, v.ty.clone());
                Some(Tempat {
                    id: v.id,
                    ty: v.ty,
                    ubah: v.ubah,
                    akar: n.clone(),
                    jalur: String::new(),
                })
            }
            ExprKind::Medan { dasar, nama } => {
                let d = self.tempat(dasar)?;
                let ty_dasar = match &d.ty {
                    Ty::Ref { dalam, .. } => (**dalam).clone(),
                    lain => lain.clone(),
                };
                let nama_struk = nama_tipe(&ty_dasar)?;
                let info = self.s.struk.get(nama_struk)?;
                let idx = *info.indeks.get(nama)?;
                let ty = info.medan[idx].1.clone();
                self.s.tipe.insert(e.id, ty.clone());
                let jalur = if d.jalur.is_empty() {
                    nama.clone()
                } else {
                    format!("{}.{}", d.jalur, nama)
                };
                Some(Tempat {
                    id: d.id,
                    ty,
                    ubah: d.ubah,
                    akar: d.akar,
                    jalur,
                })
            }
            ExprKind::Indeks { dasar, .. } => {
                let d = self.tempat(dasar)?;
                let ty = match &d.ty {
                    Ty::Array(t, _) => (**t).clone(),
                    Ty::Irisan(t) => (**t).clone(),
                    Ty::Daftar(t) => (**t).clone(),
                    _ => return None,
                };
                self.s.tipe.insert(e.id, ty.clone());
                let jalur = format!("{}[]", d.jalur);
                Some(Tempat {
                    id: d.id,
                    ty,
                    ubah: d.ubah,
                    akar: d.akar,
                    jalur,
                })
            }
            _ => None,
        }
    }

    fn tempat_tugas(&mut self, e: &Expr, konteks: &str) -> Option<Tempat> {
        let t = self.tempat(e);
        if t.is_none() {
            self.galat(
                e.span,
                format!(
                    "{} memerlukan variabel atau medan, bukan ekspresi sementara",
                    konteks
                ),
                "E0301",
            );
        }
        t
    }

    fn perlu_tempat(&mut self, e: &Expr, konteks: &str) -> Option<Tempat> {
        let t = self.tempat_tugas(e, konteks);
        if t.is_some() {
            self.periksa_indeks_tempat(e);
        }
        t
    }

    /// Tempat yang boleh ditulis harus mengevaluasi indeksnya, supaya
    /// metadata panggilan (dan pemindahan argumen) tidak hilang.
    fn periksa_indeks_tempat(&mut self, e: &Expr) {
        match &e.kind {
            ExprKind::Indeks { dasar, indeks } => {
                self.periksa_indeks_tempat(dasar);
                let ti = self.eks(indeks, Some(&Ty::Bil));
                if !self.bulat(&ti) {
                    self.galat(
                        indeks.span,
                        format!("indeks harus bilangan bulat, bukan `{}`", ti.sebutan()),
                        "E0335",
                    );
                }
            }
            ExprKind::Medan { dasar, .. } => self.periksa_indeks_tempat(dasar),
            _ => {}
        }
    }

    /// Tempat tujuan harus tetap hidup setelah nilai dan indeks dievaluasi.
    /// Pemindahan tepat pada medan tujuan diperbolehkan karena penugasan
    /// mengisinya kembali; pemindahan leluhur/keturunannya tidak.
    fn cek_tempat_tujuan_terpindah(&self, t: &Tempat) -> Option<String> {
        let v = self.cari(&t.akar)?;
        if v.dipindah {
            return Some("seluruh nilai".to_string());
        }
        if t.jalur.is_empty() {
            return None;
        }
        let dasar = t.jalur.split('[').next().unwrap_or(&t.jalur);
        for p in &v.dipindah_medan {
            if p == &t.jalur {
                continue;
            }
            let konflik = p == dasar
                || p.starts_with(&format!("{}.", dasar))
                || dasar.starts_with(&format!("{}.", p));
            if konflik {
                return Some(format!("medan `{}`", p));
            }
        }
        None
    }

    fn periksa_tempat_tujuan(&mut self, t: &Tempat, span: Span) {
        if let Some(p) = self.cek_tempat_tujuan_terpindah(t) {
            self.galat(
                span,
                format!("tempat tujuan `{}` tidak lagi hidup: {}", t.akar, p),
                "E0320",
            );
        }
        if let Some(p) = self.pinjaman_aktif(t.id, &t.akar, false) {
            if self.pinjaman_dari_lain(t.id) {
                self.galat(
                    span,
                    format!("tidak boleh menulis `{}` selama ada {}", t.akar, p),
                    "E0307",
                );
            }
        }
    }

    /// `Some(jejak)` bila `akar.jalur` tidak boleh dipakai karena sudah
    /// dipindah (seluruhnya, leluhurnya, atau salah satu keturunannya).
    fn cek_terpindah(&self, akar: &str, jalur: &str) -> Option<String> {
        let v = self.cari(akar)?;
        if v.dipindah {
            return Some("seluruh nilai".to_string());
        }
        if jalur.is_empty() {
            return v.dipindah_medan.first().map(|p| format!("medan `{}`", p));
        }
        for p in &v.dipindah_medan {
            let konflik = p == jalur
                || p.starts_with(&format!("{}.", jalur))
                || jalur.starts_with(&format!("{}.", p));
            if konflik {
                return Some(format!("medan `{}`", p));
            }
        }
        None
    }

    fn tandai_pindah(&mut self, akar: &str, jalur: &str) {
        if let Some(v) = self.cari_mut(akar) {
            if jalur.is_empty() {
                v.dipindah = true;
                v.dipindah_medan.clear();
            } else {
                v.dipindah_medan
                    .retain(|p| p != jalur && !p.starts_with(&format!("{}.", jalur)));
                v.dipindah_medan.push(jalur.to_string());
            }
        }
    }

    fn bersihkan_pindah(&mut self, akar: &str, jalur: &str) {
        if let Some(v) = self.cari_mut(akar) {
            if jalur.is_empty() {
                v.dipindah = false;
                v.dipindah_medan.clear();
            } else {
                v.dipindah_medan
                    .retain(|p| p != jalur && !p.starts_with(&format!("{}.", jalur)));
            }
        }
    }

    fn blok(&mut self, b: &Block) {
        self.masuk();
        for st in &b.stmts {
            self.stmt(st);
            self.akhir_pernyataan();
        }
        self.keluar();
    }

    fn stmt(&mut self, st: &Stmt) {
        match st {
            Stmt::Ikat {
                tetap,
                nama,
                ty,
                nilai,
                span,
            } => {
                if let Some(t) = ty {
                    if !self.s.tipe_sah(t) {
                        self.galat(
                            *span,
                            format!("tipe `{}` belum dikenal", t.sebutan()),
                            "E0203",
                        );
                    }
                }
                let akar_awal = match &nilai.kind {
                    ExprKind::Alamat { dasar, .. } => self.tempat(dasar).map(|t| (t.id, t.akar)),
                    _ => None,
                };
                let t_nilai = self.eks(nilai, ty.as_ref());
                if matches!(t_nilai, Ty::Ref { .. }) && !*tetap {
                    self.galat(
                        *span,
                        format!("variabel peminjam `{}` harus `tetap`", nama),
                        "E0302",
                    );
                }
                if let Some((id, _akar)) = akar_awal {
                    if let Some(daftar) = self.ctx.pinjaman.get_mut(&id) {
                        if let Some(last) = daftar.last_mut() {
                            last.oleh = Some(nama.clone());
                            last.kedalaman = self.ctx.kedalaman;
                        }
                    }
                }
                let t_final = match ty {
                    Some(t) => {
                        if !self.cocok(t, &t_nilai) {
                            self.galat(
                                nilai.span,
                                format!(
                                    "variabel `{}` bertipe `{}` tetapi nilai bertipe `{}`",
                                    nama,
                                    t.sebutan(),
                                    t_nilai.sebutan()
                                ),
                                "E0303",
                            );
                        }
                        t.clone()
                    }
                    None => {
                        if matches!(t_nilai, Ty::Kosong) {
                            self.galat(
                                *span,
                                format!("variabel `{}` tidak boleh bertipe `kosong`", nama),
                                "E0304",
                            );
                        }
                        t_nilai.clone()
                    }
                };
                if self.ctx.lingkup.last().unwrap().contains_key(nama) {
                    self.galat(
                        *span,
                        format!("`{}` sudah dideklarasikan pada blok ini", nama),
                        "E0305",
                    );
                }
                let k = self.ctx.kedalaman;
                let id = self.id_var();
                self.ctx.lingkup.last_mut().unwrap().insert(
                    nama.clone(),
                    Var {
                        id,
                        ty: t_final.clone(),
                        ubah: !*tetap,
                        dipindah: false,
                        dipindah_medan: Vec::new(),
                        kedalaman: k,
                    },
                );
            }
            Stmt::Tugas {
                target,
                nilai,
                span,
            } => {
                // Nilai dievaluasi lebih dulu (sama seperti penafsir dan
                // codegen). Indeks tujuan baru dievaluasi setelahnya, agar
                // pembacaan nilai tidak salah dianggap terjadi setelah move.
                let tempat = self.tempat_tugas(target, "penugasan");
                if let Some(t) = &tempat {
                    if !t.ubah && self.takaman == 0 {
                        self.galat(
                            *span,
                            format!(
                                "`{}` bersifat tetap; deklarasikan dengan `ubah` bila perlu diubah",
                                t.akar
                            ),
                            "E0306",
                        );
                    }
                    if let Some(p) = self.pinjaman_aktif(t.id, &t.akar, false) {
                        if self.pinjaman_dari_lain(t.id) {
                            self.galat(
                                *span,
                                format!("tidak boleh menulis `{}` selama ada {}", t.akar, p),
                                "E0307",
                            );
                        }
                    }
                }
                let t_target = tempat.as_ref().map(|t| t.ty.clone()).unwrap_or_else(|| {
                    self.ctx.jangan_pindah += 1;
                    let t = self.eks(target, None);
                    self.ctx.jangan_pindah -= 1;
                    t
                });
                if matches!(t_target, Ty::Ref { .. }) {
                    self.galat(*span, "pinjaman tidak dapat dipindahkan ke variabel lain; buat pinjaman baru dengan `&`".into(), "E0370");
                }
                let t_nilai = self.eks(nilai, Some(&t_target));
                if !self.cocok(&t_target, &t_nilai) {
                    self.galat(
                        nilai.span,
                        format!(
                            "tidak dapat menugaskan `{}` ke tempat bertipe `{}`",
                            t_nilai.sebutan(),
                            t_target.sebutan()
                        ),
                        "E0308",
                    );
                }
                if let Some(t) = &tempat {
                    if !matches!(target.kind, ExprKind::Var(_)) {
                        self.periksa_indeks_tempat(target);
                        self.periksa_tempat_tujuan(t, *span);
                    }
                    self.bersihkan_pindah(&t.akar, &t.jalur);
                }
            }
            Stmt::Jika {
                kond, maka, lain, ..
            } => {
                let t = self.eks(kond, Some(&Ty::Log));
                if t != Ty::Log {
                    self.galat(
                        kond.span,
                        format!("kondisi `jika` harus `log`, bukan `{}`", t.sebutan()),
                        "E0309",
                    );
                }
                self.blok(maka);
                if let Some(l) = lain {
                    self.blok(l);
                }
            }
            Stmt::Selama {
                kond,
                invarian,
                badan,
                ..
            } => {
                let t = self.eks(kond, Some(&Ty::Log));
                if t != Ty::Log {
                    self.galat(
                        kond.span,
                        format!("kondisi `selama` harus `log`, bukan `{}`", t.sebutan()),
                        "E0309",
                    );
                }
                for inv in invarian {
                    let t = self.eks(inv, Some(&Ty::Log));
                    if t != Ty::Log {
                        self.galat(
                            inv.span,
                            format!("invarian harus `log`, bukan `{}`", t.sebutan()),
                            "E0309",
                        );
                    }
                }
                self.ctx.dalam_selama += 1;
                self.blok(badan);
                self.ctx.dalam_selama -= 1;
            }
            Stmt::UntukRentang {
                nama,
                dari,
                sampai,
                badan,
                ..
            } => {
                let td = self.eks(dari, Some(&Ty::Bil));
                let ts = self.eks(sampai, Some(&Ty::Bil));
                if !self.bulat(&td) {
                    self.galat(
                        dari.span,
                        format!(
                            "batas `untuk` harus bilangan bulat, bukan `{}`",
                            td.sebutan()
                        ),
                        "E0310",
                    );
                }
                if !self.bulat(&ts) {
                    self.galat(
                        sampai.span,
                        format!(
                            "batas `untuk` harus bilangan bulat, bukan `{}`",
                            ts.sebutan()
                        ),
                        "E0311",
                    );
                }
                self.masuk();
                self.ctx.dalam_selama += 1;
                let k = self.ctx.kedalaman;
                let id = self.id_var();
                self.ctx.lingkup.last_mut().unwrap().insert(
                    nama.clone(),
                    Var {
                        id,
                        ty: Ty::Bil,
                        ubah: false,
                        dipindah: false,
                        dipindah_medan: Vec::new(),
                        kedalaman: k,
                    },
                );
                for st in &badan.stmts {
                    self.stmt(st);
                    self.akhir_pernyataan();
                }
                self.ctx.dalam_selama -= 1;
                self.keluar();
            }
            Stmt::UntukIsi {
                nama, wadah, badan, ..
            } => {
                self.ctx.jangan_pindah += 1;
                let tw = self.eks(wadah, None);
                self.ctx.jangan_pindah -= 1;
                // Pinjam wadah selama badan loop. Tanpa ini, `tambah` di badan
                // bisa merealokasi buffer sementara kode native masih memakai
                // pointer lama (use-after-free).
                let wadah_tempat = self.tempat(wadah);
                let elemen = match &tw {
                    Ty::Array(t, _) => (**t).clone(),
                    Ty::Irisan(t) => (**t).clone(),
                    Ty::Daftar(t) => (**t).clone(),
                    Ty::Teks => Ty::Huruf,
                    lain => {
                        self.galat(wadah.span, format!("`untuk ... dalam ...` memerlukan larik, irisan, `teks`, atau `Daftar`, bukan `{}`", lain.sebutan()), "E0312");
                        Ty::Bil
                    }
                };
                if !self.s.boleh_salin(&elemen) {
                    self.galat(
                        wadah.span,
                        format!(
                            "elemen `{}` tidak dapat disalin; iterasi dengan indeks lalu `ambil`",
                            elemen.sebutan()
                        ),
                        "E0313",
                    );
                }
                self.s.elemen_untuk.insert(wadah.id, elemen.clone());
                self.masuk();
                self.ctx.dalam_selama += 1;
                if let Some(t) = wadah_tempat {
                    self.ctx.pinjaman.entry(t.id).or_default().push(Pinjaman {
                        ubah: false,
                        oleh: Some(format!("iterasi `{}`", nama)),
                        kedalaman: self.ctx.kedalaman,
                    });
                }
                let k = self.ctx.kedalaman;
                let id = self.id_var();
                self.ctx.lingkup.last_mut().unwrap().insert(
                    nama.clone(),
                    Var {
                        id,
                        ty: elemen,
                        ubah: false,
                        dipindah: false,
                        dipindah_medan: Vec::new(),
                        kedalaman: k,
                    },
                );
                for st in &badan.stmts {
                    self.stmt(st);
                    self.akhir_pernyataan();
                }
                self.ctx.dalam_selama -= 1;
                self.keluar();
            }
            Stmt::Kembali(nilai, span) => match nilai {
                Some(e) => {
                    if self.ctx.hasil == Ty::Kosong {
                        self.galat(
                            *span,
                            format!("fungsi `{}` tidak mengembalikan nilai", self.fungsi),
                            "E0314",
                        );
                    }
                    let harap = self.ctx.hasil.clone();
                    let t = self.eks(e, Some(&harap));
                    if !self.cocok(&self.ctx.hasil, &t) {
                        self.galat(
                            e.span,
                            format!(
                                "`kembali` mengharapkan `{}`, tetapi nilainya `{}`",
                                self.ctx.hasil.sebutan(),
                                t.sebutan()
                            ),
                            "E0315",
                        );
                    }
                }
                None => {
                    if self.ctx.hasil != Ty::Kosong {
                        self.galat(
                            *span,
                            format!(
                                "fungsi `{}` harus mengembalikan `{}`",
                                self.fungsi,
                                self.ctx.hasil.sebutan()
                            ),
                            "E0316",
                        );
                    }
                }
            },
            Stmt::Tegas { kond, .. } => {
                let t = self.eks(kond, Some(&Ty::Log));
                if t != Ty::Log {
                    self.galat(
                        kond.span,
                        format!("`tegas` memerlukan kondisi `log`, bukan `{}`", t.sebutan()),
                        "E0309",
                    );
                }
            }
            Stmt::Ekspresi(e) => {
                let t = self.eks(e, None);
                if t != Ty::Kosong {
                    self.galat(
                        e.span,
                        format!(
                            "ekspresi bertipe `{}` tidak dipakai; buang atau simpan ke variabel",
                            t.sebutan()
                        ),
                        "E0317",
                    );
                }
            }
            Stmt::Henti(span) | Stmt::Lanjut(span) => {
                if self.ctx.dalam_selama == 0 {
                    self.galat(
                        *span,
                        "`henti`/`lanjut` hanya sah di dalam `selama` atau `untuk`".into(),
                        "E0318",
                    );
                }
            }
            Stmt::Cocok {
                nilai,
                kasus,
                lainnya,
                span,
            } => {
                let t = self.eks(nilai, None);
                let info = match &t {
                    Ty::Nama(n) => match self.s.enumerasi.get(n).cloned() {
                        Some(i) => i,
                        None => {
                            self.galat(
                                nilai.span,
                                format!("`cocok` memerlukan enumerasi, bukan `{}`", t.sebutan()),
                                "E0379",
                            );
                            for (_, b) in kasus {
                                self.blok(b);
                            }
                            if let Some(b) = lainnya {
                                self.blok(b);
                            }
                            return;
                        }
                    },
                    _ => {
                        self.galat(
                            nilai.span,
                            format!("`cocok` memerlukan enumerasi, bukan `{}`", t.sebutan()),
                            "E0379",
                        );
                        for (_, b) in kasus {
                            self.blok(b);
                        }
                        if let Some(b) = lainnya {
                            self.blok(b);
                        }
                        return;
                    }
                };
                let mut terlihat = HashSet::new();
                for (v, b) in kasus {
                    if !info.varian.iter().any(|x| x == v) {
                        self.galat(
                            *span,
                            format!(
                                "enumerasi `{}` tidak punya varian `{}` (varian: {})",
                                t.sebutan(),
                                v,
                                info.varian.join(", ")
                            ),
                            "E0380",
                        );
                    } else if !terlihat.insert(v.clone()) {
                        self.galat(
                            *span,
                            format!("varian `{}` muncul lebih dari sekali pada `cocok`", v),
                            "E0381",
                        );
                    }
                    self.blok(b);
                }
                if lainnya.is_none() && terlihat.len() != info.varian.len() {
                    let kurang: Vec<String> = info
                        .varian
                        .iter()
                        .filter(|v| !terlihat.contains(*v))
                        .cloned()
                        .collect();
                    self.galat(
                        *span,
                        format!("`cocok` belum menangani varian: {}", kurang.join(", ")),
                        "E0382",
                    );
                }
                if let Some(b) = lainnya {
                    self.blok(b);
                }
            }
            Stmt::Blok(b) => self.blok(b),
            Stmt::Takaman(b) => {
                if !self.boleh_takaman {
                    self.galat(
                        b.span,
                        "blok `takaman` memerlukan opsi `--boleh-takaman`".into(),
                        "E0385",
                    );
                }
                self.takaman += 1;
                self.blok(b);
                self.takaman -= 1;
            }
        }
    }

    fn pinjaman_dari_lain(&self, id: u64) -> bool {
        self.ctx
            .pinjaman
            .get(&id)
            .map(|d| d.iter().any(|p| p.oleh.is_some()))
            .unwrap_or(false)
    }

    fn bulat(&self, t: &Ty) -> bool {
        matches!(t, Ty::Bil | Ty::Bil32 | Ty::Bil8 | Ty::Nat | Ty::Nat8)
    }

    fn angka(&self, t: &Ty) -> bool {
        matches!(
            t,
            Ty::Bil | Ty::Bil32 | Ty::Bil8 | Ty::Nat | Ty::Nat8 | Ty::Pecah
        )
    }

    fn cocok(&self, harap: &Ty, dapat: &Ty) -> bool {
        if harap == dapat {
            return true;
        }
        matches!((harap, dapat), (Ty::Irisan(a), Ty::Array(b, _)) if a == b)
            || matches!((harap, dapat), (Ty::Irisan(a), Ty::Irisan(b)) if a == b)
    }

    fn eks(&mut self, e: &Expr, harap: Option<&Ty>) -> Ty {
        let t = self.eks_dalam(e, harap);
        self.s.tipe.insert(e.id, t.clone());
        t
    }

    fn eks_dalam(&mut self, e: &Expr, harap: Option<&Ty>) -> Ty {
        match &e.kind {
            ExprKind::Bil(v) => {
                let t = match harap {
                    Some(t @ (Ty::Bil | Ty::Bil32 | Ty::Bil8 | Ty::Nat | Ty::Nat8 | Ty::Huruf)) => {
                        t.clone()
                    }
                    _ => Ty::Bil,
                };
                if !nilai_muat(&t, *v) {
                    self.galat(
                        e.span,
                        format!("nilai {} tidak muat dalam tipe `{}`", v, t.sebutan()),
                        "E0319",
                    );
                }
                t
            }
            ExprKind::Huruf(_) => Ty::Huruf,
            ExprKind::Pecah(_) => Ty::Pecah,
            ExprKind::Teks(_) => Ty::Teks,
            ExprKind::Log(_) => Ty::Log,
            ExprKind::Var(n) => {
                if n == "hasil" && self.cari("hasil").is_none() {
                    if !self.ctx.dalam_jamin {
                        self.galat(
                            e.span,
                            "`hasil` hanya boleh dipakai pada klausa `jamin`".into(),
                            "E0374",
                        );
                    }
                    return self.ctx.hasil.clone();
                }
                match self.cari(n).cloned() {
                    Some(v) => {
                        let konflik = if self.ctx.jangan_pindah == 0 {
                            self.cek_terpindah(n, "")
                        } else if v.dipindah {
                            Some("seluruh nilai".to_string())
                        } else {
                            None
                        };
                        if let Some(p) = konflik {
                            self.galat(e.span, format!("`{}` sudah dipindahkan (move): {}; nilai tidak boleh dipakai lagi", n, p), "E0320");
                        }
                        if let Some(p) = self.pinjaman_aktif(v.id, n, true) {
                            self.galat(
                                e.span,
                                format!("`{}` sedang dipinjam secara eksklusif oleh {}", n, p),
                                "E0321",
                            );
                        }
                        if !self.s.boleh_salin(&v.ty) && self.ctx.jangan_pindah == 0 {
                            if let Some(p) = self.pinjaman_aktif(v.id, n, false) {
                                self.galat(
                                    e.span,
                                    format!("tidak dapat memindahkan `{}` selama ada {}", n, p),
                                    "E0331",
                                );
                            }
                            self.tandai_pindah(n, "");
                            self.s.pindah.insert(e.id);
                        }
                        v.ty
                    }
                    None => {
                        if self.s.sig.contains_key(n) {
                            self.galat(
                                e.span,
                                format!("`{}` adalah fungsi; panggil dengan `{}()`", n, n),
                                "E0322",
                            );
                        } else {
                            self.galat(
                                e.span,
                                format!("nama `{}` belum dideklarasikan", n),
                                "E0323",
                            );
                        }
                        Ty::Bil
                    }
                }
            }
            ExprKind::Biner(op, a, b) => {
                match op {
                    BinOp::Dan | BinOp::Atau => {
                        let ta = self.eks(a, Some(&Ty::Log));
                        let tb = self.eks(b, Some(&Ty::Log));
                        if ta != Ty::Log || tb != Ty::Log {
                            self.galat(
                                e.span,
                                format!("operator `{}` memerlukan dua `log`", op.simbol()),
                                "E0324",
                            );
                        }
                        Ty::Log
                    }
                    BinOp::Sama | BinOp::TidakSama => {
                        let ta = self.eks(a, None);
                        let tb = self.eks(b, Some(&ta));
                        let setara = ta == tb;
                        if !setara {
                            self.galat(
                                e.span,
                                format!(
                                    "tidak dapat membandingkan `{}` dengan `{}`",
                                    ta.sebutan(),
                                    tb.sebutan()
                                ),
                                "E0325",
                            );
                        }
                        Ty::Log
                    }
                    BinOp::Kecil | BinOp::KecilSama | BinOp::Besar | BinOp::BesarSama => {
                        let ta = self.eks(a, None);
                        let tb = self.eks(b, Some(&ta));
                        if ta != tb || !(self.angka(&ta) || ta == Ty::Teks) {
                            self.galat(e.span, format!("operator `{}` memerlukan dua nilai bertipe sama (angka atau teks)", op.simbol()), "E0326");
                        }
                        Ty::Log
                    }
                    BinOp::GeserKiri | BinOp::GeserKanan => {
                        let ta = self.eks(a, None);
                        let tb = self.eks(b, Some(&ta));
                        if !self.bulat(&ta) || !self.bulat(&tb) {
                            self.galat(
                                e.span,
                                format!(
                                    "operator `{}` hanya untuk bilangan bulat, bukan `{}` dan `{}`",
                                    op.simbol(),
                                    ta.sebutan(),
                                    tb.sebutan()
                                ),
                                "E0326",
                            );
                        } else if ta != tb {
                            self.galat(e.span, format!("operator `{}` memerlukan dua nilai bertipe sama, bukan `{}` dan `{}`", op.simbol(), ta.sebutan(), tb.sebutan()), "E0327");
                        }
                        ta
                    }
                    _ => {
                        let ta = self.eks(a, harap);
                        let tb = self.eks(b, Some(&ta));
                        if ta != tb {
                            self.galat(
                                b.span,
                                format!(
                                    "`{}` memerlukan dua nilai bertipe sama, bukan `{}` dan `{}`",
                                    op.simbol(),
                                    ta.sebutan(),
                                    tb.sebutan()
                                ),
                                "E0327",
                            );
                        }
                        if !self.angka(&ta) {
                            self.galat(
                                e.span,
                                format!(
                                    "operator `{}` hanya untuk angka, bukan `{}`",
                                    op.simbol(),
                                    ta.sebutan()
                                ),
                                "E0328",
                            );
                        }
                        ta
                    }
                }
            }
            ExprKind::Uner(op, a) => match op {
                UnOp::Neg => {
                    let t = self.eks(a, harap);
                    if !matches!(t, Ty::Bil | Ty::Bil32 | Ty::Bil8 | Ty::Pecah) {
                        self.galat(
                            a.span,
                            format!("`-` hanya untuk bilangan bertanda, bukan `{}`", t.sebutan()),
                            "E0329",
                        );
                    }
                    t
                }
                UnOp::Bukan => {
                    let t = self.eks(a, Some(&Ty::Log));
                    if t != Ty::Log {
                        self.galat(
                            a.span,
                            format!("`bukan` memerlukan `log`, bukan `{}`", t.sebutan()),
                            "E0330",
                        );
                    }
                    Ty::Log
                }
            },
            ExprKind::Alamat { ubah, dasar } => {
                if *ubah && matches!(dasar.kind, ExprKind::Medan { .. } | ExprKind::Indeks { .. }) {
                    self.galat(
                        e.span,
                        "peminjaman `&ubah` pada medan atau elemen belum didukung; salin ke variabel lokal lebih dulu".into(),
                        "E0375",
                    );
                    let ty = self.tempat(dasar).map(|t| t.ty).unwrap_or(Ty::Bil);
                    let hasil = Ty::Ref {
                        ubah: *ubah,
                        dalam: Box::new(ty),
                    };
                    self.s.tipe.insert(e.id, hasil.clone());
                    return hasil;
                }
                let t = match self.perlu_tempat(dasar, "peminjaman") {
                    Some(t) => t,
                    None => return Ty::Kosong,
                };
                if let Some(p) = self.cek_terpindah(&t.akar, &t.jalur) {
                    self.galat(
                        e.span,
                        format!(
                            "tidak dapat meminjam `{}` yang sudah dipindahkan: {}",
                            t.akar, p
                        ),
                        "E0331",
                    );
                }
                if *ubah {
                    if !t.ubah {
                        self.galat(
                            e.span,
                            format!("`&ubah {}` memerlukan variabel `ubah`", t.akar),
                            "E0332",
                        );
                    }
                    if let Some(p) = self.pinjaman_aktif(t.id, &t.akar, false) {
                        self.galat(
                            e.span,
                            format!(
                                "tidak dapat meminjam `&ubah {}` karena sudah ada {}",
                                t.akar, p
                            ),
                            "E0333",
                        );
                    }
                } else if let Some(p) = self.pinjaman_aktif(t.id, &t.akar, true) {
                    self.galat(
                        e.span,
                        format!("tidak dapat meminjam `&{}` karena sudah ada {}", t.akar, p),
                        "E0334",
                    );
                }
                let k = self.ctx.kedalaman;
                self.ctx.pinjaman.entry(t.id).or_default().push(Pinjaman {
                    ubah: *ubah,
                    oleh: None,
                    kedalaman: k,
                });
                let hasil = Ty::Ref {
                    ubah: *ubah,
                    dalam: Box::new(t.ty),
                };
                self.s.tipe.insert(e.id, hasil.clone());
                hasil
            }
            ExprKind::Indeks { dasar, indeks } => {
                self.ctx.jangan_pindah += 1;
                let td = self.eks(dasar, None);
                self.ctx.jangan_pindah -= 1;
                let ti = self.eks(indeks, Some(&Ty::Bil));
                if !self.bulat(&ti) {
                    self.galat(
                        indeks.span,
                        format!("indeks harus bilangan bulat, bukan `{}`", ti.sebutan()),
                        "E0335",
                    );
                }
                if let Some(t) = self.tempat(dasar) {
                    if let Some(p) = self.pinjaman_aktif(t.id, &t.akar, true) {
                        self.galat(
                            e.span,
                            format!("`{}` sedang dipinjam secara eksklusif oleh {}", t.akar, p),
                            "E0336",
                        );
                    }
                }
                match td.clone() {
                    Ty::Array(t, _) | Ty::Irisan(t) | Ty::Daftar(t) => *t,
                    Ty::Teks => Ty::Huruf,
                    Ty::Ref { dalam, .. } => match (*dalam).clone() {
                        Ty::Array(t, _) | Ty::Irisan(t) | Ty::Daftar(t) => *t,
                        _ => {
                            self.galat(
                                dasar.span,
                                format!("tipe `{}` tidak dapat diindeks", td.sebutan()),
                                "E0337",
                            );
                            Ty::Bil
                        }
                    },
                    lain => {
                        self.galat(
                            dasar.span,
                            format!("tipe `{}` tidak dapat diindeks", lain.sebutan()),
                            "E0337",
                        );
                        Ty::Bil
                    }
                }
            }
            ExprKind::Medan { dasar, nama } => {
                self.ctx.jangan_pindah += 1;
                let td = self.eks(dasar, None);
                self.ctx.jangan_pindah -= 1;
                let tempat_dasar = self.tempat(dasar);
                let jalur_medan = tempat_dasar.as_ref().map(|t| {
                    if t.jalur.is_empty() {
                        nama.clone()
                    } else {
                        format!("{}.{}", t.jalur, nama)
                    }
                });
                if let Some(t) = &tempat_dasar {
                    if let Some(p) = self.pinjaman_aktif(t.id, &t.akar, true) {
                        self.galat(
                            e.span,
                            format!("`{}` sedang dipinjam secara eksklusif oleh {}", t.akar, p),
                            "E0336",
                        );
                    }
                    if let Some(jalur) = &jalur_medan {
                        if let Some(p) = self.cek_terpindah(&t.akar, jalur) {
                            let nama_penuh = format!("{}.{}", t.akar, jalur);
                            self.galat(
                                e.span,
                                format!("`{}` tidak boleh dipakai: {}", nama_penuh, p),
                                "E0320",
                            );
                        }
                    }
                }
                let nama_struk = match &td {
                    Ty::Nama(n) => Some(n.clone()),
                    Ty::Ref { dalam, .. } => match &**dalam {
                        Ty::Nama(n) => Some(n.clone()),
                        _ => None,
                    },
                    _ => None,
                };
                let ty_medan =
                    match nama_struk.and_then(|n| self.s.struk.get(&n).cloned().map(|s| (n, s))) {
                        Some((n, info)) => match info.indeks.get(nama) {
                            Some(i) => info.medan[*i].1.clone(),
                            None => {
                                let daftar: Vec<String> =
                                    info.medan.iter().map(|(m, _)| m.clone()).collect();
                                self.galat(
                                    e.span,
                                    format!(
                                        "struk `{}` tidak punya medan `{}` (medan: {})",
                                        n,
                                        nama,
                                        daftar.join(", ")
                                    ),
                                    "E0338",
                                );
                                Ty::Bil
                            }
                        },
                        None => {
                            self.galat(
                                e.span,
                                format!(
                                    "`{}` bukan struk, jadi tidak punya medan `{}`",
                                    td.sebutan(),
                                    nama
                                ),
                                "E0339",
                            );
                            Ty::Bil
                        }
                    };
                if !self.s.boleh_salin(&ty_medan) && self.ctx.jangan_pindah == 0 {
                    if matches!(td, Ty::Ref { .. }) {
                        self.galat(e.span, "tidak dapat memindahkan medan keluar dari pinjaman; salin dulu ke variabel lokal".into(), "E0377");
                    } else if let (Some(t), Some(jalur)) = (&tempat_dasar, &jalur_medan) {
                        if let Some(p) = self.pinjaman_aktif(t.id, &t.akar, false) {
                            self.galat(
                                e.span,
                                format!(
                                    "tidak dapat memindahkan `{}.{}` selama ada {}",
                                    t.akar, jalur, p
                                ),
                                "E0331",
                            );
                        }
                        self.tandai_pindah(&t.akar, jalur);
                    }
                    self.s.pindah.insert(e.id);
                }
                ty_medan
            }
            ExprKind::Sebagai { nilai, ty } => {
                let asal = self.eks(nilai, None);
                if !self.s.tipe_sah(ty) {
                    self.galat(
                        e.span,
                        format!("tipe `{}` belum dikenal", ty.sebutan()),
                        "E0203",
                    );
                }
                let sah =
                    self.angka(&asal) && self.angka(ty) && asal != Ty::Pecah && *ty != Ty::Pecah;
                let sah = sah
                    || (asal == Ty::Pecah && self.bulat(ty))
                    || (self.bulat(&asal) && *ty == Ty::Pecah)
                    || (asal == Ty::Huruf && self.bulat(ty))
                    || (self.bulat(&asal) && *ty == Ty::Huruf)
                    || asal == *ty;
                if !sah {
                    self.galat(
                        e.span,
                        format!(
                            "konversi `sebagai` dari `{}` ke `{}` tidak diizinkan",
                            asal.sebutan(),
                            ty.sebutan()
                        ),
                        "E0340",
                    );
                }
                ty.clone()
            }
            ExprKind::StrukLit { nama, medan } => {
                let info = match self.s.struk.get(nama).cloned() {
                    Some(i) => i,
                    None => {
                        if self.s.enumerasi.contains_key(nama) {
                            self.galat(
                                e.span,
                                format!("`{}` adalah enumerasi; gunakan `{}::Varian`", nama, nama),
                                "E0341",
                            );
                        } else {
                            self.galat(
                                e.span,
                                format!("struk `{}` belum dideklarasikan", nama),
                                "E0342",
                            );
                        }
                        for (_, mv) in medan {
                            self.eks(mv, None);
                        }
                        return Ty::Kosong;
                    }
                };
                let mut dilihat = vec![false; info.medan.len()];
                for (mn, mv) in medan {
                    match info.indeks.get(mn) {
                        Some(i) => {
                            if dilihat[*i] {
                                self.galat(
                                    mv.span,
                                    format!("medan `{}` diisi dua kali", mn),
                                    "E0343",
                                );
                            }
                            dilihat[*i] = true;
                            let harap_m = info.medan[*i].1.clone();
                            let tm = self.eks(mv, Some(&harap_m));
                            if !self.cocok(&harap_m, &tm) {
                                self.galat(
                                    mv.span,
                                    format!(
                                        "medan `{}` bertipe `{}`, tetapi diisi `{}`",
                                        mn,
                                        harap_m.sebutan(),
                                        tm.sebutan()
                                    ),
                                    "E0344",
                                );
                            }
                        }
                        None => {
                            let daftar: Vec<String> =
                                info.medan.iter().map(|(m, _)| m.clone()).collect();
                            self.galat(
                                mv.span,
                                format!(
                                    "struk `{}` tidak punya medan `{}` (medan: {})",
                                    nama,
                                    mn,
                                    daftar.join(", ")
                                ),
                                "E0345",
                            );
                            self.eks(mv, None);
                        }
                    }
                }
                for (i, (mn, _)) in info.medan.iter().enumerate() {
                    if !dilihat[i] {
                        self.galat(
                            e.span,
                            format!("medan `{}` belum diisi pada literal `{}`", mn, nama),
                            "E0346",
                        );
                    }
                }
                Ty::Nama(nama.clone())
            }
            ExprKind::Varian { enumerasi, varian } => {
                match self.s.enumerasi.get(enumerasi).cloned() {
                    Some(info) => {
                        if !info.varian.contains(varian) {
                            self.galat(
                                e.span,
                                format!(
                                    "enumerasi `{}` tidak punya varian `{}` (varian: {})",
                                    enumerasi,
                                    varian,
                                    info.varian.join(", ")
                                ),
                                "E0347",
                            );
                        }
                        Ty::Nama(enumerasi.clone())
                    }
                    None => {
                        self.galat(
                            e.span,
                            format!("enumerasi `{}` belum dideklarasikan", enumerasi),
                            "E0348",
                        );
                        Ty::Kosong
                    }
                }
            }
            ExprKind::ArrayLit(isi) => {
                let harap_elemen = match harap {
                    Some(Ty::Array(t, _)) => Some((**t).clone()),
                    Some(Ty::Irisan(t)) => Some((**t).clone()),
                    _ => None,
                };
                if isi.is_empty() {
                    self.galat(
                        e.span,
                        "larik kosong `[]` tidak dapat disimpulkan tipenya".into(),
                        "E0349",
                    );
                    return Ty::Array(Box::new(Ty::Bil), 0);
                }
                let t0 = self.eks(&isi[0], harap_elemen.as_ref());
                if self.s.memuat_heap(&t0) || !self.s.boleh_salin(&t0) {
                    self.galat(
                        e.span,
                        format!(
                            "larik tidak boleh berisi tipe `{}` yang memiliki memori heap atau tidak dapat disalin",
                            t0.sebutan()
                        ),
                        "E0203",
                    );
                }
                for x in &isi[1..] {
                    let tx = self.eks(x, Some(&t0));
                    if !self.cocok(&t0, &tx) {
                        self.galat(
                            x.span,
                            format!(
                                "elemen larik bertipe `{}`, sebelumnya `{}`",
                                tx.sebutan(),
                                t0.sebutan()
                            ),
                            "E0350",
                        );
                    }
                }
                if let Some(h) = harap_elemen {
                    if !self.cocok(&h, &t0) {
                        self.galat(
                            e.span,
                            format!(
                                "larik `{}` tidak cocok dengan `{}`",
                                t0.sebutan(),
                                h.sebutan()
                            ),
                            "E0351",
                        );
                    }
                }
                Ty::Array(Box::new(t0), isi.len())
            }
            ExprKind::Panggil { nama, args } => self.panggil(e, nama, args, harap),
        }
    }

    fn metode_ubah(&mut self, e: &Expr, args: &[Expr], nama: &str) -> Ty {
        let butuh = if nama == "tambah" { 2 } else { 3 };
        if args.len() != butuh {
            self.galat(
                e.span,
                format!("metode `{}` memerlukan {} argumen", nama, butuh),
                "E0359",
            );
            for a in args {
                self.eks(a, None);
            }
            return Ty::Kosong;
        }
        if matches!(
            args[0].kind,
            ExprKind::Medan { .. } | ExprKind::Indeks { .. }
        ) {
            self.galat(
                args[0].span,
                "metode pada medan/elemen belum didukung; salin wadah ke variabel lokal".into(),
                "E0375",
            );
            for a in args {
                self.eks(a, None);
            }
            return Ty::Kosong;
        }
        let tempat = match self.perlu_tempat(&args[0], "penerima metode") {
            Some(t) => t,
            None => return Ty::Kosong,
        };
        if !tempat.ubah {
            self.galat(
                args[0].span,
                format!("metode `{}` memerlukan penerima `ubah`", nama),
                "E0383",
            );
        }
        if let Some(p) = self.pinjaman_aktif(tempat.id, &tempat.akar, false) {
            self.galat(
                args[0].span,
                format!("tidak dapat mengubah `{}` selama ada {}", tempat.akar, p),
                "E0336",
            );
        }
        let elemen = match &tempat.ty {
            Ty::Daftar(t) => (**t).clone(),
            Ty::Ref { ubah: true, dalam } => match &**dalam {
                Ty::Daftar(t) => (**t).clone(),
                _ => Ty::Bil,
            },
            _ => {
                self.galat(
                    args[0].span,
                    format!("metode `{}` hanya berlaku untuk `Daftar<T>`", nama),
                    "E0384",
                );
                Ty::Bil
            }
        };
        if nama == "tambah" {
            let t1 = self.eks(&args[1], Some(&elemen));
            if !self.cocok(&elemen, &t1) {
                self.galat(
                    args[1].span,
                    format!(
                        "`tambah` mengharapkan `{}`, tetapi diberi `{}`",
                        elemen.sebutan(),
                        t1.sebutan()
                    ),
                    "E0361",
                );
            }
        } else {
            let ti = self.eks(&args[1], Some(&Ty::Bil));
            if !self.bulat(&ti) {
                self.galat(
                    args[1].span,
                    format!(
                        "indeks `sisip` harus bilangan bulat, bukan `{}`",
                        ti.sebutan()
                    ),
                    "E0365",
                );
            }
            let tv = self.eks(&args[2], Some(&elemen));
            if !self.cocok(&elemen, &tv) {
                self.galat(
                    args[2].span,
                    format!(
                        "`sisip` mengharapkan `{}`, tetapi diberi `{}`",
                        elemen.sebutan(),
                        tv.sebutan()
                    ),
                    "E0361",
                );
            }
        }
        self.s.metode.insert(e.id);
        self.s
            .panggil
            .insert(e.id, Panggilan::Bawaan(nama.to_string()));
        Ty::Kosong
    }

    fn panggil(&mut self, e: &Expr, nama_awal: &str, args: &[Expr], harap: Option<&Ty>) -> Ty {
        let mut nama = nama_awal.to_string();
        let metode = nama.starts_with("__metode__");
        if metode {
            nama = nama.trim_start_matches("__metode__").to_string();
        }
        nama = bawaan_kanonik(&nama).to_string();
        if metode && !self.s.baatan.iter().any(|b| b == &nama) {
            self.galat(e.span, format!("tipe tidak punya metode `{}` (v0.1 hanya punya metode bawaan: panjang, ambil, tambah, sisip)", nama), "E0352");
            for a in args {
                self.eks(a, None);
            }
            return Ty::Bil;
        }
        if !self.s.baatan.iter().any(|b| b == &nama) {
            let sig = match self.s.sig.get(&nama).cloned() {
                Some(s) => s,
                None => {
                    self.galat(
                        e.span,
                        format!("fungsi `{}` belum dideklarasikan", nama),
                        "E0353",
                    );
                    for a in args {
                        self.eks(a, None);
                    }
                    return Ty::Kosong;
                }
            };
            if args.len() != sig.params.len() {
                self.galat(
                    e.span,
                    format!(
                        "`{}` memerlukan {} argumen, tetapi diberi {}",
                        nama,
                        sig.params.len(),
                        args.len()
                    ),
                    "E0354",
                );
            }
            for (i, a) in args.iter().enumerate() {
                match sig.params.get(i) {
                    Some(p) => {
                        let simpan = self.ctx.jangan_pindah;
                        self.ctx.jangan_pindah = 0;
                        let t = self.eks(a, Some(p));
                        self.ctx.jangan_pindah = simpan;
                        if !self.cocok(p, &t) {
                            self.galat(
                                a.span,
                                format!(
                                    "argumen ke-{} `{}` bertipe `{}`, diharapkan `{}`",
                                    i + 1,
                                    nama,
                                    t.sebutan(),
                                    p.sebutan()
                                ),
                                "E0355",
                            );
                        }
                    }
                    None => {
                        self.eks(a, None);
                    }
                }
            }
            self.s.panggil.insert(e.id, Panggilan::Fungsi(nama));
            return sig.hasil;
        }
        if metode && (nama == "tambah" || nama == "sisip") {
            return self.metode_ubah(e, args, &nama);
        }
        self.s.panggil.insert(e.id, Panggilan::Bawaan(nama.clone()));
        match nama.as_str() {
            "cetak" | "tulis" => {
                for a in args {
                    let t = self.eks(a, None);
                    let enum_sah = matches!(&t, Ty::Nama(n) if self.s.enumerasi.contains_key(n));
                    if !enum_sah
                        && !matches!(
                            t,
                            Ty::Bil
                                | Ty::Bil32
                                | Ty::Bil8
                                | Ty::Nat
                                | Ty::Nat8
                                | Ty::Pecah
                                | Ty::Log
                                | Ty::Teks
                                | Ty::Huruf
                        )
                    {
                        self.galat(
                            a.span,
                            format!("`{}` tidak dapat mencetak tipe `{}`", nama, t.sebutan()),
                            "E0356",
                        );
                    }
                }
                Ty::Kosong
            }
            "daftar" => {
                if !args.is_empty() {
                    self.galat(e.span, "`daftar()` tidak menerima argumen".into(), "E0357");
                }
                match harap {
                    Some(Ty::Daftar(t)) => Ty::Daftar(t.clone()),
                    _ => {
                        self.galat(e.span, "`daftar()` memerlukan anotasi tipe, mis. `ubah d: Daftar<bil> = daftar()`".into(), "E0358");
                        Ty::Daftar(Box::new(Ty::Bil))
                    }
                }
            }
            "tambah" => {
                if args.len() != 2 {
                    self.galat(
                        e.span,
                        "`tambah` memerlukan 2 argumen: wadah dan nilai".into(),
                        "E0359",
                    );
                    for a in args {
                        self.eks(a, None);
                    }
                    return Ty::Kosong;
                }
                self.ctx.jangan_pindah += 1;
                let t0 = self.eks(&args[0], None);
                self.ctx.jangan_pindah -= 1;
                let elemen = match &t0 {
                    Ty::Ref { ubah: true, dalam } => match &**dalam {
                        Ty::Daftar(t) => (**t).clone(),
                        _ => {
                            self.galat(
                                args[0].span,
                                "`tambah` memerlukan `&ubah Daftar<T>`".into(),
                                "E0360",
                            );
                            Ty::Bil
                        }
                    },
                    _ => {
                        self.galat(
                            args[0].span,
                            "`tambah` memerlukan `&ubah Daftar<T>` (tulis `tambah(&ubah d, x)`)"
                                .into(),
                            "E0360",
                        );
                        Ty::Bil
                    }
                };
                let t1 = self.eks(&args[1], Some(&elemen));
                if !self.cocok(&elemen, &t1) {
                    self.galat(
                        args[1].span,
                        format!(
                            "`tambah` mengharapkan `{}`, tetapi diberi `{}`",
                            elemen.sebutan(),
                            t1.sebutan()
                        ),
                        "E0361",
                    );
                }
                Ty::Kosong
            }
            "sisip" => {
                if args.len() != 3 {
                    self.galat(
                        e.span,
                        "`sisip` memerlukan 3 argumen: wadah, indeks, dan nilai".into(),
                        "E0359",
                    );
                    for a in args {
                        self.eks(a, None);
                    }
                    return Ty::Kosong;
                }
                self.ctx.jangan_pindah += 1;
                let t0 = self.eks(&args[0], None);
                self.ctx.jangan_pindah -= 1;
                let elemen = match &t0 {
                    Ty::Ref { ubah: true, dalam } => match &**dalam {
                        Ty::Daftar(t) => (**t).clone(),
                        _ => {
                            self.galat(
                                args[0].span,
                                "`sisip` memerlukan `&ubah Daftar<T>`".into(),
                                "E0360",
                            );
                            Ty::Bil
                        }
                    },
                    _ => {
                        self.galat(
                            args[0].span,
                            "`sisip` memerlukan `&ubah Daftar<T>` (tulis `sisip(&ubah d, i, x)`)"
                                .into(),
                            "E0360",
                        );
                        Ty::Bil
                    }
                };
                let ti = self.eks(&args[1], Some(&Ty::Bil));
                if !self.bulat(&ti) {
                    self.galat(
                        args[1].span,
                        format!(
                            "indeks `sisip` harus bilangan bulat, bukan `{}`",
                            ti.sebutan()
                        ),
                        "E0365",
                    );
                }
                let tv = self.eks(&args[2], Some(&elemen));
                if !self.cocok(&elemen, &tv) {
                    self.galat(
                        args[2].span,
                        format!(
                            "`sisip` mengharapkan `{}`, tetapi diberi `{}`",
                            elemen.sebutan(),
                            tv.sebutan()
                        ),
                        "E0361",
                    );
                }
                Ty::Kosong
            }
            "panjang" => {
                if args.len() != 1 {
                    self.galat(e.span, "`panjang` memerlukan 1 argumen".into(), "E0362");
                }
                let t = match args.first() {
                    Some(a) => {
                        self.ctx.jangan_pindah += 1;
                        let t = self.eks(a, None);
                        self.ctx.jangan_pindah -= 1;
                        t
                    }
                    None => Ty::Kosong,
                };
                let t = match t {
                    Ty::Ref { dalam, .. } => *dalam,
                    lain => lain,
                };
                if !matches!(
                    t,
                    Ty::Teks | Ty::Irisan(_) | Ty::Array(_, _) | Ty::Daftar(_)
                ) {
                    self.galat(
                        e.span,
                        format!("`panjang` tidak berlaku untuk `{}`", t.sebutan()),
                        "E0363",
                    );
                }
                Ty::Bil
            }
            "ambil" => {
                if args.len() != 2 {
                    self.galat(
                        e.span,
                        "`ambil` memerlukan 2 argumen: wadah dan indeks".into(),
                        "E0364",
                    );
                    for a in args {
                        self.eks(a, None);
                    }
                    return Ty::Kosong;
                }
                self.ctx.jangan_pindah += 1;
                let t = self.eks(&args[0], None);
                self.ctx.jangan_pindah -= 1;
                let t = match t {
                    Ty::Ref { dalam, .. } => *dalam,
                    lain => lain,
                };
                let ti = self.eks(&args[1], Some(&Ty::Bil));
                if !self.bulat(&ti) {
                    self.galat(
                        args[1].span,
                        format!("indeks harus bilangan bulat, bukan `{}`", ti.sebutan()),
                        "E0365",
                    );
                }
                match &t {
                    Ty::Array(e2, _) | Ty::Irisan(e2) | Ty::Daftar(e2) => (**e2).clone(),
                    Ty::Teks => Ty::Huruf,
                    _ => {
                        self.galat(
                            args[0].span,
                            format!("`ambil` tidak berlaku untuk `{}`", t.sebutan()),
                            "E0366",
                        );
                        Ty::Bil
                    }
                }
            }
            "gagal" => {
                if args.len() != 1 {
                    self.galat(e.span, "`gagal` memerlukan 1 argumen teks".into(), "E0368");
                }
                for a in args {
                    let t = self.eks(a, Some(&Ty::Teks));
                    if t != Ty::Teks {
                        self.galat(
                            a.span,
                            format!("`gagal` memerlukan `teks`, bukan `{}`", t.sebutan()),
                            "E0369",
                        );
                    }
                }
                Ty::Kosong
            }
            lain => {
                self.galat(
                    e.span,
                    format!("fungsi bawaan `{}` tidak dikenal", lain),
                    "E0367",
                );
                for a in args {
                    self.eks(a, None);
                }
                Ty::Kosong
            }
        }
    }
}

fn nilai_muat(t: &Ty, v: i64) -> bool {
    match t {
        Ty::Bil => true,
        Ty::Bil32 => v >= i32::MIN as i64 && v <= i32::MAX as i64,
        Ty::Bil8 => v >= i8::MIN as i64 && v <= i8::MAX as i64,
        Ty::Nat => v >= 0,
        Ty::Nat8 => v >= 0 && v <= u8::MAX as i64,
        Ty::Huruf => v >= 0 && v <= u8::MAX as i64,
        _ => false,
    }
}
