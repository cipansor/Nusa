//! Lexer bahasa Nusa. Kata kunci Indonesia + singkatan.

use crate::diag::{Diag, Hasil, Span};

#[derive(Clone, Debug, PartialEq)]
pub enum Tok {
    Bil(i64),
    Des(f64),
    Teks(String),
    Id(String),

    Fungsi,
    Kembalikan,
    Jika,
    Lainnya,
    Selama,
    Untuk,
    Dalam,
    Tetap,
    Var,
    Dan,
    Atau,
    Bukan,
    Benar,
    Salah,
    Kosong,
    TBil,
    TDes,
    TLog,
    TTeks,
    TDaftar,
    Syarat,
    Jamin,
    Invarian,
    Tegaskan,
    Hentikan,
    Lanjutkan,

    LPar,
    RPar,
    LBrace,
    RBrace,
    LBracket,
    RBracket,
    Koma,
    TitikDua,
    Panah,
    Titik,
    TitikTitik,
    TitikTitikSama,

    Plus,
    Minus,
    Bintang,
    GarisMiring,
    Persen,
    Sama,
    SamaSama,
    TidakSama,
    Kurang,
    KurangSama,
    Lebih,
    LebihSama,
    PlusSama,
    MinusSama,
    BintangSama,
    GarisMiringSama,
    PersenSama,
    DanDan,
    AtauAtau,
    Seru,

    BaruBaris,
    Akhir,
}

impl Tok {
    pub fn deskripsi(&self) -> String {
        match self {
            Tok::Bil(n) => format!("angka `{}`", n),
            Tok::Des(n) => format!("angka `{}`", n),
            Tok::Teks(_) => "teks".to_string(),
            Tok::Id(n) => format!("nama `{}`", n),
            Tok::Akhir => "akhir berkas".to_string(),
            Tok::BaruBaris => "baris baru".to_string(),
            lainnya => format!("`{}`", lainnya.teks()),
        }
    }

    pub fn teks(&self) -> &'static str {
        match self {
            Tok::Fungsi => "fungsi",
            Tok::Kembalikan => "kembalikan",
            Tok::Jika => "jika",
            Tok::Lainnya => "lainnya",
            Tok::Selama => "selama",
            Tok::Untuk => "untuk",
            Tok::Dalam => "dalam",
            Tok::Tetap => "tetap",
            Tok::Var => "var",
            Tok::Dan => "dan",
            Tok::Atau => "atau",
            Tok::Bukan => "bukan",
            Tok::Benar => "benar",
            Tok::Salah => "salah",
            Tok::Kosong => "kosong",
            Tok::TBil => "bil",
            Tok::TDes => "des",
            Tok::TLog => "log",
            Tok::TTeks => "teks",
            Tok::TDaftar => "daftar",
            Tok::Syarat => "syarat",
            Tok::Jamin => "jamin",
            Tok::Invarian => "invarian",
            Tok::Tegaskan => "tegaskan",
            Tok::Hentikan => "hentikan",
            Tok::Lanjutkan => "lanjutkan",
            Tok::LPar => "(",
            Tok::RPar => ")",
            Tok::LBrace => "{",
            Tok::RBrace => "}",
            Tok::LBracket => "[",
            Tok::RBracket => "]",
            Tok::Koma => ",",
            Tok::TitikDua => ":",
            Tok::Panah => "->",
            Tok::Titik => ".",
            Tok::TitikTitik => "..",
            Tok::TitikTitikSama => "..=",
            Tok::Plus => "+",
            Tok::Minus => "-",
            Tok::Bintang => "*",
            Tok::GarisMiring => "/",
            Tok::Persen => "%",
            Tok::Sama => "=",
            Tok::SamaSama => "==",
            Tok::TidakSama => "!=",
            Tok::Kurang => "<",
            Tok::KurangSama => "<=",
            Tok::Lebih => ">",
            Tok::LebihSama => ">=",
            Tok::PlusSama => "+=",
            Tok::MinusSama => "-=",
            Tok::BintangSama => "*=",
            Tok::GarisMiringSama => "/=",
            Tok::PersenSama => "%=",
            Tok::DanDan => "&&",
            Tok::AtauAtau => "||",
            Tok::Seru => "!",
            Tok::BaruBaris => "baris baru",
            Tok::Akhir => "akhir berkas",
            Tok::Bil(_) | Tok::Des(_) | Tok::Teks(_) | Tok::Id(_) => "literal",
        }
    }
}

#[derive(Clone, Debug)]
pub struct Token {
    pub tok: Tok,
    pub span: Span,
}

pub fn lex(sumber: &str) -> Hasil<Vec<Token>> {
    let mut lx = Lexer {
        s: sumber,
        i: 0,
        kedalaman: 0,
        tokens: Vec::new(),
        lanjut: false,
    };
    lx.jalan()?;
    Ok(lx.tokens)
}

struct Lexer<'a> {
    s: &'a str,
    i: usize,
    /// Kedalaman tanda kurung `(` dan `[`; baris baru di dalamnya diabaikan.
    kedalaman: i32,
    tokens: Vec<Token>,
    /// Token terakhir memungkinkan ekspresi berlanjut ke baris berikutnya.
    lanjut: bool,
}

impl<'a> Lexer<'a> {
    fn jalan(&mut self) -> Hasil<()> {
        while self.i < self.s.len() {
            let c = self.potong();
            match c {
                ' ' | '\t' | '\r' => {}
                '\n' => self.baris_baru(),
                '#' => self.lewati_komentar(),
                '/' if self.sisa().starts_with('/') => self.lewati_komentar(),
                '0'..='9' => self.angka()?,
                '"' => self.teks()?,
                c if c == '_' || c.is_alphabetic() => {
                    let mulai = self.i - c.len_utf8();
                    self.identifier(mulai)?;
                }
                '(' => {
                    self.kedalaman += 1;
                    self.dorong(Tok::LPar, 1);
                }
                ')' => {
                    self.kedalaman -= 1;
                    self.dorong(Tok::RPar, 1);
                }
                '[' => {
                    self.kedalaman += 1;
                    self.dorong(Tok::LBracket, 1);
                }
                ']' => {
                    self.kedalaman -= 1;
                    self.dorong(Tok::RBracket, 1);
                }
                '{' => self.dorong(Tok::LBrace, 1),
                '}' => self.dorong(Tok::RBrace, 1),
                ',' => self.dorong(Tok::Koma, 1),
                ':' => self.dorong(Tok::TitikDua, 1),
                ';' => {
                    self.tokens.push(Token {
                        tok: Tok::BaruBaris,
                        span: Span::baru(self.i - 1, self.i),
                    });
                    self.lanjut = false;
                }
                '+' => {
                    if self.ambil('=') {
                        self.dorong(Tok::PlusSama, 2);
                    } else {
                        self.dorong(Tok::Plus, 1);
                    }
                }
                '-' => {
                    if self.ambil('>') {
                        self.dorong(Tok::Panah, 2);
                    } else if self.ambil('=') {
                        self.dorong(Tok::MinusSama, 2);
                    } else {
                        self.dorong(Tok::Minus, 1);
                    }
                }
                '*' => {
                    if self.ambil('=') {
                        self.dorong(Tok::BintangSama, 2);
                    } else {
                        self.dorong(Tok::Bintang, 1);
                    }
                }
                '/' => {
                    if self.ambil('=') {
                        self.dorong(Tok::GarisMiringSama, 2);
                    } else {
                        self.dorong(Tok::GarisMiring, 1);
                    }
                }
                '%' => {
                    if self.ambil('=') {
                        self.dorong(Tok::PersenSama, 2);
                    } else {
                        self.dorong(Tok::Persen, 1);
                    }
                }
                '=' => {
                    if self.ambil('=') {
                        self.dorong(Tok::SamaSama, 2);
                    } else {
                        self.dorong(Tok::Sama, 1);
                    }
                }
                '!' => {
                    if self.ambil('=') {
                        self.dorong(Tok::TidakSama, 2);
                    } else {
                        self.dorong(Tok::Seru, 1);
                    }
                }
                '<' => {
                    if self.ambil('=') {
                        self.dorong(Tok::KurangSama, 2);
                    } else {
                        self.dorong(Tok::Kurang, 1);
                    }
                }
                '>' => {
                    if self.ambil('=') {
                        self.dorong(Tok::LebihSama, 2);
                    } else {
                        self.dorong(Tok::Lebih, 1);
                    }
                }
                '&' => {
                    if self.ambil('&') {
                        self.dorong(Tok::DanDan, 2);
                    } else {
                        return Err(self.galat(
                            "NUSA-L001",
                            "`&` tidak dikenal; gunakan `dan` atau `&&`",
                            self.i - 1,
                            self.i,
                        ));
                    }
                }
                '|' => {
                    if self.ambil('|') {
                        self.dorong(Tok::AtauAtau, 2);
                    } else {
                        return Err(self.galat(
                            "NUSA-L002",
                            "`|` tidak dikenal; gunakan `atau` atau `||`",
                            self.i - 1,
                            self.i,
                        ));
                    }
                }
                '.' => {
                    if self.ambil('.') {
                        if self.ambil('=') {
                            self.dorong(Tok::TitikTitikSama, 3);
                        } else {
                            self.dorong(Tok::TitikTitik, 2);
                        }
                    } else {
                        self.dorong(Tok::Titik, 1);
                    }
                }
                c => {
                    return Err(self.galat(
                        "NUSA-L003",
                        format!("karakter `{}` tidak dikenal", c),
                        self.i - c.len_utf8(),
                        self.i,
                    ));
                }
            }
        }
        self.tokens.push(Token {
            tok: Tok::Akhir,
            span: Span::baru(self.s.len(), self.s.len()),
        });
        Ok(())
    }

    fn baris_baru(&mut self) {
        if self.kedalaman > 0 || self.lanjut {
            self.lanjut = false;
            return;
        }
        let pos = self.i - 1;
        if !matches!(
            self.tokens.last().map(|t| &t.tok),
            Some(Tok::BaruBaris) | None
        ) {
            self.tokens.push(Token {
                tok: Tok::BaruBaris,
                span: Span::baru(pos, self.i),
            });
        }
    }

    fn lewati_komentar(&mut self) {
        while self.i < self.s.len() && self.s.as_bytes()[self.i] != b'\n' {
            self.i += 1;
        }
    }

    fn angka(&mut self) -> Hasil<()> {
        let mulai = self.i - 1;
        let sisa = self.sisa();
        if !self.lanjut && sisa.starts_with("..") {
            // tidak mungkin: angka diikuti titik-titik ditangani di bawah
        }
        if sisa.starts_with('x') || sisa.starts_with('X') {
            self.i += 1;
            let awal = self.i;
            while self.i < self.s.len()
                && (self.s.as_bytes()[self.i].is_ascii_hexdigit()
                    || self.s.as_bytes()[self.i] == b'_')
            {
                self.i += 1;
            }
            let digit = self.s[awal..self.i].replace('_', "");
            if digit.is_empty() {
                return Err(self.galat("NUSA-L004", "angka heksadesimal kosong", mulai, self.i));
            }
            let n = i64::from_str_radix(&digit, 16).map_err(|_| {
                self.galat(
                    "NUSA-L005",
                    "angka heksadesimal terlalu besar untuk `bil` (i64)",
                    mulai,
                    self.i,
                )
            })?;
            self.tokens.push(Token {
                tok: Tok::Bil(n),
                span: Span::baru(mulai, self.i),
            });
            self.lanjut = false;
            return Ok(());
        }
        if sisa.starts_with('b') || sisa.starts_with('B') {
            self.i += 1;
            let awal = self.i;
            while self.i < self.s.len()
                && (self.s.as_bytes()[self.i] == b'0'
                    || self.s.as_bytes()[self.i] == b'1'
                    || self.s.as_bytes()[self.i] == b'_')
            {
                self.i += 1;
            }
            let digit = self.s[awal..self.i].replace('_', "");
            if digit.is_empty() {
                return Err(self.galat("NUSA-L006", "angka biner kosong", mulai, self.i));
            }
            let n = i64::from_str_radix(&digit, 2).map_err(|_| {
                self.galat(
                    "NUSA-L005",
                    "angka biner terlalu besar untuk `bil` (i64)",
                    mulai,
                    self.i,
                )
            })?;
            self.tokens.push(Token {
                tok: Tok::Bil(n),
                span: Span::baru(mulai, self.i),
            });
            self.lanjut = false;
            return Ok(());
        }

        let mut desimal = false;
        while self.i < self.s.len() {
            let b = self.s.as_bytes()[self.i];
            if b.is_ascii_digit() || b == b'_' {
                self.i += 1;
            } else {
                break;
            }
        }
        if self.i < self.s.len()
            && self.s.as_bytes()[self.i] == b'.'
            && !self.sisa()[1..].starts_with('.')
            && self
                .sisa()
                .as_bytes()
                .get(1)
                .is_some_and(|b| b.is_ascii_digit())
        {
            desimal = true;
            self.i += 1;
            while self.i < self.s.len()
                && (self.s.as_bytes()[self.i].is_ascii_digit() || self.s.as_bytes()[self.i] == b'_')
            {
                self.i += 1;
            }
        }
        if self.i < self.s.len()
            && (self.s.as_bytes()[self.i] == b'e' || self.s.as_bytes()[self.i] == b'E')
        {
            let simpan = self.i;
            let mut j = self.i + 1;
            if j < self.s.len() && (self.s.as_bytes()[j] == b'+' || self.s.as_bytes()[j] == b'-') {
                j += 1;
            }
            if j < self.s.len() && self.s.as_bytes()[j].is_ascii_digit() {
                desimal = true;
                self.i = j;
                while self.i < self.s.len() && self.s.as_bytes()[self.i].is_ascii_digit() {
                    self.i += 1;
                }
            } else {
                self.i = simpan;
            }
        }
        let mentah = self.s[mulai..self.i].replace('_', "");
        if desimal {
            let n: f64 = mentah
                .parse()
                .map_err(|_| self.galat("NUSA-L007", "angka desimal tidak valid", mulai, self.i))?;
            self.tokens.push(Token {
                tok: Tok::Des(n),
                span: Span::baru(mulai, self.i),
            });
        } else {
            let n: i64 = mentah.parse().map_err(|_| {
                self.galat(
                    "NUSA-L005",
                    "angka terlalu besar untuk `bil` (i64)",
                    mulai,
                    self.i,
                )
            })?;
            self.tokens.push(Token {
                tok: Tok::Bil(n),
                span: Span::baru(mulai, self.i),
            });
        }
        self.lanjut = false;
        Ok(())
    }

    fn teks(&mut self) -> Hasil<()> {
        let mulai = self.i - 1;
        let mut isi = String::new();
        loop {
            if self.i >= self.s.len() {
                return Err(self.galat(
                    "NUSA-L008",
                    "teks tidak ditutup dengan `\"`",
                    mulai,
                    self.i,
                ));
            }
            let c = self.potong();
            match c {
                '"' => break,
                '\\' => {
                    if self.i >= self.s.len() {
                        return Err(self.galat(
                            "NUSA-L008",
                            "teks tidak ditutup dengan `\"`",
                            mulai,
                            self.i,
                        ));
                    }
                    let e = self.potong();
                    match e {
                        'n' => isi.push('\n'),
                        't' => isi.push('\t'),
                        'r' => isi.push('\r'),
                        '0' => isi.push('\0'),
                        '\\' => isi.push('\\'),
                        '"' => isi.push('"'),
                        'u' => {
                            if !self.ambil('{') {
                                return Err(self.galat(
                                    "NUSA-L009",
                                    "escape `\\u` harus berbentuk `\\u{...}`",
                                    self.i - 2,
                                    self.i,
                                ));
                            }
                            let awal = self.i;
                            while self.i < self.s.len() && self.s.as_bytes()[self.i] != b'}' {
                                self.i += 1;
                            }
                            if self.i >= self.s.len() {
                                return Err(self.galat(
                                    "NUSA-L009",
                                    "escape `\\u{...}` tidak ditutup",
                                    awal,
                                    self.i,
                                ));
                            }
                            let hex = &self.s[awal..self.i];
                            self.i += 1;
                            let kode = u32::from_str_radix(hex, 16).map_err(|_| {
                                self.galat(
                                    "NUSA-L009",
                                    "kode unicode tidak valid",
                                    awal,
                                    self.i - 1,
                                )
                            })?;
                            let ch = char::from_u32(kode).ok_or_else(|| {
                                self.galat(
                                    "NUSA-L009",
                                    "kode unicode di luar jangkauan",
                                    awal,
                                    self.i - 1,
                                )
                            })?;
                            isi.push(ch);
                        }
                        lain => {
                            return Err(self.galat(
                                "NUSA-L010",
                                format!("escape `\\{}` tidak dikenal", lain),
                                self.i - 2,
                                self.i,
                            ))
                        }
                    }
                }
                c => isi.push(c),
            }
        }
        self.tokens.push(Token {
            tok: Tok::Teks(isi),
            span: Span::baru(mulai, self.i),
        });
        self.lanjut = false;
        Ok(())
    }

    fn identifier(&mut self, mulai: usize) -> Hasil<()> {
        while self.i < self.s.len() {
            let c = self.s[self.i..].chars().next().unwrap();
            if c == '_' || c.is_alphanumeric() {
                self.i += c.len_utf8();
            } else {
                break;
            }
        }
        let kata = &self.s[mulai..self.i];
        let tok = match kata {
            "fungsi" | "fng" => Tok::Fungsi,
            "kembalikan" | "kmb" => Tok::Kembalikan,
            "jika" | "jk" => Tok::Jika,
            "lainnya" | "lain" | "ln" => Tok::Lainnya,
            "selama" | "slm" => Tok::Selama,
            "untuk" | "utk" => Tok::Untuk,
            "dalam" | "dlm" => Tok::Dalam,
            "tetap" | "ttp" => Tok::Tetap,
            "var" => Tok::Var,
            "dan" | "dn" => Tok::Dan,
            "atau" | "at" => Tok::Atau,
            "bukan" | "bkn" => Tok::Bukan,
            "benar" | "bn" => Tok::Benar,
            "salah" | "sl" => Tok::Salah,
            "kosong" | "ksg" => Tok::Kosong,
            "bil" | "bilangan" => Tok::TBil,
            "des" | "desimal" => Tok::TDes,
            "log" | "logika" => Tok::TLog,
            "teks" => Tok::TTeks,
            "daftar" | "dft" => Tok::TDaftar,
            "syarat" | "syr" => Tok::Syarat,
            "jamin" | "jmn" => Tok::Jamin,
            "invarian" | "inv" => Tok::Invarian,
            "tegaskan" | "tgs" => Tok::Tegaskan,
            "hentikan" | "htk" => Tok::Hentikan,
            "lanjutkan" | "lnj" => Tok::Lanjutkan,
            _ => Tok::Id(kata.to_string()),
        };
        self.tokens.push(Token {
            tok,
            span: Span::baru(mulai, self.i),
        });
        self.lanjut = false;
        Ok(())
    }

    fn dorong(&mut self, tok: Tok, panjang: usize) {
        self.lanjut = matches!(
            tok,
            Tok::Plus
                | Tok::Minus
                | Tok::Bintang
                | Tok::GarisMiring
                | Tok::Persen
                | Tok::Sama
                | Tok::SamaSama
                | Tok::TidakSama
                | Tok::Kurang
                | Tok::KurangSama
                | Tok::Lebih
                | Tok::LebihSama
                | Tok::PlusSama
                | Tok::MinusSama
                | Tok::BintangSama
                | Tok::GarisMiringSama
                | Tok::PersenSama
                | Tok::DanDan
                | Tok::AtauAtau
                | Tok::Koma
                | Tok::Titik
                | Tok::TitikTitik
                | Tok::TitikTitikSama
                | Tok::Panah
                | Tok::TitikDua
                | Tok::LPar
                | Tok::LBracket
                | Tok::LBrace
                | Tok::Dan
                | Tok::Atau
                | Tok::Bukan
        );
        self.tokens.push(Token {
            tok,
            span: Span::baru(self.i - panjang, self.i),
        });
    }

    fn potong(&mut self) -> char {
        let c = self.s[self.i..].chars().next().unwrap();
        self.i += c.len_utf8();
        c
    }

    fn ambil(&mut self, c: char) -> bool {
        if self.i < self.s.len() && self.s.as_bytes()[self.i] == c as u8 {
            self.i += 1;
            true
        } else {
            false
        }
    }

    fn sisa(&self) -> &'a str {
        &self.s[self.i..]
    }

    fn galat(
        &self,
        kode: &'static str,
        pesan: impl Into<String>,
        mulai: usize,
        akhir: usize,
    ) -> Diag {
        Diag::baru(kode, pesan).di(Span::baru(mulai, akhir))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn jenis(s: &str) -> Vec<Tok> {
        lex(s).unwrap().into_iter().map(|t| t.tok).collect()
    }

    #[test]
    fn kata_kunci_dan_singkatan_setara() {
        assert_eq!(
            jenis("fungsi fng"),
            vec![Tok::Fungsi, Tok::Fungsi, Tok::Akhir]
        );
        assert_eq!(
            jenis("kembalikan kmb"),
            vec![Tok::Kembalikan, Tok::Kembalikan, Tok::Akhir]
        );
        assert_eq!(
            jenis("bilangan bil"),
            vec![Tok::TBil, Tok::TBil, Tok::Akhir]
        );
    }

    #[test]
    fn baris_baru_di_dalam_kurung_diabaikan() {
        let t = jenis("f(1,\n2)\nx");
        assert!(t.contains(&Tok::BaruBaris));
        assert_eq!(t.first(), Some(&Tok::Id("f".into())));
    }

    #[test]
    fn operator_dua_karakter() {
        assert_eq!(
            jenis("== != <= >= .. ..= -> += -= *= /= %="),
            vec![
                Tok::SamaSama,
                Tok::TidakSama,
                Tok::KurangSama,
                Tok::LebihSama,
                Tok::TitikTitik,
                Tok::TitikTitikSama,
                Tok::Panah,
                Tok::PlusSama,
                Tok::MinusSama,
                Tok::BintangSama,
                Tok::GarisMiringSama,
                Tok::PersenSama,
                Tok::Akhir
            ]
        );
    }

    #[test]
    fn angka_des_dan_rentang() {
        assert_eq!(
            jenis("1.5 0..10 1_000"),
            vec![
                Tok::Des(1.5),
                Tok::Bil(0),
                Tok::TitikTitik,
                Tok::Bil(10),
                Tok::Bil(1000),
                Tok::Akhir
            ]
        );
    }

    #[test]
    fn pengenal_unicode() {
        assert_eq!(
            jenis("var π: bil = 3\n"),
            vec![
                Tok::Var,
                Tok::Id("π".into()),
                Tok::TitikDua,
                Tok::TBil,
                Tok::Sama,
                Tok::Bil(3),
                Tok::BaruBaris,
                Tok::Akhir
            ]
        );
    }

    #[test]
    fn teks_dengan_escape() {
        assert_eq!(
            jenis(r#""halo\n\u{1F600}""#),
            vec![Tok::Teks("halo\n😀".into()), Tok::Akhir]
        );
    }
}
