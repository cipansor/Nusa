//! Pemeriksa tipe dan analisis keselamatan bahasa Nusa.

use crate::ast::*;
use crate::diag::{Diag, Hasil, Span};
use std::collections::HashMap;

#[derive(Clone, Debug)]
pub struct TandaTangan {
    pub params: Vec<Tipe>,
    pub hasil: Tipe,
}

#[derive(Clone, Debug)]
pub struct Info {
    pub tipe_ekspresi: HashMap<ExprId, Tipe>,
    pub tanda_tangan: HashMap<String, TandaTangan>,
}

impl Info {
    pub fn tipe(&self, e: &Expr) -> &Tipe {
        self.tipe_ekspresi
            .get(&e.id)
            .unwrap_or_else(|| panic!("tipe ekspresi {} belum dihitung", e.id))
    }
}

#[derive(Clone, Debug)]
struct Ikatan {
    tipe: Tipe,
    bisa_ubah: bool,
    span: Span,
}

pub fn periksa(program: &Program) -> Hasil<Info> {
    let mut tc = Typeck {
        lingkup: vec![HashMap::new()],
        tipe_ekspresi: HashMap::new(),
        tanda_tangan: HashMap::new(),
        dalam_kontrak: false,
        kedalaman_loop: 0,
        fungsi: "".into(),
    };

    for f in &program.fungsi {
        if BUILTIN.contains(&f.nama.as_str()) {
            return Err(Diag::baru(
                "NUSA-S001",
                format!("nama `{}` dipakai oleh fungsi bawaan", f.nama),
            )
            .di(f.span)
            .catat("pilih nama lain"));
        }
        if tc.tanda_tangan.contains_key(&f.nama) {
            return Err(Diag::baru(
                "NUSA-S002",
                format!("fungsi `{}` dideklarasikan lebih dari sekali", f.nama),
            )
            .di(f.span));
        }
        let mut terlihat = HashMap::new();
        if f.nama == "main" || f.nama.starts_with("nusa_") {
            return Err(Diag::baru(
                "NUSA-S031",
                format!("nama fungsi `{}` dicadangkan oleh sistem", f.nama),
            )
            .di(f.span));
        }
        for p in &f.params {
            if p.nama == "hasil" {
                return Err(Diag::baru(
                    "NUSA-S032",
                    "`hasil` adalah nama khusus untuk nilai kembalian pada `jamin`",
                )
                .di(p.span));
            }
            if terlihat.contains_key(&p.nama) {
                return Err(Diag::baru(
                    "NUSA-S003",
                    format!("parameter `{}` ganda pada fungsi `{}`", p.nama, f.nama),
                )
                .di(p.span));
            }
            terlihat.insert(p.nama.clone(), ());
        }
        tc.tanda_tangan.insert(
            f.nama.clone(),
            TandaTangan {
                params: f.params.iter().map(|p| p.tipe.clone()).collect(),
                hasil: f.hasil.clone(),
            },
        );
    }

    let utama = program
        .fungsi
        .iter()
        .find(|f| f.nama == "utama")
        .ok_or_else(|| {
            Diag::baru("NUSA-S004", "program harus memiliki fungsi `utama`")
                .catat("contoh: fungsi utama() -> kosong { ... }")
        })?;
    if !utama.params.is_empty() {
        return Err(
            Diag::baru("NUSA-S005", "fungsi `utama` tidak boleh memiliki parameter").di(utama.span),
        );
    }
    if utama.hasil != Tipe::Kosong {
        return Err(
            Diag::baru("NUSA-S006", "fungsi `utama` harus bertipe hasil `kosong`").di(utama.span),
        );
    }

    for f in &program.fungsi {
        tc.fungsi = f.nama.clone();
        tc.lingkup = vec![HashMap::new()];
        for p in &f.params {
            tc.lingkup[0].insert(
                p.nama.clone(),
                Ikatan {
                    tipe: p.tipe.clone(),
                    bisa_ubah: true,
                    span: p.span,
                },
            );
        }
        // Kontrak: hanya parameter dan (untuk jamin) `hasil`.
        tc.dalam_kontrak = true;
        for s in &f.syarat {
            tc.lingkup[0].remove("hasil");
            tc.harap(s, &Tipe::Log, "syarat")?;
        }
        if f.hasil == Tipe::Kosong {
            for j in &f.jamin {
                if menyebut(j, "hasil") {
                    return Err(Diag::baru(
                        "NUSA-S007",
                        "`hasil` tidak tersedia pada fungsi bertipe `kosong`",
                    )
                    .di(j.span));
                }
            }
        } else {
            tc.lingkup[0].insert(
                "hasil".into(),
                Ikatan {
                    tipe: f.hasil.clone(),
                    bisa_ubah: false,
                    span: f.span,
                },
            );
        }
        for j in &f.jamin {
            tc.harap(j, &Tipe::Log, "jamin")?;
        }
        tc.lingkup[0].remove("hasil");
        tc.dalam_kontrak = false;

        for p in &f.params {
            if p.tipe == Tipe::Kosong {
                return Err(Diag::baru(
                    "NUSA-S008",
                    format!("parameter `{}` tidak boleh bertipe `kosong`", p.nama),
                )
                .di(p.span));
            }
        }

        tc.blok(&f.tubuh)?;

        if f.hasil != Tipe::Kosong && !selalu_kembali(&f.tubuh) {
            return Err(Diag::baru(
                "NUSA-S009",
                format!(
                    "fungsi `{}` bertipe hasil `{}` tetapi tidak selalu mengembalikan nilai",
                    f.nama,
                    f.hasil.nama()
                ),
            )
            .di(f.span)
            .catat("pastikan jalur terakhir memakai `kembalikan` pada semua cabang"));
        }
    }

    Ok(Info {
        tipe_ekspresi: tc.tipe_ekspresi,
        tanda_tangan: tc.tanda_tangan,
    })
}

const BUILTIN: &[&str] = &["cetak", "cetakln", "panjang", "mutlak"];

fn menyebut(e: &Expr, nama: &str) -> bool {
    match &e.kind {
        ExprKind::Nama(n) => n == nama,
        ExprKind::Daftar(xs) | ExprKind::Panggil(_, xs) => xs.iter().any(|x| menyebut(x, nama)),
        ExprKind::Metode(res, _, xs) => menyebut(res, nama) || xs.iter().any(|x| menyebut(x, nama)),
        ExprKind::Unary(_, a) => menyebut(a, nama),
        ExprKind::Biner(_, a, b) | ExprKind::Indeks(a, b) => menyebut(a, nama) || menyebut(b, nama),
        _ => false,
    }
}

fn selalu_kembali(stmts: &[Stmt]) -> bool {
    match stmts.last() {
        Some(Stmt::Kembalikan { .. }) => true,
        Some(Stmt::Jika {
            maka,
            lainnya: Some(lain),
            ..
        }) => selalu_kembali(maka) && selalu_kembali(lain),
        Some(Stmt::Selama { kondisi, tubuh, .. }) => {
            matches!(kondisi.kind, ExprKind::Log(true)) && !berisi_henti(tubuh)
        }
        _ => false,
    }
}

fn berisi_henti(stmts: &[Stmt]) -> bool {
    stmts.iter().any(|s| match s {
        Stmt::Hentikan { .. } => true,
        Stmt::Jika { maka, lainnya, .. } => {
            berisi_henti(maka) || lainnya.as_deref().is_some_and(berisi_henti)
        }
        _ => false,
    })
}

struct Typeck {
    lingkup: Vec<HashMap<String, Ikatan>>,
    tipe_ekspresi: HashMap<ExprId, Tipe>,
    tanda_tangan: HashMap<String, TandaTangan>,
    dalam_kontrak: bool,
    kedalaman_loop: usize,
    fungsi: String,
}

impl Typeck {
    fn galat(&self, kode: &'static str, pesan: impl Into<String>, span: Span) -> Diag {
        Diag::baru(kode, pesan).di(span)
    }

    fn cari(&self, nama: &str) -> Option<&Ikatan> {
        self.lingkup.iter().rev().find_map(|l| l.get(nama))
    }

    fn deklarasikan(&mut self, nama: &str, tipe: Tipe, bisa_ubah: bool, span: Span) -> Hasil<()> {
        if let Some(lama) = self.cari(nama) {
            return Err(self
                .galat(
                    "NUSA-S011",
                    format!("variabel `{}` sudah dideklarasikan", nama),
                    span,
                )
                .catat(format!(
                    "deklarasi sebelumnya pada offset {}",
                    lama.span.mulai
                )));
        }
        self.lingkup.last_mut().unwrap().insert(
            nama.to_string(),
            Ikatan {
                tipe,
                bisa_ubah,
                span,
            },
        );
        Ok(())
    }

    fn blok(&mut self, stmts: &[Stmt]) -> Hasil<()> {
        self.lingkup.push(HashMap::new());
        for s in stmts {
            self.pernyataan(s)?;
        }
        self.lingkup.pop();
        Ok(())
    }

    fn pernyataan(&mut self, s: &Stmt) -> Hasil<()> {
        match s {
            Stmt::Tetap {
                nama,
                tipe,
                nilai,
                span,
            }
            | Stmt::Var {
                nama,
                tipe,
                nilai,
                span,
            } => {
                let bisa_ubah = matches!(s, Stmt::Var { .. });
                let tipe = match tipe {
                    Some(t) => {
                        if *t == Tipe::Kosong {
                            return Err(self.galat(
                                "NUSA-S012",
                                "variabel tidak boleh bertipe `kosong`",
                                *span,
                            ));
                        }
                        self.harap(nilai, t, "nilai awal")?;
                        t.clone()
                    }
                    None => self.cek(nilai, None)?,
                };
                if tipe == Tipe::Kosong {
                    return Err(self.galat(
                        "NUSA-S013",
                        "variabel tidak boleh bertipe `kosong`",
                        *span,
                    ));
                }
                self.deklarasikan(nama, tipe, bisa_ubah, *span)
            }
            Stmt::Tugas {
                target,
                op,
                nilai,
                span,
            } => self.tugas(target, *op, nilai, *span),
            Stmt::Ekspresi(e) => {
                match &e.kind {
                    ExprKind::Panggil(..) | ExprKind::Metode(..) => {
                        self.cek(e, Some(Tipe::Kosong))?;
                    }
                    _ => {
                        return Err(self.galat(
                            "NUSA-S014",
                            "ekspresi ini tidak menghasilkan efek; tulis penugasan atau panggilan",
                            e.span,
                        ))
                    }
                }
                Ok(())
            }
            Stmt::Jika {
                kondisi,
                maka,
                lainnya,
                ..
            } => {
                self.harap(kondisi, &Tipe::Log, "kondisi `jika`")?;
                self.blok(maka)?;
                if let Some(l) = lainnya {
                    self.blok(l)?;
                }
                Ok(())
            }
            Stmt::Selama {
                kondisi,
                invarian,
                tubuh,
                ..
            } => {
                self.harap(kondisi, &Tipe::Log, "kondisi `selama`")?;
                for inv in invarian {
                    self.harap(inv, &Tipe::Log, "invarian")?;
                }
                self.kedalaman_loop += 1;
                self.blok(tubuh)?;
                self.kedalaman_loop -= 1;
                Ok(())
            }
            Stmt::Untuk {
                nama,
                nama_span,
                iterasi,
                invarian,
                tubuh,
                ..
            } => {
                self.lingkup.push(HashMap::new());
                let tipe_var = match iterasi {
                    Iterasi::Rentang { mulai, akhir, .. } => {
                        self.harap(mulai, &Tipe::Bil, "batas awal rentang")?;
                        self.harap(akhir, &Tipe::Bil, "batas akhir rentang")?;
                        Tipe::Bil
                    }
                    Iterasi::Setiap(e) => {
                        let t = self.cek(e, None)?;
                        match t {
                            Tipe::Daftar(isi) => *isi,
                            lain => {
                                return Err(self.galat(
                                    "NUSA-S015",
                                    format!(
                                    "`untuk ... dalam` hanya mendukung `daftar`, ditemukan `{}`",
                                    lain.nama()
                                ),
                                    e.span,
                                ))
                            }
                        }
                    }
                };
                self.deklarasikan(nama, tipe_var, false, *nama_span)?;
                for inv in invarian {
                    self.harap(inv, &Tipe::Log, "invarian")?;
                }
                self.kedalaman_loop += 1;
                for s in tubuh {
                    self.pernyataan(s)?;
                }
                self.kedalaman_loop -= 1;
                self.lingkup.pop();
                Ok(())
            }
            Stmt::Kembalikan { nilai, span } => {
                let hasil = self.tanda_tangan[&self.fungsi].hasil.clone();
                match (nilai, &hasil) {
                    (None, Tipe::Kosong) => Ok(()),
                    (Some(e), Tipe::Kosong) => Err(self.galat(
                        "NUSA-S016",
                        format!("fungsi `{}` tidak mengembalikan nilai", self.fungsi),
                        e.span,
                    )),
                    (None, _) => Err(self.galat(
                        "NUSA-S017",
                        format!(
                            "fungsi `{}` harus mengembalikan `{}`",
                            self.fungsi,
                            hasil.nama()
                        ),
                        *span,
                    )),
                    (Some(e), t) => {
                        self.harap(e, t, "nilai kembalian")?;
                        Ok(())
                    }
                }
            }
            Stmt::Tegaskan { kondisi, span } => {
                self.harap(kondisi, &Tipe::Log, "tegaskan").map_err(|d| {
                    d.catat("`tegaskan` memeriksa kondisi saat program berjalan")
                        .di(*span)
                })
            }
            Stmt::Hentikan { span } | Stmt::Lanjutkan { span } => {
                if self.kedalaman_loop == 0 {
                    return Err(self.galat(
                        "NUSA-S018",
                        "`hentikan`/`lanjutkan` hanya boleh di dalam loop",
                        *span,
                    ));
                }
                Ok(())
            }
        }
    }

    fn tugas(&mut self, target: &Expr, op: TugasOp, nilai: &Expr, span: Span) -> Hasil<()> {
        match &target.kind {
            ExprKind::Nama(nama) => {
                let ikatan = self
                    .cari(nama)
                    .ok_or_else(|| {
                        self.galat(
                            "NUSA-S019",
                            format!("variabel `{}` tidak ditemukan", nama),
                            target.span,
                        )
                    })?
                    .clone();
                if !ikatan.bisa_ubah {
                    return Err(self
                        .galat(
                            "NUSA-S020",
                            format!("variabel `{}` bersifat tetap dan tidak dapat diubah", nama),
                            target.span,
                        )
                        .catat("gunakan `var` alih-alih `tetap` bila perlu diubah"));
                }
                self.tipe_ekspresi.insert(target.id, ikatan.tipe.clone());
                match op {
                    TugasOp::Isi => self.harap(nilai, &ikatan.tipe, "penugasan")?,
                    _ => {
                        let t = self.cek(nilai, Some(ikatan.tipe.clone()))?;
                        self.pastikan_operasi_tugas(op, &ikatan.tipe, &t, span)?;
                    }
                }
                Ok(())
            }
            ExprKind::Indeks(base, indeks) => {
                let (nama, ikatan) = match &base.kind {
                    ExprKind::Nama(n) => {
                        let ikatan = self
                            .cari(n)
                            .ok_or_else(|| {
                                self.galat(
                                    "NUSA-S019",
                                    format!("variabel `{}` tidak ditemukan", n),
                                    base.span,
                                )
                            })?
                            .clone();
                        (n.clone(), ikatan)
                    }
                    _ => {
                        return Err(self.galat(
                            "NUSA-S021",
                            "penugasan elemen hanya didukung untuk variabel daftar langsung",
                            base.span,
                        ))
                    }
                };
                if !ikatan.bisa_ubah {
                    return Err(self.galat(
                        "NUSA-S020",
                        format!("variabel `{}` bersifat tetap dan tidak dapat diubah", nama),
                        base.span,
                    ));
                }
                let elemen = match &ikatan.tipe {
                    Tipe::Daftar(t) => (**t).clone(),
                    lain => {
                        return Err(self.galat(
                            "NUSA-S022",
                            format!("`{}` bertipe `{}`, bukan daftar", nama, lain.nama()),
                            base.span,
                        ))
                    }
                };
                self.tipe_ekspresi.insert(base.id, ikatan.tipe.clone());
                self.harap(indeks, &Tipe::Bil, "indeks daftar")?;
                self.tipe_ekspresi.insert(target.id, elemen.clone());
                match op {
                    TugasOp::Isi => self.harap(nilai, &elemen, "penugasan elemen")?,
                    _ => {
                        let t = self.cek(nilai, Some(elemen.clone()))?;
                        self.pastikan_operasi_tugas(op, &elemen, &t, span)?;
                    }
                }
                Ok(())
            }
            _ => Err(self.galat(
                "NUSA-P008",
                "sisi kiri penugasan harus variabel atau elemen daftar",
                target.span,
            )),
        }
    }

    fn pastikan_operasi_tugas(
        &mut self,
        op: TugasOp,
        tujuan: &Tipe,
        nilai: &Tipe,
        span: Span,
    ) -> Hasil<()> {
        let sah = match op {
            TugasOp::Isi => tujuan == nilai,
            TugasOp::Tambah => matches!(
                (tujuan, nilai),
                (Tipe::Bil, Tipe::Bil) | (Tipe::Des, Tipe::Des) | (Tipe::Teks, Tipe::Teks)
            ),
            TugasOp::Kurang | TugasOp::Kali => matches!(
                (tujuan, nilai),
                (Tipe::Bil, Tipe::Bil) | (Tipe::Des, Tipe::Des)
            ),
            TugasOp::Bagi => matches!(
                (tujuan, nilai),
                (Tipe::Bil, Tipe::Bil) | (Tipe::Des, Tipe::Des)
            ),
            TugasOp::Mod => matches!((tujuan, nilai), (Tipe::Bil, Tipe::Bil)),
        };
        if sah {
            Ok(())
        } else {
            Err(self.galat(
                "NUSA-S023",
                format!(
                    "operasi `{}=` tidak sah antara `{}` dan `{}`",
                    simbol_tugas(op),
                    tujuan.nama(),
                    nilai.nama()
                ),
                span,
            ))
        }
    }

    /// Memeriksa ekspresi terhadap tipe yang diharapkan dan menyimpan hasilnya.
    fn harap(&mut self, e: &Expr, harapan: &Tipe, konteks: &str) -> Hasil<()> {
        let t = self.cek(e, Some(harapan.clone()))?;
        if &t != harapan {
            return Err(self.galat(
                "NUSA-T001",
                format!(
                    "{}: tipe tidak cocok; diharapkan `{}`, ditemukan `{}`",
                    konteks,
                    harapan.nama(),
                    t.nama()
                ),
                e.span,
            ));
        }
        Ok(())
    }

    fn cek(&mut self, e: &Expr, harapan: Option<Tipe>) -> Hasil<Tipe> {
        let t = match &e.kind {
            ExprKind::Bil(_) => match &harapan {
                Some(Tipe::Des) => Tipe::Des,
                _ => Tipe::Bil,
            },
            ExprKind::Des(_) => Tipe::Des,
            ExprKind::Teks(_) => Tipe::Teks,
            ExprKind::Log(_) => Tipe::Log,
            ExprKind::Nama(nama) => {
                let ikatan = self.cari(nama).ok_or_else(|| {
                    self.galat(
                        "NUSA-S024",
                        format!("nama `{}` tidak ditemukan", nama),
                        e.span,
                    )
                })?;
                if self.dalam_kontrak && !matches!(nama.as_str(), "hasil") {
                    // masih boleh: parameter; dicek lewat ada-tidaknya ikatan
                }
                ikatan.tipe.clone()
            }
            ExprKind::Daftar(isi) => {
                let elemen = match harapan {
                    Some(Tipe::Daftar(t)) => Some(*t),
                    Some(lain) => {
                        return Err(self.galat(
                            "NUSA-T003",
                            format!("daftar tidak dapat dipakai sebagai `{}`", lain.nama()),
                            e.span,
                        ))
                    }
                    None => None,
                };
                if isi.is_empty() {
                    match elemen {
                        Some(t) => Tipe::Daftar(Box::new(t)),
                        None => {
                            return Err(self
                                .galat(
                                    "NUSA-T004",
                                    "daftar kosong membutuhkan tipe eksplisit",
                                    e.span,
                                )
                                .catat("contoh: `var d: daftar<bil> = []`"))
                        }
                    }
                } else if let Some(t) = elemen {
                    for x in isi {
                        self.harap(x, &t, "elemen daftar")?;
                    }
                    Tipe::Daftar(Box::new(t))
                } else {
                    let pertama = self.cek(&isi[0], None)?;
                    if pertama == Tipe::Kosong {
                        return Err(self.galat(
                            "NUSA-T005",
                            "daftar tidak boleh berisi `kosong`",
                            isi[0].span,
                        ));
                    }
                    self.tipe_ekspresi.insert(isi[0].id, pertama.clone());
                    for x in &isi[1..] {
                        self.harap(x, &pertama, "elemen daftar")?;
                    }
                    Tipe::Daftar(Box::new(pertama))
                }
            }
            ExprKind::Unary(op, a) => match op {
                UnOp::Neg => {
                    let t = self.cek(a, harapan.clone())?;
                    match t {
                        Tipe::Bil | Tipe::Des => t,
                        lain => {
                            return Err(self.galat(
                                "NUSA-T006",
                                format!("`-` hanya untuk `bil`/`des`, bukan `{}`", lain.nama()),
                                e.span,
                            ))
                        }
                    }
                }
                UnOp::Tidak => {
                    self.harap(a, &Tipe::Log, "operan `bukan`")?;
                    Tipe::Log
                }
            },
            ExprKind::Biner(op, a, b) => self.biner(*op, a, b, e.span)?,
            ExprKind::Panggil(nama, args) => self.panggil(nama, args, e.span, harapan.as_ref())?,
            ExprKind::Metode(res, nama, args) => self.metode(res, nama, args, e.span)?,
            ExprKind::Indeks(base, indeks) => {
                let tb = self.cek(base, None)?;
                let elemen = match tb {
                    Tipe::Daftar(t) => *t,
                    lain => {
                        return Err(self.galat(
                            "NUSA-T007",
                            format!("`[]` hanya untuk `daftar`, bukan `{}`", lain.nama()),
                            base.span,
                        ))
                    }
                };
                self.harap(indeks, &Tipe::Bil, "indeks daftar")?;
                elemen
            }
        };

        self.tipe_ekspresi.insert(e.id, t.clone());
        Ok(t)
    }

    fn biner(&mut self, op: BinOp, a: &Expr, b: &Expr, span: Span) -> Hasil<Tipe> {
        match op {
            BinOp::Dan | BinOp::Atau => {
                self.harap(a, &Tipe::Log, "operan logika")?;
                self.harap(b, &Tipe::Log, "operan logika")?;
                Ok(Tipe::Log)
            }
            BinOp::Tambah | BinOp::Kurang | BinOp::Kali | BinOp::Bagi | BinOp::Mod => {
                let ta = self.cek(a, None)?;
                let tb = self.cek(b, Some(ta.clone()))?;
                let (ta, tb) = if ta == Tipe::Des && b.kind_is_int_literal() {
                    self.tipe_ekspresi.insert(b.id, Tipe::Des);
                    (ta, Tipe::Des)
                } else if tb == Tipe::Des && a.kind_is_int_literal() {
                    self.tipe_ekspresi.insert(a.id, Tipe::Des);
                    (Tipe::Des, tb)
                } else {
                    (ta, tb)
                };
                if ta != tb {
                    return Err(self
                        .galat(
                            "NUSA-T008",
                            format!(
                                "tipe tidak cocok pada `{}`: `{}` dan `{}`",
                                op.simbol(),
                                ta.nama(),
                                tb.nama()
                            ),
                            span,
                        )
                        .catat(
                            "Nusa tidak melakukan konversi implisit; gunakan angka yang setipe",
                        ));
                }
                match op {
                    BinOp::Tambah if ta == Tipe::Teks => Ok(Tipe::Teks),
                    BinOp::Tambah | BinOp::Kurang | BinOp::Kali | BinOp::Bagi
                        if matches!(ta, Tipe::Bil | Tipe::Des) =>
                    {
                        Ok(ta)
                    }
                    BinOp::Mod if ta == Tipe::Bil => Ok(Tipe::Bil),
                    BinOp::Mod => Err(self.galat(
                        "NUSA-T009",
                        "`%` hanya untuk `bil`; untuk desimal gunakan fungsi lain",
                        span,
                    )),
                    _ => Err(self.galat(
                        "NUSA-T010",
                        format!(
                            "operator `{}` tidak berlaku untuk `{}`",
                            op.simbol(),
                            ta.nama()
                        ),
                        span,
                    )),
                }
            }
            _ => {
                // perbandingan
                let ta = self.cek(a, None)?;
                let tb = self.cek(b, Some(ta.clone()))?;
                let (ta, tb) = if ta == Tipe::Des && b.kind_is_int_literal() {
                    self.tipe_ekspresi.insert(b.id, Tipe::Des);
                    (ta, Tipe::Des)
                } else if tb == Tipe::Des && a.kind_is_int_literal() {
                    self.tipe_ekspresi.insert(a.id, Tipe::Des);
                    (Tipe::Des, tb)
                } else {
                    (ta, tb)
                };
                if ta != tb {
                    return Err(self.galat(
                        "NUSA-T011",
                        format!(
                            "perbandingan `{}` membutuhkan tipe sama, ditemukan `{}` dan `{}`",
                            op.simbol(),
                            ta.nama(),
                            tb.nama()
                        ),
                        span,
                    ));
                }
                let sah = match op {
                    BinOp::SamaSama | BinOp::TidakSama => {
                        matches!(ta, Tipe::Bil | Tipe::Des | Tipe::Log | Tipe::Teks)
                    }
                    _ => matches!(ta, Tipe::Bil | Tipe::Des | Tipe::Teks),
                };
                if !sah {
                    return Err(self.galat(
                        "NUSA-T012",
                        format!("perbandingan tidak berlaku untuk `{}`", ta.nama()),
                        span,
                    ));
                }
                Ok(Tipe::Log)
            }
        }
    }

    fn panggil(
        &mut self,
        nama: &str,
        args: &[Expr],
        span: Span,
        harapan: Option<&Tipe>,
    ) -> Hasil<Tipe> {
        if self.dalam_kontrak {
            return Err(self
                .galat("NUSA-S025", "kontrak tidak boleh memanggil fungsi", span)
                .catat("kontrak harus berupa ekspresi murni atas parameter dan `hasil`"));
        }
        match nama {
            "cetak" | "cetakln" => {
                for a in args {
                    let t = self.cek(a, None)?;
                    if !bisa_dicetak(&t) {
                        return Err(self.galat(
                            "NUSA-T013",
                            format!("`{}` tidak dapat dicetak", t.nama()),
                            a.span,
                        ));
                    }
                }
                Ok(Tipe::Kosong)
            }
            "panjang" => {
                self.arity(nama, args, 1, span)?;
                let t = self.cek(&args[0], None)?;
                match t {
                    Tipe::Teks | Tipe::Daftar(_) => Ok(Tipe::Bil),
                    lain => Err(self.galat(
                        "NUSA-T014",
                        format!(
                            "`panjang` hanya untuk `teks`/`daftar`, bukan `{}`",
                            lain.nama()
                        ),
                        args[0].span,
                    )),
                }
            }
            "mutlak" => {
                self.arity(nama, args, 1, span)?;
                let t = self.cek(&args[0], None)?;
                match t {
                    Tipe::Bil | Tipe::Des => Ok(t),
                    lain => Err(self.galat(
                        "NUSA-T015",
                        format!("`mutlak` hanya untuk `bil`/`des`, bukan `{}`", lain.nama()),
                        args[0].span,
                    )),
                }
            }
            _ => {
                let sig = self.tanda_tangan.get(nama).cloned().ok_or_else(|| {
                    self.galat(
                        "NUSA-S026",
                        format!("fungsi `{}` tidak ditemukan", nama),
                        span,
                    )
                    .catat("periksa ejaan atau deklarasikan fungsinya")
                })?;
                if args.len() != sig.params.len() {
                    return Err(self.galat(
                        "NUSA-S027",
                        format!(
                            "fungsi `{}` membutuhkan {} argumen, diberikan {}",
                            nama,
                            sig.params.len(),
                            args.len()
                        ),
                        span,
                    ));
                }
                for (a, p) in args.iter().zip(&sig.params) {
                    self.harap(a, p, &format!("argumen `{}`", nama))?;
                }
                if sig.hasil == Tipe::Kosong {
                    if let Some(h) = harapan {
                        if *h != Tipe::Kosong {
                            return Err(self.galat(
                                "NUSA-T016",
                                format!("fungsi `{}` tidak mengembalikan nilai", nama),
                                span,
                            ));
                        }
                    }
                }
                Ok(sig.hasil)
            }
        }
    }

    fn metode(&mut self, res: &Expr, nama: &str, args: &[Expr], span: Span) -> Hasil<Tipe> {
        if self.dalam_kontrak {
            return Err(self.galat("NUSA-S025", "kontrak tidak boleh memanggil metode", span));
        }
        let tres = self.cek(res, None)?;
        match (nama, &tres) {
            ("tambah", Tipe::Daftar(elemen)) => {
                if args.len() != 1 {
                    return Err(self.galat(
                        "NUSA-S028",
                        format!("`tambah` membutuhkan 1 argumen, diberikan {}", args.len()),
                        span,
                    ));
                }
                self.pastikan_tempat_ubah(res, "tambah")?;
                self.harap(&args[0], elemen, "nilai yang ditambahkan")?;
                Ok(Tipe::Kosong)
            }
            ("panjang", Tipe::Daftar(_) | Tipe::Teks) => {
                if !args.is_empty() {
                    return Err(self.galat("NUSA-S028", "`panjang` tidak menerima argumen", span));
                }
                Ok(Tipe::Bil)
            }
            ("tambah", lain) => Err(self.galat(
                "NUSA-T017",
                format!("`tambah` hanya untuk `daftar`, bukan `{}`", lain.nama()),
                span,
            )),
            ("panjang", lain) => Err(self.galat(
                "NUSA-T018",
                format!(
                    "`panjang` hanya untuk `daftar`/`teks`, bukan `{}`",
                    lain.nama()
                ),
                span,
            )),
            _ => Err(self.galat(
                "NUSA-S029",
                format!("metode `{}` tidak dikenal untuk `{}`", nama, tres.nama()),
                span,
            )),
        }
    }

    fn pastikan_tempat_ubah(&mut self, e: &Expr, aksi: &str) -> Hasil<()> {
        let nama = match &e.kind {
            ExprKind::Nama(n) => n,
            _ => {
                return Err(self
                    .galat(
                        "NUSA-S030",
                        format!("`{}` belum mendukung mutasi elemen daftar bersarang", aksi),
                        e.span,
                    )
                    .catat("salin elemen ke variabel dahulu, ubah variabel itu, lalu tugaskan kembali ke daftar"))
            }
        };
        let ikatan = self.cari(nama).ok_or_else(|| {
            self.galat(
                "NUSA-S019",
                format!("variabel `{}` tidak ditemukan", nama),
                e.span,
            )
        })?;
        if !ikatan.bisa_ubah {
            return Err(self.galat(
                "NUSA-S020",
                format!("variabel `{}` bersifat tetap dan tidak dapat diubah", nama),
                e.span,
            ));
        }
        Ok(())
    }

    fn arity(&self, nama: &str, args: &[Expr], n: usize, span: Span) -> Hasil<()> {
        if args.len() != n {
            Err(self.galat(
                "NUSA-S028",
                format!(
                    "`{}` membutuhkan {} argumen, diberikan {}",
                    nama,
                    n,
                    args.len()
                ),
                span,
            ))
        } else {
            Ok(())
        }
    }
}

fn simbol_tugas(op: TugasOp) -> &'static str {
    match op {
        TugasOp::Isi => "",
        TugasOp::Tambah => "+",
        TugasOp::Kurang => "-",
        TugasOp::Kali => "*",
        TugasOp::Bagi => "/",
        TugasOp::Mod => "%",
    }
}

impl Expr {
    fn kind_is_int_literal(&self) -> bool {
        matches!(self.kind, ExprKind::Bil(_))
            || matches!(&self.kind, ExprKind::Unary(UnOp::Neg, a) if a.kind_is_int_literal())
    }
}

fn bisa_dicetak(t: &Tipe) -> bool {
    match t {
        Tipe::Bil | Tipe::Des | Tipe::Log | Tipe::Teks => true,
        Tipe::Daftar(e) => bisa_dicetak(e),
        Tipe::Kosong => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::lex;
    use crate::parser::parse;

    fn periksa_str(s: &str) -> Hasil<Info> {
        let program = parse(lex(s).unwrap()).unwrap();
        periksa(&program)
    }

    fn galat(s: &str) -> Diag {
        match periksa_str(s) {
            Ok(_) => panic!("program seharusnya ditolak"),
            Err(d) => d,
        }
    }

    #[test]
    fn tetap_tidak_boleh_diubah() {
        let d = galat("fungsi utama() -> kosong {\n tetap x: bil = 1\n x = 2\n}\n");
        assert_eq!(d.kode, "NUSA-S020");
    }

    #[test]
    fn tipe_tidak_cocok() {
        let d = galat("fungsi utama() -> kosong {\n var s: teks = 3\n}\n");
        assert_eq!(d.kode, "NUSA-T001");
    }

    #[test]
    fn fungsi_harus_mengembalikan() {
        let d = galat("fungsi f() -> bil {\n var x: bil = 1\n}\nfungsi utama() -> kosong { }\n");
        assert_eq!(d.kode, "NUSA-S009");
    }

    #[test]
    fn daftar_kosong_butuh_tipe() {
        let d = galat("fungsi utama() -> kosong {\n var d = []\n}\n");
        assert_eq!(d.kode, "NUSA-T004");
    }

    #[test]
    fn kontrak_tidak_boleh_memanggil() {
        let d = galat(
            "fungsi f(x: bil) -> bil\n syarat g(x) > 0\n{\n kembalikan x\n}\nfungsi g(x: bil) -> bil { kembalikan x }\nfungsi utama() -> kosong { }\n",
        );
        assert_eq!(d.kode, "NUSA-S025");
    }

    #[test]
    fn parameter_boleh_diubah_dan_hasil_lokal_boleh() {
        assert!(periksa_str(
            "fungsi f(d: daftar<bil>) -> kosong {\n d.tambah(1)\n var hasil: bil = 2\n}\nfungsi utama() -> kosong { }\n"
        )
        .is_ok());
    }

    #[test]
    fn panjang_hanya_untuk_teks_dan_daftar() {
        let d = galat("fungsi utama() -> kosong {\n cetakln(panjang(3))\n}\n");
        assert_eq!(d.kode, "NUSA-T014");
    }

    #[test]
    fn utama_wajib_ada() {
        let d = galat("fungsi bukan_utama() -> kosong { }\n");
        assert_eq!(d.kode, "NUSA-S004");
    }
}
