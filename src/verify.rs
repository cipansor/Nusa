//! Verifikasi formal kontrak dengan SMT-LIB (z3).
//!
//! Model: `bil` diperlakukan sebagai bilangan bulat matematis. Pemeriksaan
//! luapan saat berjalan menjamin program tidak pernah menghasilkan nilai yang
//! salah: bila luapan terjadi, program berhenti (panic). Dengan begitu bukti
//! fungsional tetap sah untuk eksekusi yang tidak berhenti.

use crate::ast::*;
use crate::diag::{Diag, Hasil};
use crate::types::Info;
use std::collections::{HashMap, HashSet};
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Clone, Debug)]
enum Tm {
    Bulat(i64),
    Ben(bool),
    Var(String),
    Tidak(Box<Tm>),
    Dan(Vec<Tm>),
    Atau(Vec<Tm>),
    Op(String, Box<Tm>, Box<Tm>),
}

impl Tm {
    fn tulis(&self, keluar: &mut String) {
        match self {
            Tm::Bulat(n) => {
                if *n < 0 {
                    let _ = write!(keluar, "(- {})", (*n as i128).unsigned_abs());
                } else {
                    let _ = write!(keluar, "{}", n);
                }
            }
            Tm::Ben(b) => keluar.push_str(if *b { "true" } else { "false" }),
            Tm::Var(v) => keluar.push_str(v),
            Tm::Tidak(a) => {
                keluar.push_str("(not ");
                a.tulis(keluar);
                keluar.push(')');
            }
            Tm::Dan(xs) => {
                if xs.is_empty() {
                    keluar.push_str("true");
                } else {
                    keluar.push_str("(and");
                    for x in xs {
                        keluar.push(' ');
                        x.tulis(keluar);
                    }
                    keluar.push(')');
                }
            }
            Tm::Atau(xs) => {
                if xs.is_empty() {
                    keluar.push_str("false");
                } else {
                    keluar.push_str("(or");
                    for x in xs {
                        keluar.push(' ');
                        x.tulis(keluar);
                    }
                    keluar.push(')');
                }
            }
            Tm::Op(op, a, b) => {
                let _ = write!(keluar, "({} ", op);
                a.tulis(keluar);
                keluar.push(' ');
                b.tulis(keluar);
                keluar.push(')');
            }
        }
    }

    fn kumpulkan_var(&self, keluar: &mut HashSet<String>) {
        match self {
            Tm::Var(v) => {
                keluar.insert(v.clone());
            }
            Tm::Tidak(a) => a.kumpulkan_var(keluar),
            Tm::Dan(xs) | Tm::Atau(xs) => {
                for x in xs {
                    x.kumpulkan_var(keluar);
                }
            }
            Tm::Op(_, a, b) => {
                a.kumpulkan_var(keluar);
                b.kumpulkan_var(keluar);
            }
            _ => {}
        }
    }
}

#[derive(Clone)]
struct Keadaan {
    env: HashMap<String, Tm>,
    pc: Vec<Tm>,
    aliran: Aliran,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Aliran {
    Normal,
    Kembalikan,
    Hentikan,
    Lanjutkan,
}

struct Vc {
    ket: String,
    anteseden: Vec<Tm>,
    konklusi: Tm,
}

struct Verif<'a> {
    info: &'a Info,
    f: &'a Fungsi,
    pre: Vec<Tm>,
    vcs: Vec<Vc>,
    tipe: HashMap<String, Tipe>,
    deklarasi: HashMap<String, Tipe>,
    n_simbol: usize,
    jalur: usize,
}

const JALUR_MAKS: usize = 64;

struct Rentang<'a> {
    nama: &'a str,
    mulai: &'a Expr,
    akhir: &'a Expr,
    inklusif: bool,
}

fn sanitasi(nama: &str) -> String {
    nama.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

fn tipe_didukung(t: &Tipe) -> bool {
    matches!(t, Tipe::Bil | Tipe::Log)
}

impl<'a> Verif<'a> {
    fn simbol(&mut self, asal: &str, tipe: &Tipe) -> String {
        let nama = format!("v_{}_{}", sanitasi(asal), self.n_simbol);
        self.n_simbol += 1;
        self.deklarasi.insert(nama.clone(), tipe.clone());
        nama
    }

    fn tipe_of(&self, nama: &str) -> Option<Tipe> {
        self.tipe.get(nama).cloned()
    }

    fn expr(&mut self, e: &Expr, env: &HashMap<String, Tm>) -> Result<Tm, String> {
        match &e.kind {
            ExprKind::Bil(n) => Ok(Tm::Bulat(*n)),
            ExprKind::Log(b) => Ok(Tm::Ben(*b)),
            ExprKind::Nama(n) => env
                .get(n)
                .cloned()
                .ok_or_else(|| format!("variabel `{}` tidak ada pada lingkungan verifikasi", n)),
            ExprKind::Unary(UnOp::Neg, a) => {
                let x = self.expr(a, env)?;
                Ok(Tm::Op("-".into(), Box::new(Tm::Bulat(0)), Box::new(x)))
            }
            ExprKind::Unary(UnOp::Tidak, a) => Ok(Tm::Tidak(Box::new(self.expr(a, env)?))),
            ExprKind::Biner(op, a, b) => {
                let x = self.expr(a, env)?;
                let y = self.expr(b, env)?;
                match op {
                    BinOp::Dan => Ok(Tm::Dan(vec![x, y])),
                    BinOp::Atau => Ok(Tm::Atau(vec![x, y])),
                    _ => {
                        let nama = match op {
                            BinOp::Tambah => "+",
                            BinOp::Kurang => "-",
                            BinOp::Kali => "*",
                            BinOp::Bagi => "div",
                            BinOp::Mod => "mod",
                            BinOp::SamaSama => "=",
                            BinOp::TidakSama => "distinct",
                            BinOp::KurangDari => "<",
                            BinOp::KurangSama => "<=",
                            BinOp::LebihDari => ">",
                            BinOp::LebihSama => ">=",
                            _ => unreachable!(),
                        };
                        Ok(Tm::Op(nama.into(), Box::new(x), Box::new(y)))
                    }
                }
            }
            ExprKind::Des(_) => Err("`des` belum didukung verifikasi".into()),
            ExprKind::Teks(_) => Err("`teks` belum didukung verifikasi".into()),
            ExprKind::Daftar(_) => Err("`daftar` belum didukung verifikasi".into()),
            ExprKind::Panggil(n, _) => Err(format!("panggilan `{}` belum didukung verifikasi", n)),
            ExprKind::Metode(_, n, _) => Err(format!("metode `{}` belum didukung verifikasi", n)),
            ExprKind::Indeks(..) => Err("indeks daftar belum didukung verifikasi".into()),
        }
    }

    fn vc(&mut self, ket: String, ante: Vec<Tm>, konklusi: Tm) {
        let mut anteseden = self.pre.clone();
        anteseden.extend(ante);
        self.vcs.push(Vc {
            ket,
            anteseden,
            konklusi,
        });
    }

    fn post(&mut self, st: &Keadaan, ret: Option<&Tm>) -> Result<(), String> {
        if self.f.jamin.is_empty() {
            return Ok(());
        }
        let mut env = st.env.clone();
        let mut ante = st.pc.clone();
        if let Some(r) = ret {
            let h = self.simbol("hasil", &self.f.hasil);
            env.insert("hasil".into(), Tm::Var(h.clone()));
            ante.push(Tm::Op(
                "=".into(),
                Box::new(Tm::Var(h)),
                Box::new(r.clone()),
            ));
        }
        let mut posts = Vec::new();
        for j in &self.f.jamin {
            posts.push(self.expr(j, &env)?);
        }
        let ringkas = self
            .f
            .jamin
            .iter()
            .map(cetak_ekspr)
            .collect::<Vec<_>>()
            .join(" dan ");
        self.vc(format!("jamin: {}", ringkas), ante, Tm::Dan(posts));
        Ok(())
    }

    fn jalan_daftar(
        &mut self,
        stmts: &[Stmt],
        mulai: usize,
        st: Keadaan,
    ) -> Result<Vec<Keadaan>, String> {
        if mulai >= stmts.len() {
            return Ok(vec![st]);
        }
        self.jalur += 1;
        if self.jalur > JALUR_MAKS * 16 {
            return Err("terlalu banyak jalur eksekusi; sederhanakan fungsi".into());
        }
        let lanjut = self.satu(&stmts[mulai], st)?;
        let mut keluar = Vec::new();
        for s in lanjut {
            if s.aliran == Aliran::Normal {
                keluar.extend(self.jalan_daftar(stmts, mulai + 1, s)?);
            } else {
                keluar.push(s);
            }
        }
        Ok(keluar)
    }

    fn satu(&mut self, s: &Stmt, st: Keadaan) -> Result<Vec<Keadaan>, String> {
        match s {
            Stmt::Tetap {
                nama, tipe, nilai, ..
            }
            | Stmt::Var {
                nama, tipe, nilai, ..
            } => {
                let tipe = match tipe {
                    Some(t) => t.clone(),
                    None => self.info.tipe(nilai).clone(),
                };
                if !tipe_didukung(&tipe) {
                    return Err(format!("tipe `{}` belum didukung verifikasi", tipe.nama()));
                }
                let t = self.expr(nilai, &st.env)?;
                let mut env = st.env;
                env.insert(nama.clone(), t);
                self.tipe.insert(nama.clone(), tipe);
                Ok(vec![Keadaan {
                    env,
                    pc: st.pc,
                    aliran: Aliran::Normal,
                }])
            }
            Stmt::Tugas {
                target, op, nilai, ..
            } => {
                let nama = match &target.kind {
                    ExprKind::Nama(n) => n.clone(),
                    _ => return Err("penugasan indeks belum didukung verifikasi".into()),
                };
                if !st.env.contains_key(&nama) {
                    return Err(format!("variabel `{}` tidak ada", nama));
                }
                let lama = st.env.get(&nama).cloned().unwrap();
                let kanan = self.expr(nilai, &st.env)?;
                let baru = match op {
                    TugasOp::Isi => kanan,
                    TugasOp::Tambah => Tm::Op("+".into(), Box::new(lama), Box::new(kanan)),
                    TugasOp::Kurang => Tm::Op("-".into(), Box::new(lama), Box::new(kanan)),
                    TugasOp::Kali => Tm::Op("*".into(), Box::new(lama), Box::new(kanan)),
                    TugasOp::Bagi => Tm::Op("div".into(), Box::new(lama), Box::new(kanan)),
                    TugasOp::Mod => Tm::Op("mod".into(), Box::new(lama), Box::new(kanan)),
                };
                let mut env = st.env;
                env.insert(nama, baru);
                Ok(vec![Keadaan {
                    env,
                    pc: st.pc,
                    aliran: Aliran::Normal,
                }])
            }
            Stmt::Ekspresi(e) => Err(format!(
                "pernyataan `{}` belum didukung verifikasi",
                cetak_ekspr(e)
            )),
            Stmt::Jika {
                kondisi,
                maka,
                lainnya,
                ..
            } => {
                let c = self.expr(kondisi, &st.env)?;
                let mut s1 = st.clone();
                s1.pc.push(c.clone());
                let mut s2 = st.clone();
                s2.pc.push(Tm::Tidak(Box::new(c)));
                let mut keluar = self.jalan_daftar(maka, 0, s1)?;
                let kosong: &[Stmt] = &[];
                let cabang_lain = lainnya.as_deref().unwrap_or(kosong);
                keluar.extend(self.jalan_daftar(cabang_lain, 0, s2)?);
                Ok(keluar)
            }
            Stmt::Selama {
                kondisi,
                invarian,
                tubuh,
                ..
            } => self.selama(kondisi, invarian, tubuh, st),
            Stmt::Untuk {
                nama,
                iterasi,
                invarian,
                tubuh,
                ..
            } => match iterasi {
                Iterasi::Rentang {
                    mulai,
                    akhir,
                    inklusif,
                } => self.rentang(
                    Rentang {
                        nama,
                        mulai,
                        akhir,
                        inklusif: *inklusif,
                    },
                    invarian,
                    tubuh,
                    st,
                ),
                Iterasi::Setiap(_) => {
                    Err("`untuk ... dalam daftar` belum didukung verifikasi".into())
                }
            },
            Stmt::Kembalikan { nilai, .. } => {
                let ret = match nilai {
                    Some(e) => Some(self.expr(e, &st.env)?),
                    None => None,
                };
                self.post(&st, ret.as_ref())?;
                let mut selesai = st;
                selesai.aliran = Aliran::Kembalikan;
                Ok(vec![selesai])
            }
            Stmt::Tegaskan { kondisi, .. } => {
                let c = self.expr(kondisi, &st.env)?;
                self.vc(
                    format!("tegasan: {}", cetak_ekspr(kondisi)),
                    st.pc.clone(),
                    c.clone(),
                );
                let mut pc = st.pc;
                pc.push(c);
                Ok(vec![Keadaan {
                    env: st.env,
                    pc,
                    aliran: Aliran::Normal,
                }])
            }
            Stmt::Hentikan { .. } => Ok(vec![Keadaan {
                env: st.env,
                pc: st.pc,
                aliran: Aliran::Hentikan,
            }]),
            Stmt::Lanjutkan { .. } => Ok(vec![Keadaan {
                env: st.env,
                pc: st.pc,
                aliran: Aliran::Lanjutkan,
            }]),
        }
    }

    fn invarian_di(
        &mut self,
        invarian: &[Expr],
        env: &HashMap<String, Tm>,
    ) -> Result<Vec<Tm>, String> {
        let mut hasil = Vec::new();
        for i in invarian {
            hasil.push(self.expr(i, env)?);
        }
        Ok(hasil)
    }

    fn invarian_otomatis(
        nama: &str,
        a: &str,
        b: &str,
        inklusif: bool,
        env: &HashMap<String, Tm>,
    ) -> Vec<Tm> {
        let i = match env.get(nama) {
            Some(v) => v.clone(),
            None => return Vec::new(),
        };
        let atas = if inklusif {
            Tm::Op(
                "+".into(),
                Box::new(Tm::Var(b.to_string())),
                Box::new(Tm::Bulat(1)),
            )
        } else {
            Tm::Var(b.to_string())
        };
        vec![
            Tm::Op(
                ">=".into(),
                Box::new(i.clone()),
                Box::new(Tm::Var(a.to_string())),
            ),
            Tm::Op("<=".into(), Box::new(i), Box::new(atas)),
        ]
    }

    fn selama(
        &mut self,
        kondisi: &Expr,
        invarian: &[Expr],
        tubuh: &[Stmt],
        st: Keadaan,
    ) -> Result<Vec<Keadaan>, String> {
        if invarian.is_empty() {
            return Err("loop `selama` membutuhkan `invarian` agar dapat diverifikasi".into());
        }
        let invs = self.invarian_di(invarian, &st.env)?;
        let inv = Tm::Dan(invs.clone());
        self.vc(
            format!("invarian awal: {}", ringkas_invarian(invarian)),
            st.pc.clone(),
            inv.clone(),
        );

        let mut modif = HashSet::new();
        kumpulkan_tugas(tubuh, &mut modif);
        let mut env_masuk = st.env.clone();
        for v in &modif {
            if st.env.contains_key(v) {
                if let Some(t) = self.tipe_of(v) {
                    let sym = self.simbol(v, &t);
                    env_masuk.insert(v.clone(), Tm::Var(sym));
                }
            }
        }
        let mut masuk = Keadaan {
            env: env_masuk,
            pc: st.pc.clone(),
            aliran: Aliran::Normal,
        };
        let inv_masuk = self.invarian_di(invarian, &masuk.env)?;
        masuk.pc.extend(inv_masuk);
        let cond_masuk = self.expr(kondisi, &masuk.env)?;
        masuk.pc.push(cond_masuk);
        let badan = self.jalan_daftar(tubuh, 0, masuk)?;
        let mut keluar = Vec::new();
        for s in &badan {
            if s.aliran == Aliran::Kembalikan {
                continue;
            }
            let inv2 = self.invarian_di(invarian, &s.env)?;
            let fakta = Tm::Dan(inv2);
            self.vc(
                format!("invarian terjaga: {}", ringkas_invarian(invarian)),
                s.pc.clone(),
                fakta.clone(),
            );
            if s.aliran == Aliran::Hentikan {
                let mut sb = s.clone();
                sb.pc.push(fakta);
                sb.aliran = Aliran::Normal;
                keluar.push(sb);
            }
        }

        let mut env2 = st.env.clone();
        let mut pc2 = st.pc.clone();
        for v in &modif {
            if let Some(t) = self.tipe_of(v) {
                let sym = self.simbol(v, &t);
                env2.insert(v.clone(), Tm::Var(sym));
            }
        }
        let inv3 = self.invarian_di(invarian, &env2)?;
        pc2.extend(inv3);
        let cond2 = self.expr(kondisi, &env2)?;
        pc2.push(Tm::Tidak(Box::new(cond2)));
        keluar.push(Keadaan {
            env: env2,
            pc: pc2,
            aliran: Aliran::Normal,
        });
        Ok(keluar)
    }

    fn rentang(
        &mut self,
        r: Rentang<'_>,
        invarian: &[Expr],
        tubuh: &[Stmt],
        st: Keadaan,
    ) -> Result<Vec<Keadaan>, String> {
        let Rentang {
            nama,
            mulai,
            akhir,
            inklusif,
        } = r;
        let a = self.expr(mulai, &st.env)?;
        let b = self.expr(akhir, &st.env)?;
        let a_sym = self.simbol(&format!("{}_mulai", nama), &Tipe::Bil);
        let b_sym = self.simbol(&format!("{}_akhir", nama), &Tipe::Bil);
        let i_sym = self.simbol(nama, &Tipe::Bil);

        let mut pc0 = st.pc.clone();
        pc0.push(Tm::Op(
            "=".into(),
            Box::new(Tm::Var(a_sym.clone())),
            Box::new(a),
        ));
        pc0.push(Tm::Op(
            "=".into(),
            Box::new(Tm::Var(b_sym.clone())),
            Box::new(b),
        ));

        let mut env = st.env.clone();
        env.insert(nama.to_string(), Tm::Var(i_sym.clone()));
        self.tipe.insert(nama.to_string(), Tipe::Bil);

        let i = Tm::Var(i_sym.clone());
        let atas = if inklusif {
            Tm::Op(
                "+".into(),
                Box::new(Tm::Var(b_sym.clone())),
                Box::new(Tm::Bulat(1)),
            )
        } else {
            Tm::Var(b_sym.clone())
        };
        let mut invs = vec![
            Tm::Op(
                ">=".into(),
                Box::new(i.clone()),
                Box::new(Tm::Var(a_sym.clone())),
            ),
            Tm::Op("<=".into(), Box::new(i.clone()), Box::new(atas)),
        ];
        invs.extend(self.invarian_di(invarian, &env)?);
        let inv = Tm::Dan(invs.clone());

        let mut ante_init = pc0.clone();
        ante_init.push(Tm::Op(
            "=".into(),
            Box::new(Tm::Var(i_sym.clone())),
            Box::new(Tm::Var(a_sym.clone())),
        ));
        self.vc(
            format!("invarian awal `untuk {}`", nama),
            ante_init,
            inv.clone(),
        );

        let mut modif = HashSet::new();
        kumpulkan_tugas(tubuh, &mut modif);
        modif.insert(nama.to_string());
        let i_masuk = self.simbol(nama, &Tipe::Bil);
        let mut env_masuk = st.env.clone();
        env_masuk.insert(nama.to_string(), Tm::Var(i_masuk.clone()));
        for v in &modif {
            if v != nama && st.env.contains_key(v) {
                if let Some(t) = self.tipe_of(v) {
                    let sym = self.simbol(v, &t);
                    env_masuk.insert(v.clone(), Tm::Var(sym));
                }
            }
        }
        let mut masuk = Keadaan {
            env: env_masuk,
            pc: pc0.clone(),
            aliran: Aliran::Normal,
        };
        masuk.pc.extend(Self::invarian_otomatis(
            nama, &a_sym, &b_sym, inklusif, &masuk.env,
        ));
        masuk.pc.extend(self.invarian_di(invarian, &masuk.env)?);
        let i_m = masuk.env.get(nama).cloned().unwrap();
        let cond_masuk = Tm::Op(
            if inklusif { "<=" } else { "<" }.into(),
            Box::new(i_m),
            Box::new(Tm::Var(b_sym.clone())),
        );
        masuk.pc.push(cond_masuk);
        let badan = self.jalan_daftar(tubuh, 0, masuk)?;
        let mut keluar = Vec::new();
        for s in &badan {
            if s.aliran == Aliran::Kembalikan {
                continue;
            }
            let mut check = Self::invarian_otomatis(nama, &a_sym, &b_sym, inklusif, &s.env);
            check.extend(self.invarian_di(invarian, &s.env)?);
            if s.aliran == Aliran::Hentikan {
                let fakta = Tm::Dan(check);
                self.vc(
                    format!("invarian saat `hentikan` (`untuk {}`)", nama),
                    s.pc.clone(),
                    fakta.clone(),
                );
                let mut sb = s.clone();
                sb.pc.push(fakta);
                sb.aliran = Aliran::Normal;
                keluar.push(sb);
                continue;
            }
            let mut env2 = s.env.clone();
            let lama = env2
                .get(nama)
                .cloned()
                .unwrap_or_else(|| Tm::Var(i_sym.clone()));
            env2.insert(
                nama.to_string(),
                Tm::Op("+".into(), Box::new(lama), Box::new(Tm::Bulat(1))),
            );
            let mut check2 = Self::invarian_otomatis(nama, &a_sym, &b_sym, inklusif, &env2);
            check2.extend(self.invarian_di(invarian, &env2)?);
            self.vc(
                format!("invarian terjaga (`untuk {}`)", nama),
                s.pc.clone(),
                Tm::Dan(check2),
            );
        }

        let mut env2 = st.env.clone();
        for v in &modif {
            if v == nama {
                continue;
            }
            if let Some(t) = self.tipe_of(v) {
                let sym = self.simbol(v, &t);
                env2.insert(v.clone(), Tm::Var(sym));
            }
        }
        let i2 = self.simbol(nama, &Tipe::Bil);
        env2.insert(nama.to_string(), Tm::Var(i2.clone()));

        let mut pc2 = pc0;
        let mut inv_exit = Self::invarian_otomatis(nama, &a_sym, &b_sym, inklusif, &env2);
        inv_exit.extend(self.invarian_di(invarian, &env2)?);
        pc2.extend(inv_exit);
        let cond_exit = Tm::Op(
            if inklusif { "<=" } else { "<" }.into(),
            Box::new(Tm::Var(i2)),
            Box::new(Tm::Var(b_sym)),
        );
        pc2.push(Tm::Tidak(Box::new(cond_exit)));
        keluar.push(Keadaan {
            env: env2,
            pc: pc2,
            aliran: Aliran::Normal,
        });
        Ok(keluar)
    }
}

fn ringkas_invarian(invarian: &[Expr]) -> String {
    invarian
        .iter()
        .map(cetak_ekspr)
        .collect::<Vec<_>>()
        .join(" dan ")
}

fn kumpulkan_tugas(stmts: &[Stmt], keluar: &mut HashSet<String>) {
    for s in stmts {
        match s {
            Stmt::Tugas { target, .. } => {
                if let ExprKind::Nama(n) = &target.kind {
                    keluar.insert(n.clone());
                }
            }
            Stmt::Jika { maka, lainnya, .. } => {
                kumpulkan_tugas(maka, keluar);
                if let Some(l) = lainnya {
                    kumpulkan_tugas(l, keluar);
                }
            }
            Stmt::Selama { tubuh, .. } => kumpulkan_tugas(tubuh, keluar),
            Stmt::Untuk { nama, tubuh, .. } => {
                keluar.insert(nama.clone());
                kumpulkan_tugas(tubuh, keluar);
            }
            _ => {}
        }
    }
}

fn periksa_dukungan(f: &Fungsi, info: &Info) -> Result<(), String> {
    if f.hasil != Tipe::Kosong && !tipe_didukung(&f.hasil) {
        return Err(format!(
            "tipe hasil `{}` belum didukung verifikasi",
            f.hasil.nama()
        ));
    }
    for p in &f.params {
        if !tipe_didukung(&p.tipe) {
            return Err(format!(
                "parameter `{}` bertipe `{}` yang belum didukung verifikasi",
                p.nama,
                p.tipe.nama()
            ));
        }
    }
    fn ekspr(e: &Expr) -> Result<(), String> {
        match &e.kind {
            ExprKind::Bil(_) | ExprKind::Log(_) | ExprKind::Nama(_) => Ok(()),
            ExprKind::Unary(_, a) => ekspr(a),
            ExprKind::Biner(_, a, b) => {
                ekspr(a)?;
                ekspr(b)
            }
            ExprKind::Des(_) => Err("`des` belum didukung verifikasi".into()),
            ExprKind::Teks(_) => Err("`teks` belum didukung verifikasi".into()),
            ExprKind::Daftar(_) => Err("`daftar` belum didukung verifikasi".into()),
            ExprKind::Panggil(n, _) => Err(format!("panggilan `{}` belum didukung verifikasi", n)),
            ExprKind::Metode(_, n, _) => Err(format!("metode `{}` belum didukung verifikasi", n)),
            ExprKind::Indeks(..) => Err("indeks daftar belum didukung verifikasi".into()),
        }
    }
    fn pernyataan(s: &Stmt, info: &Info) -> Result<(), String> {
        match s {
            Stmt::Tetap { tipe, nilai, .. } | Stmt::Var { tipe, nilai, .. } => {
                let t = match tipe {
                    Some(t) => t.clone(),
                    None => info.tipe(nilai).clone(),
                };
                if !tipe_didukung(&t) {
                    return Err(format!("tipe `{}` belum didukung verifikasi", t.nama()));
                }
                ekspr(nilai)
            }
            Stmt::Tugas { target, nilai, .. } => {
                if !matches!(target.kind, ExprKind::Nama(_)) {
                    return Err("penugasan indeks belum didukung verifikasi".into());
                }
                ekspr(nilai)
            }
            Stmt::Ekspresi(e) => Err(format!(
                "pernyataan `{}` belum didukung verifikasi",
                cetak_ekspr(e)
            )),
            Stmt::Jika {
                kondisi,
                maka,
                lainnya,
                ..
            } => {
                ekspr(kondisi)?;
                for x in maka {
                    pernyataan(x, info)?;
                }
                if let Some(l) = lainnya {
                    for x in l {
                        pernyataan(x, info)?;
                    }
                }
                Ok(())
            }
            Stmt::Selama {
                kondisi,
                invarian,
                tubuh,
                ..
            } => {
                if invarian.is_empty() {
                    return Err(
                        "loop `selama` membutuhkan `invarian` agar dapat diverifikasi".into(),
                    );
                }
                ekspr(kondisi)?;
                for i in invarian {
                    ekspr(i)?;
                }
                for x in tubuh {
                    pernyataan(x, info)?;
                }
                Ok(())
            }
            Stmt::Untuk {
                iterasi,
                invarian,
                tubuh,
                ..
            } => {
                match iterasi {
                    Iterasi::Rentang { mulai, akhir, .. } => {
                        ekspr(mulai)?;
                        ekspr(akhir)?;
                    }
                    Iterasi::Setiap(_) => {
                        return Err("`untuk ... dalam daftar` belum didukung verifikasi".into())
                    }
                }
                for i in invarian {
                    ekspr(i)?;
                }
                for x in tubuh {
                    pernyataan(x, info)?;
                }
                Ok(())
            }
            Stmt::Kembalikan { nilai, .. } => {
                if let Some(e) = nilai {
                    ekspr(e)?;
                }
                Ok(())
            }
            Stmt::Tegaskan { kondisi, .. } => ekspr(kondisi),
            Stmt::Hentikan { .. } | Stmt::Lanjutkan { .. } => Ok(()),
        }
    }
    for s in &f.tubuh {
        pernyataan(s, info)?;
    }
    Ok(())
}

fn ada_tegasan(stmts: &[Stmt]) -> bool {
    stmts.iter().any(|s| match s {
        Stmt::Tegaskan { .. } => true,
        Stmt::Jika { maka, lainnya, .. } => {
            ada_tegasan(maka) || lainnya.as_deref().is_some_and(ada_tegasan)
        }
        Stmt::Selama { tubuh, .. } | Stmt::Untuk { tubuh, .. } => ada_tegasan(tubuh),
        _ => false,
    })
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Status {
    Terbukti,
    Gagal,
    TidakDiketahui,
    Dilewati,
    TanpaKontrak,
    TidakDiperiksa,
}

impl Status {
    pub fn label(&self) -> &'static str {
        match self {
            Status::Terbukti => "TERBUKTI",
            Status::Gagal => "GAGAL",
            Status::TidakDiketahui => "TIDAK DIKETAHUI",
            Status::Dilewati => "DILEWATI",
            Status::TanpaKontrak => "TANPA KONTRAK",
            Status::TidakDiperiksa => "TIDAK DIPERIKSA",
        }
    }
}

#[derive(Clone, Debug)]
pub struct HasilVc {
    pub ket: String,
    pub status: Status,
    pub berkas: Option<PathBuf>,
    pub model: Option<String>,
    pub alasan: Option<String>,
}

#[derive(Clone, Debug)]
pub struct LaporanFungsi {
    pub nama: String,
    pub status: Status,
    pub vcs: Vec<HasilVc>,
    pub alasan: Option<String>,
}

#[derive(Clone, Debug)]
pub struct Laporan {
    pub fungsi: Vec<LaporanFungsi>,
    pub dir: PathBuf,
}

fn tulis_smt(vc: &Vc, deklarasi: &HashMap<String, Tipe>) -> String {
    let mut dipakai = HashSet::new();
    for a in &vc.anteseden {
        a.kumpulkan_var(&mut dipakai);
    }
    vc.konklusi.kumpulkan_var(&mut dipakai);
    let mut urut: Vec<String> = dipakai.into_iter().collect();
    urut.sort();
    let mut s = String::new();
    s.push_str("; Berkas bukti otomatis dari kompiler Nusa\n");
    s.push_str("(set-logic ALL)\n");
    for v in &urut {
        let smt = match deklarasi.get(v) {
            Some(Tipe::Log) => "Bool",
            _ => "Int",
        };
        let _ = writeln!(s, "(declare-const {} {})", v, smt);
    }
    let rumus = if vc.anteseden.is_empty() {
        vc.konklusi.clone()
    } else {
        Tm::Op(
            "=>".into(),
            Box::new(Tm::Dan(vc.anteseden.clone())),
            Box::new(vc.konklusi.clone()),
        )
    };
    s.push_str("(assert (not ");
    rumus.tulis(&mut s);
    s.push_str("))\n");
    s.push_str("(check-sat)\n(get-model)\n");
    s
}

fn jalankan_z3(berkas: &Path) -> (Status, Option<String>, Option<String>) {
    match Command::new("z3").arg("-smt2").arg(berkas).output() {
        Err(_) => (
            Status::TidakDiketahui,
            None,
            Some("z3 tidak ditemukan; berkas SMT tetap ditulis".into()),
        ),
        Ok(o) => {
            let out = String::from_utf8_lossy(&o.stdout).to_string();
            let kata = out.split_whitespace().next().unwrap_or("");
            if !o.status.success() && !matches!(kata, "sat" | "unsat" | "unknown") {
                let err = format!(
                    "{}{}",
                    String::from_utf8_lossy(&o.stdout),
                    String::from_utf8_lossy(&o.stderr)
                );
                return (
                    Status::TidakDiketahui,
                    None,
                    Some(format!("z3 keluar dengan galat: {}", err.trim())),
                );
            }
            match kata {
                "unsat" => (Status::Terbukti, None, None),
                "sat" => {
                    let model = out.lines().skip(1).take(8).collect::<Vec<_>>().join("\n");
                    (
                        Status::Gagal,
                        Some(model),
                        Some("kontrak tidak berlaku untuk semua masukan".into()),
                    )
                }
                "unknown" => (
                    Status::TidakDiketahui,
                    None,
                    Some("z3 menjawab `unknown`".into()),
                ),
                lain => (
                    Status::TidakDiketahui,
                    None,
                    Some(format!("jawaban z3 tidak dikenal: `{}`", lain)),
                ),
            }
        }
    }
}

fn verifikasi_fungsi(f: &Fungsi, info: &Info, dir: &Path, pakai_z3: bool) -> LaporanFungsi {
    if f.syarat.is_empty() && f.jamin.is_empty() && !ada_tegasan(&f.tubuh) {
        return LaporanFungsi {
            nama: f.nama.clone(),
            status: Status::TanpaKontrak,
            vcs: Vec::new(),
            alasan: Some("tidak ada `syarat`, `jamin`, atau `tegaskan`".into()),
        };
    }
    if let Err(alasan) = periksa_dukungan(f, info) {
        return LaporanFungsi {
            nama: f.nama.clone(),
            status: Status::Dilewati,
            vcs: Vec::new(),
            alasan: Some(alasan),
        };
    }
    let mut v = Verif {
        info,
        f,
        pre: Vec::new(),
        vcs: Vec::new(),
        tipe: HashMap::new(),
        deklarasi: HashMap::new(),
        n_simbol: 0,
        jalur: 0,
    };
    let hasil = (|| -> Result<(), String> {
        let mut env = HashMap::new();
        for p in &f.params {
            let sym = v.simbol(&p.nama, &p.tipe);
            env.insert(p.nama.clone(), Tm::Var(sym));
            v.tipe.insert(p.nama.clone(), p.tipe.clone());
        }
        let mut pre = Vec::new();
        for s in &f.syarat {
            pre.push(v.expr(s, &env)?);
        }
        v.pre = pre;
        let awal = Keadaan {
            env,
            pc: Vec::new(),
            aliran: Aliran::Normal,
        };
        let akhir = v.jalan_daftar(&f.tubuh, 0, awal)?;
        for st in akhir {
            match st.aliran {
                Aliran::Kembalikan => {}
                Aliran::Normal => {
                    if f.hasil == Tipe::Kosong {
                        v.post(&st, None)?;
                    } else {
                        return Err("fungsi tidak selalu mengembalikan nilai".into());
                    }
                }
                _ => return Err("aliran kendali loop tidak sah di luar loop".into()),
            }
        }
        Ok(())
    })();
    if let Err(alasan) = hasil {
        return LaporanFungsi {
            nama: f.nama.clone(),
            status: Status::Dilewati,
            vcs: Vec::new(),
            alasan: Some(alasan),
        };
    }
    if v.vcs.is_empty() {
        return LaporanFungsi {
            nama: f.nama.clone(),
            status: Status::TanpaKontrak,
            vcs: Vec::new(),
            alasan: Some("tidak ada kewajiban verifikasi".into()),
        };
    }
    if let Err(e) = fs::create_dir_all(dir) {
        return LaporanFungsi {
            nama: f.nama.clone(),
            status: Status::TidakDiperiksa,
            vcs: Vec::new(),
            alasan: Some(format!("tidak dapat membuat direktori bukti: {}", e)),
        };
    }
    let mut hasil_vc = Vec::new();
    for (i, vc) in v.vcs.iter().enumerate() {
        let berkas = dir.join(format!("{}.{}.smt2", sanitasi(&f.nama), i + 1));
        let isi = tulis_smt(vc, &v.deklarasi);
        let _ = fs::write(&berkas, isi);
        let (status, model, alasan) = if pakai_z3 {
            jalankan_z3(&berkas)
        } else {
            (
                Status::TidakDiperiksa,
                None,
                Some("z3 tidak dijalankan (--tanpa-z3)".into()),
            )
        };
        hasil_vc.push(HasilVc {
            ket: vc.ket.clone(),
            status,
            berkas: Some(berkas),
            model,
            alasan,
        });
    }
    let status = if hasil_vc.iter().any(|h| h.status == Status::Gagal) {
        Status::Gagal
    } else if hasil_vc.iter().any(|h| h.status == Status::TidakDiketahui) {
        Status::TidakDiketahui
    } else if hasil_vc.iter().any(|h| h.status == Status::TidakDiperiksa) {
        Status::TidakDiperiksa
    } else {
        Status::Terbukti
    };
    LaporanFungsi {
        nama: f.nama.clone(),
        status,
        vcs: hasil_vc,
        alasan: None,
    }
}

pub fn verifikasi_program(program: &Program, info: &Info, dir: &Path, pakai_z3: bool) -> Laporan {
    let fungsi = program
        .fungsi
        .iter()
        .map(|f| verifikasi_fungsi(f, info, dir, pakai_z3))
        .collect();
    Laporan {
        fungsi,
        dir: dir.to_path_buf(),
    }
}

pub fn verifikasi_berkas(berkas: &Path) -> Hasil<Laporan> {
    let sumber = fs::read_to_string(berkas).map_err(|e| {
        Diag::baru(
            "NUSA-D006",
            format!("tidak dapat membaca `{}`: {}", berkas.display(), e),
        )
    })?;
    let a = crate::analisis(&sumber)?;
    Ok(verifikasi_program(
        &a.program,
        &a.info,
        Path::new("bukti"),
        true,
    ))
}

pub fn perintah(args: &[String]) {
    let mut berkas: Option<PathBuf> = None;
    let mut pakai_z3 = true;
    let mut dir = PathBuf::from("bukti");
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--tanpa-z3" => pakai_z3 = false,
            "--keluaran-dir" | "--dir" => {
                i += 1;
                if i >= args.len() {
                    eprintln!("nusa: `{}` membutuhkan direktori", args[i - 1]);
                    std::process::exit(2);
                }
                dir = PathBuf::from(&args[i]);
            }
            lain if lain.starts_with('-') => {
                eprintln!("nusa: opsi `{}` tidak dikenal", lain);
                std::process::exit(2);
            }
            lain => {
                if berkas.is_some() {
                    eprintln!("nusa: hanya satu berkas sumber untuk verifikasi");
                    std::process::exit(2);
                }
                berkas = Some(PathBuf::from(lain));
            }
        }
        i += 1;
    }
    let berkas = match berkas {
        Some(b) => b,
        None => {
            eprintln!("nusa: pemakaian: nusa verifikasi <berkas.ns> [--tanpa-z3]");
            std::process::exit(2);
        }
    };
    let sumber = match fs::read_to_string(&berkas) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("nusa: tidak dapat membaca `{}`: {}", berkas.display(), e);
            std::process::exit(1);
        }
    };
    let a = match crate::analisis(&sumber) {
        Ok(a) => a,
        Err(d) => {
            eprint!("{}", d.render(&sumber, &berkas.display().to_string()));
            std::process::exit(1);
        }
    };
    let laporan = verifikasi_program(&a.program, &a.info, &dir, pakai_z3);
    println!("verifikasi formal: {}", berkas.display());
    let mut terbukti = 0;
    let mut gagal = 0;
    let mut tidak_diketahui = 0;
    let mut dilewati = 0;
    let mut tanpa_kontrak = 0;
    for f in &laporan.fungsi {
        match f.status {
            Status::Terbukti => terbukti += 1,
            Status::Gagal => gagal += 1,
            Status::TidakDiketahui => tidak_diketahui += 1,
            Status::Dilewati => dilewati += 1,
            Status::TanpaKontrak => tanpa_kontrak += 1,
            Status::TidakDiperiksa => tidak_diketahui += 1,
        }
        println!("  fungsi `{}`: {}", f.nama, f.status.label());
        if let Some(a) = &f.alasan {
            println!("    - {}", a);
        }
        for vc in &f.vcs {
            println!("    [{}] {}", vc.status.label(), vc.ket);
            if let Some(m) = &vc.model {
                if vc.status == Status::Gagal {
                    println!("        model: {}", m.replace('\n', " "));
                }
            }
            if let Some(a) = &vc.alasan {
                if vc.status != Status::Gagal {
                    println!("        {}", a);
                }
            }
        }
    }
    println!(
        "ringkasan: terbukti={} gagal={} tidak-diketahui={} dilewati={} tanpa-kontrak={}",
        terbukti, gagal, tidak_diketahui, dilewati, tanpa_kontrak
    );
    if pakai_z3 {
        println!("berkas bukti SMT: {}", laporan.dir.display());
    }
    if gagal > 0 || (pakai_z3 && tidak_diketahui > 0) {
        std::process::exit(1);
    }
}
