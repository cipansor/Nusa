//! Parser rekursif-turun untuk bahasa Nusa.

use crate::ast::*;
use crate::diag::{Diag, Hasil, Span};
use crate::lexer::{Tok, Token};

pub fn parse(tokens: Vec<Token>) -> Hasil<Program> {
    let mut p = Parser {
        tokens,
        pos: 0,
        id_berikut: 0,
    };
    let mut fungsi = Vec::new();
    p.lewati_baris();
    while p.intai() != &Tok::Akhir {
        if p.intai() == &Tok::Fungsi {
            fungsi.push(p.fungsi()?);
        } else {
            return Err(p.galat(
                "NUSA-P001",
                format!(
                    "hanya deklarasi `fungsi` yang boleh berada di tingkat atas, ditemukan {}",
                    p.intai().deskripsi()
                ),
            ));
        }
        p.lewati_baris();
    }
    if fungsi.is_empty() {
        return Err(Diag::baru(
            "NUSA-P002",
            "berkas tidak berisi satu pun `fungsi`",
        ));
    }
    Ok(Program { fungsi })
}

struct Parser {
    tokens: Vec<Token>,
    pos: usize,
    id_berikut: ExprId,
}

impl Parser {
    fn intai(&self) -> &Tok {
        &self.tokens[self.pos.min(self.tokens.len() - 1)].tok
    }

    fn span_intai(&self) -> Span {
        self.tokens[self.pos.min(self.tokens.len() - 1)].span
    }

    fn maju(&mut self) -> Token {
        let t = self.tokens[self.pos.min(self.tokens.len() - 1)].clone();
        if self.pos < self.tokens.len() - 1 {
            self.pos += 1;
        }
        t
    }

    fn cocok(&mut self, t: &Tok) -> bool {
        if self.intai() == t {
            self.maju();
            true
        } else {
            false
        }
    }

    fn harap(&mut self, t: &Tok, konteks: &str) -> Hasil<Token> {
        if self.intai() == t {
            Ok(self.maju())
        } else {
            Err(self.galat(
                "NUSA-P003",
                format!(
                    "{}: diharapkan {}, ditemukan {}",
                    konteks,
                    t.deskripsi(),
                    self.intai().deskripsi()
                ),
            ))
        }
    }

    fn lewati_baris(&mut self) {
        while self.intai() == &Tok::BaruBaris {
            self.maju();
        }
    }

    fn galat(&self, kode: &'static str, pesan: impl Into<String>) -> Diag {
        Diag::baru(kode, pesan).di(self.span_intai())
    }

    fn ekspr(&mut self, kind: ExprKind, span: Span) -> Expr {
        let id = self.id_berikut;
        self.id_berikut += 1;
        Expr { id, kind, span }
    }

    fn nama(&mut self, konteks: &str) -> Hasil<(String, Span)> {
        match self.intai().clone() {
            Tok::Id(n) => {
                let sp = self.span_intai();
                self.maju();
                Ok((n, sp))
            }
            lain => Err(self.galat(
                "NUSA-P004",
                format!(
                    "{}: diharapkan nama, ditemukan {}",
                    konteks,
                    lain.deskripsi()
                ),
            )),
        }
    }

    fn tipe(&mut self) -> Hasil<Tipe> {
        let sp = self.span_intai();
        let dasar = match self.intai() {
            Tok::TBil => Tipe::Bil,
            Tok::TDes => Tipe::Des,
            Tok::TLog => Tipe::Log,
            Tok::TTeks => Tipe::Teks,
            Tok::Kosong => Tipe::Kosong,
            Tok::TDaftar => {
                self.maju();
                self.harap(&Tok::Kurang, "tipe `daftar` membutuhkan elemen")?;
                let isi = self.tipe()?;
                self.harap(&Tok::Lebih, "tipe `daftar` harus ditutup dengan `>`")?;
                return Ok(Tipe::Daftar(Box::new(isi)));
            }
            lain => {
                return Err(Diag::baru(
                    "NUSA-P005",
                    format!(
                        "diharapkan tipe (bil, des, log, teks, daftar<...>, kosong), ditemukan {}",
                        lain.deskripsi()
                    ),
                )
                .di(sp))
            }
        };
        self.maju();
        Ok(dasar)
    }

    fn fungsi(&mut self) -> Hasil<Fungsi> {
        let awal = self.harap(&Tok::Fungsi, "deklarasi fungsi")?;
        let (nama, _) = self.nama("nama fungsi")?;
        self.harap(&Tok::LPar, "daftar parameter")?;
        let mut params = Vec::new();
        if self.intai() != &Tok::RPar {
            loop {
                let (pnama, pspan) = self.nama("nama parameter")?;
                self.harap(&Tok::TitikDua, "parameter harus bertipe")?;
                let tipe = self.tipe()?;
                params.push(Param {
                    nama: pnama,
                    tipe,
                    span: pspan,
                });
                if !self.cocok(&Tok::Koma) {
                    break;
                }
            }
        }
        self.harap(&Tok::RPar, "penutup daftar parameter")?;

        let hasil = if self.cocok(&Tok::Panah) {
            self.tipe()?
        } else {
            Tipe::Kosong
        };

        let mut syarat = Vec::new();
        let mut jamin = Vec::new();
        loop {
            let simpan = self.pos;
            self.lewati_baris();
            match self.intai() {
                Tok::Syarat => {
                    self.maju();
                    syarat.push(self.ekspresi()?);
                }
                Tok::Jamin => {
                    self.maju();
                    jamin.push(self.ekspresi()?);
                }
                _ => {
                    self.pos = simpan;
                    break;
                }
            }
        }

        self.lewati_baris();
        let tubuh = self.blok()?;
        let akhir = self.tokens[self.pos.saturating_sub(1)].span;
        Ok(Fungsi {
            nama,
            params,
            hasil,
            syarat,
            jamin,
            tubuh,
            span: Span::gabung(awal.span, akhir),
        })
    }

    fn blok(&mut self) -> Hasil<Vec<Stmt>> {
        self.harap(&Tok::LBrace, "awal blok `{`")?;
        let mut stmts = Vec::new();
        self.lewati_baris();
        loop {
            match self.intai() {
                Tok::RBrace => {
                    self.maju();
                    break;
                }
                Tok::Akhir => {
                    return Err(self.galat("NUSA-P006", "blok tidak ditutup dengan `}`"));
                }
                _ => {}
            }
            stmts.push(self.pernyataan()?);
            match self.intai() {
                Tok::BaruBaris => self.lewati_baris(),
                Tok::RBrace | Tok::Akhir => {}
                _ => {
                    return Err(self.galat(
                        "NUSA-P007",
                        format!(
                            "akhir pernyataan diharapkan (baris baru atau `}}`), ditemukan {}",
                            self.intai().deskripsi()
                        ),
                    ))
                }
            }
        }
        Ok(stmts)
    }

    fn pernyataan(&mut self) -> Hasil<Stmt> {
        let span = self.span_intai();
        match self.intai().clone() {
            Tok::Tetap | Tok::Var => {
                let tetap = self.intai() == &Tok::Tetap;
                self.maju();
                let (nama, nspan) = self.nama("nama variabel")?;
                let tipe = if self.cocok(&Tok::TitikDua) {
                    Some(self.tipe()?)
                } else {
                    None
                };
                self.harap(&Tok::Sama, "variabel harus diberi nilai awal dengan `=`")?;
                let nilai = self.ekspresi()?;
                let akhir = Span::gabung(span, nilai.span);
                if tetap {
                    Ok(Stmt::Tetap {
                        nama,
                        tipe,
                        nilai,
                        span: Span::gabung(nspan, akhir),
                    })
                } else {
                    Ok(Stmt::Var {
                        nama,
                        tipe,
                        nilai,
                        span: Span::gabung(nspan, akhir),
                    })
                }
            }
            Tok::Jika => self.jika(),
            Tok::Selama => {
                self.maju();
                let kondisi = self.ekspresi()?;
                let invarian = self.invarian()?;
                self.lewati_baris();
                let tubuh = self.blok()?;
                Ok(Stmt::Selama {
                    kondisi,
                    invarian,
                    tubuh,
                    span,
                })
            }
            Tok::Untuk => {
                self.maju();
                let (nama, nama_span) = self.nama("variabel loop")?;
                self.harap(&Tok::Dalam, "loop `untuk` membutuhkan `dalam`")?;
                let pertama = self.ekspresi()?;
                let iterasi = if self.cocok(&Tok::TitikTitik) {
                    let akhir = self.ekspresi()?;
                    Iterasi::Rentang {
                        mulai: pertama,
                        akhir,
                        inklusif: false,
                    }
                } else if self.cocok(&Tok::TitikTitikSama) {
                    let akhir = self.ekspresi()?;
                    Iterasi::Rentang {
                        mulai: pertama,
                        akhir,
                        inklusif: true,
                    }
                } else {
                    Iterasi::Setiap(pertama)
                };
                let invarian = self.invarian()?;
                self.lewati_baris();
                let tubuh = self.blok()?;
                Ok(Stmt::Untuk {
                    nama,
                    nama_span,
                    iterasi,
                    invarian,
                    tubuh,
                    span,
                })
            }
            Tok::Kembalikan => {
                self.maju();
                let nilai = match self.intai() {
                    Tok::BaruBaris | Tok::RBrace | Tok::Akhir => None,
                    _ => Some(self.ekspresi()?),
                };
                Ok(Stmt::Kembalikan {
                    span: Span::gabung(span, nilai.as_ref().map(|e| e.span).unwrap_or(span)),
                    nilai,
                })
            }
            Tok::Tegaskan => {
                self.maju();
                let kondisi = self.ekspresi()?;
                Ok(Stmt::Tegaskan { span, kondisi })
            }
            Tok::Hentikan => {
                self.maju();
                Ok(Stmt::Hentikan { span })
            }
            Tok::Lanjutkan => {
                self.maju();
                Ok(Stmt::Lanjutkan { span })
            }
            _ => {
                let e = self.ekspresi()?;
                let op = match self.intai() {
                    Tok::Sama => Some(TugasOp::Isi),
                    Tok::PlusSama => Some(TugasOp::Tambah),
                    Tok::MinusSama => Some(TugasOp::Kurang),
                    Tok::BintangSama => Some(TugasOp::Kali),
                    Tok::GarisMiringSama => Some(TugasOp::Bagi),
                    Tok::PersenSama => Some(TugasOp::Mod),
                    _ => None,
                };
                if let Some(op) = op {
                    self.maju();
                    if !matches!(e.kind, ExprKind::Nama(_) | ExprKind::Indeks(..)) {
                        return Err(Diag::baru(
                            "NUSA-P008",
                            "sisi kiri penugasan harus variabel atau elemen daftar",
                        )
                        .di(e.span)
                        .catat("contoh yang benar: `x = 10` atau `d[0] = 10`"));
                    }
                    let nilai = self.ekspresi()?;
                    let sp = Span::gabung(e.span, nilai.span);
                    Ok(Stmt::Tugas {
                        target: e,
                        op,
                        nilai,
                        span: sp,
                    })
                } else {
                    Ok(Stmt::Ekspresi(e))
                }
            }
        }
    }

    fn jika(&mut self) -> Hasil<Stmt> {
        let span = self.harap(&Tok::Jika, "`jika`")?.span;
        let kondisi = self.ekspresi()?;
        self.lewati_baris();
        let maka = self.blok()?;

        let simpan = self.pos;
        self.lewati_baris();
        let lainnya = if self.cocok(&Tok::Lainnya) {
            self.lewati_baris();
            if self.intai() == &Tok::Jika {
                Some(vec![self.jika()?])
            } else {
                Some(self.blok()?)
            }
        } else {
            self.pos = simpan;
            None
        };
        Ok(Stmt::Jika {
            kondisi,
            maka,
            lainnya,
            span,
        })
    }

    fn invarian(&mut self) -> Hasil<Vec<Expr>> {
        let mut hasil = Vec::new();
        loop {
            let simpan = self.pos;
            self.lewati_baris();
            if self.cocok(&Tok::Invarian) {
                hasil.push(self.ekspresi()?);
            } else {
                self.pos = simpan;
                break;
            }
        }
        Ok(hasil)
    }

    fn ekspresi(&mut self) -> Hasil<Expr> {
        self.atau()
    }

    fn atau(&mut self) -> Hasil<Expr> {
        let mut kiri = self.dan()?;
        while matches!(self.intai(), Tok::Atau | Tok::AtauAtau) {
            self.maju();
            let kanan = self.dan()?;
            let sp = Span::gabung(kiri.span, kanan.span);
            kiri = self.ekspr(
                ExprKind::Biner(BinOp::Atau, Box::new(kiri), Box::new(kanan)),
                sp,
            );
        }
        Ok(kiri)
    }

    fn dan(&mut self) -> Hasil<Expr> {
        let mut kiri = self.banding()?;
        while matches!(self.intai(), Tok::Dan | Tok::DanDan) {
            self.maju();
            let kanan = self.banding()?;
            let sp = Span::gabung(kiri.span, kanan.span);
            kiri = self.ekspr(
                ExprKind::Biner(BinOp::Dan, Box::new(kiri), Box::new(kanan)),
                sp,
            );
        }
        Ok(kiri)
    }

    fn banding(&mut self) -> Hasil<Expr> {
        let mut kiri = self.jumlah()?;
        loop {
            let op = match self.intai() {
                Tok::SamaSama => BinOp::SamaSama,
                Tok::TidakSama => BinOp::TidakSama,
                Tok::Kurang => BinOp::KurangDari,
                Tok::KurangSama => BinOp::KurangSama,
                Tok::Lebih => BinOp::LebihDari,
                Tok::LebihSama => BinOp::LebihSama,
                _ => break,
            };
            self.maju();
            let kanan = self.jumlah()?;
            let sp = Span::gabung(kiri.span, kanan.span);
            kiri = self.ekspr(ExprKind::Biner(op, Box::new(kiri), Box::new(kanan)), sp);
        }
        Ok(kiri)
    }

    fn jumlah(&mut self) -> Hasil<Expr> {
        let mut kiri = self.kali()?;
        loop {
            let op = match self.intai() {
                Tok::Plus => BinOp::Tambah,
                Tok::Minus => BinOp::Kurang,
                _ => break,
            };
            self.maju();
            let kanan = self.kali()?;
            let sp = Span::gabung(kiri.span, kanan.span);
            kiri = self.ekspr(ExprKind::Biner(op, Box::new(kiri), Box::new(kanan)), sp);
        }
        Ok(kiri)
    }

    fn kali(&mut self) -> Hasil<Expr> {
        let mut kiri = self.unary()?;
        loop {
            let op = match self.intai() {
                Tok::Bintang => BinOp::Kali,
                Tok::GarisMiring => BinOp::Bagi,
                Tok::Persen => BinOp::Mod,
                _ => break,
            };
            self.maju();
            let kanan = self.unary()?;
            let sp = Span::gabung(kiri.span, kanan.span);
            kiri = self.ekspr(ExprKind::Biner(op, Box::new(kiri), Box::new(kanan)), sp);
        }
        Ok(kiri)
    }

    fn unary(&mut self) -> Hasil<Expr> {
        let span = self.span_intai();
        match self.intai() {
            Tok::Minus => {
                self.maju();
                let e = self.unary()?;
                let sp = Span::gabung(span, e.span);
                Ok(self.ekspr(ExprKind::Unary(UnOp::Neg, Box::new(e)), sp))
            }
            Tok::Bukan | Tok::Seru => {
                self.maju();
                let e = self.unary()?;
                let sp = Span::gabung(span, e.span);
                Ok(self.ekspr(ExprKind::Unary(UnOp::Tidak, Box::new(e)), sp))
            }
            _ => self.postfix(),
        }
    }

    fn postfix(&mut self) -> Hasil<Expr> {
        let mut e = self.atom()?;
        loop {
            match self.intai() {
                Tok::LPar => {
                    let (nama, awal) = match &e.kind {
                        ExprKind::Nama(n) => (n.clone(), e.span),
                        _ => {
                            return Err(Diag::baru(
                                "NUSA-P009",
                                "pemanggilan hanya dapat dilakukan pada nama fungsi",
                            )
                            .di(self.span_intai()))
                        }
                    };
                    self.maju();
                    let (args, span_args) = self.argumen()?;
                    let sp = Span::gabung(awal, span_args);
                    e = self.ekspr(ExprKind::Panggil(nama, args), sp);
                }
                Tok::LBracket => {
                    self.maju();
                    let indeks = self.ekspresi()?;
                    let tutup = self.harap(&Tok::RBracket, "penutup indeks `]`")?;
                    let sp = Span::gabung(e.span, tutup.span);
                    e = self.ekspr(ExprKind::Indeks(Box::new(e), Box::new(indeks)), sp);
                }
                Tok::Titik => {
                    self.maju();
                    let (nama, _) = self.nama("nama metode setelah `.`")?;
                    if self.intai() != &Tok::LPar {
                        return Err(self.galat(
                            "NUSA-P010",
                            "Nusa v0.1 belum mendukung bidang; setelah `.` diharapkan pemanggilan metode",
                        ));
                    }
                    self.maju();
                    let (args, span_args) = self.argumen()?;
                    let sp = Span::gabung(e.span, span_args);
                    e = self.ekspr(ExprKind::Metode(Box::new(e), nama, args), sp);
                }
                _ => break,
            }
        }
        Ok(e)
    }

    fn argumen(&mut self) -> Hasil<(Vec<Expr>, Span)> {
        let mut args = Vec::new();
        if self.intai() != &Tok::RPar {
            loop {
                args.push(self.ekspresi()?);
                if !self.cocok(&Tok::Koma) {
                    break;
                }
            }
        }
        let tutup = self.harap(&Tok::RPar, "penutup argumen `)`")?;
        Ok((args, tutup.span))
    }

    fn atom(&mut self) -> Hasil<Expr> {
        let span = self.span_intai();
        match self.intai().clone() {
            Tok::Bil(n) => {
                self.maju();
                Ok(self.ekspr(ExprKind::Bil(n), span))
            }
            Tok::Des(n) => {
                self.maju();
                Ok(self.ekspr(ExprKind::Des(n), span))
            }
            Tok::Teks(s) => {
                self.maju();
                Ok(self.ekspr(ExprKind::Teks(s), span))
            }
            Tok::Benar => {
                self.maju();
                Ok(self.ekspr(ExprKind::Log(true), span))
            }
            Tok::Salah => {
                self.maju();
                Ok(self.ekspr(ExprKind::Log(false), span))
            }
            Tok::Id(n) => {
                self.maju();
                Ok(self.ekspr(ExprKind::Nama(n), span))
            }
            Tok::LPar => {
                self.maju();
                let e = self.ekspresi()?;
                let tutup = self.harap(&Tok::RPar, "penutup kurung `)`")?;
                let sp = Span::gabung(span, tutup.span);
                Ok(self.ekspr(e.kind, sp))
            }
            Tok::LBracket => {
                self.maju();
                let mut isi = Vec::new();
                if self.intai() != &Tok::RBracket {
                    loop {
                        isi.push(self.ekspresi()?);
                        if !self.cocok(&Tok::Koma) {
                            break;
                        }
                        if self.intai() == &Tok::RBracket {
                            break;
                        }
                    }
                }
                let tutup = self.harap(&Tok::RBracket, "penutup daftar `]`")?;
                let sp = Span::gabung(span, tutup.span);
                Ok(self.ekspr(ExprKind::Daftar(isi), sp))
            }
            lain => Err(Diag::baru(
                "NUSA-P011",
                format!("ekspresi diharapkan, ditemukan {}", lain.deskripsi()),
            )
            .di(span)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::lex;

    fn ast(s: &str) -> Program {
        parse(lex(s).unwrap()).unwrap()
    }

    #[test]
    fn fungsi_sederhana() {
        let p = ast("fungsi utama() -> kosong {\n cetakln(\"halo\")\n}");
        assert_eq!(p.fungsi.len(), 1);
        assert_eq!(p.fungsi[0].nama, "utama");
        assert_eq!(p.fungsi[0].hasil, Tipe::Kosong);
    }

    #[test]
    fn presedensi_dan_singkatan() {
        let p = ast("fng f(a: bil) -> bil {\n kmb a + 2 * 3\n}");
        let Stmt::Kembalikan { nilai: Some(e), .. } = &p.fungsi[0].tubuh[0] else {
            panic!("bukan kembalikan")
        };
        assert!(matches!(e.kind, ExprKind::Biner(BinOp::Tambah, _, _)));
    }

    #[test]
    fn jika_lainnya_dan_rentang() {
        let p = ast(
            "fungsi utama() -> kosong {\n jika 1 < 2 {\n  untuk i dalam 0..10 {\n   cetakln(i)\n  }\n } lainnya {\n  cetakln(0)\n }\n}",
        );
        assert!(matches!(p.fungsi[0].tubuh[0], Stmt::Jika { .. }));
    }

    #[test]
    fn kontrak_terurai() {
        let p = ast(
            "fungsi bagi(a: bil, b: bil) -> bil\n syarat b != 0\n jamin hasil >= 0\n{\n kembalikan a / b\n}",
        );
        assert_eq!(p.fungsi[0].syarat.len(), 1);
        assert_eq!(p.fungsi[0].jamin.len(), 1);
    }

    #[test]
    fn daftar_dan_indeks() {
        let p = ast("fungsi utama() -> kosong {\n var d: daftar<bil> = [1, 2, 3]\n d[0] = 4\n cetakln(d[0])\n}");
        assert!(matches!(p.fungsi[0].tubuh[1], Stmt::Tugas { .. }));
    }
}
