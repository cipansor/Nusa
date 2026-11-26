//! Pelentik (lexer) bahasa NUSA.

use crate::ast::Span;
use crate::diag::Diagnostik;

#[derive(Clone, Debug, PartialEq)]
pub enum Tok {
    Ident(String),
    Bil(i128),
    Char(u8),
    Pecah(f64),
    Teks(String),

    Fungsi,
    Tetap,
    Ubah,
    Kembali,
    Jika,
    Selain,
    Selama,
    Untuk,
    Dalam,
    Dari,
    Sampai,
    Ulang,
    Henti,
    Lanjut,
    Dan,
    Atau,
    Bukan,
    Benar,
    Salah,
    Struk,
    Enumerasi,
    Cocok,
    Kasus,
    Lainnya,
    Tegas,
    Perlu,
    Jamin,
    Invarian,
    Impor,
    Takaman,
    Sebagai,
    Luar,
    Jenis,

    Plus,
    Minus,
    Bintang,
    GarisMiring,
    Persen,
    SamaSama,
    TidakSama,
    Kecil,
    KecilSama,
    Besar,
    BesarSama,
    Amp,
    TandaSeru,

    LPar,
    RPar,
    LBracket,
    RBracket,
    LBrace,
    RBrace,
    Koma,
    TitikKoma,
    Titik,
    Panah,
    Titik2,
    Titik2Sama,
    TitikDua,
    DuaTitik2,
    Sama,

    Selesai,
}

impl Tok {
    pub fn sebutan(&self) -> String {
        match self {
            Tok::Ident(n) => format!("`{}`", n),
            Tok::Bil(n) => format!("`{}`", n),
            Tok::Char(c) => format!("`'{}'`", *c as char),
            Tok::Pecah(n) => format!("`{}`", n),
            Tok::Teks(_) => "`teks`".into(),
            Tok::Fungsi => "`fungsi`".into(),
            Tok::Tetap => "`tetap`".into(),
            Tok::Ubah => "`ubah`".into(),
            Tok::Kembali => "`kembali`".into(),
            Tok::Jika => "`jika`".into(),
            Tok::Selain => "`selain`".into(),
            Tok::Selama => "`selama`".into(),
            Tok::Untuk => "`untuk`".into(),
            Tok::Dalam => "`dalam`".into(),
            Tok::Dari => "`dari`".into(),
            Tok::Sampai => "`sampai`".into(),
            Tok::Ulang => "`ulang`".into(),
            Tok::Henti => "`henti`".into(),
            Tok::Lanjut => "`lanjut`".into(),
            Tok::Dan => "`dan`".into(),
            Tok::Atau => "`atau`".into(),
            Tok::Bukan => "`bukan`".into(),
            Tok::Benar => "`benar`".into(),
            Tok::Salah => "`salah`".into(),
            Tok::Struk => "`struk`".into(),
            Tok::Enumerasi => "`enumerasi`".into(),
            Tok::Cocok => "`cocok`".into(),
            Tok::Kasus => "`kasus`".into(),
            Tok::Lainnya => "`lainnya`".into(),
            Tok::Tegas => "`tegas`".into(),
            Tok::Perlu => "`perlu`".into(),
            Tok::Jamin => "`jamin`".into(),
            Tok::Invarian => "`invarian`".into(),
            Tok::Impor => "`impor`".into(),
            Tok::Sebagai => "`sebagai`".into(),
            Tok::Luar => "`luar`".into(),
            Tok::Jenis => "`jenis`".into(),
            Tok::Takaman => "`takaman`".into(),
            Tok::Plus => "`+`".into(),
            Tok::Minus => "`-`".into(),
            Tok::Bintang => "`*`".into(),
            Tok::GarisMiring => "`/`".into(),
            Tok::Persen => "`%`".into(),
            Tok::SamaSama => "`==`".into(),
            Tok::TidakSama => "`!=`".into(),
            Tok::Kecil => "`<`".into(),
            Tok::KecilSama => "`<=`".into(),
            Tok::Besar => "`>`".into(),
            Tok::BesarSama => "`>=`".into(),
            Tok::Amp => "`&`".into(),
            Tok::TandaSeru => "`!`".into(),
            Tok::LPar => "`(`".into(),
            Tok::RPar => "`)`".into(),
            Tok::LBracket => "`[`".into(),
            Tok::RBracket => "`]`".into(),
            Tok::LBrace => "`{`".into(),
            Tok::RBrace => "`}`".into(),
            Tok::Koma => "`,`".into(),
            Tok::TitikKoma => "`;`".into(),
            Tok::Titik => "`.`".into(),
            Tok::Panah => "`->`".into(),
            Tok::Titik2 => "`..`".into(),
            Tok::Titik2Sama => "`..=`".into(),
            Tok::TitikDua => "`:`".into(),
            Tok::DuaTitik2 => "`::`".into(),
            Tok::Sama => "`=`".into(),
            Tok::Selesai => "akhir berkas".into(),
        }
    }
}

/// Kata kunci kanonik beserta singkatan Indonesianya.
pub const KATA_KUNCI: &[(&str, Tok)] = &[
    ("fungsi", Tok::Fungsi),
    ("fn", Tok::Fungsi),
    ("fng", Tok::Fungsi),
    ("tetap", Tok::Tetap),
    ("tet", Tok::Tetap),
    ("ttp", Tok::Tetap),
    ("ubah", Tok::Ubah),
    ("ubh", Tok::Ubah),
    ("var", Tok::Ubah),
    ("kembali", Tok::Kembali),
    ("kembalikan", Tok::Kembali),
    ("krb", Tok::Kembali),
    ("jika", Tok::Jika),
    ("jik", Tok::Jika),
    ("selain", Tok::Selain),
    ("sln", Tok::Selain),
    ("lain", Tok::Selain),
    ("selama", Tok::Selama),
    ("slm", Tok::Selama),
    ("untuk", Tok::Untuk),
    ("utk", Tok::Untuk),
    ("dalam", Tok::Dalam),
    ("dlm", Tok::Dalam),
    ("dari", Tok::Dari),
    ("dr", Tok::Dari),
    ("sampai", Tok::Sampai),
    ("sp", Tok::Sampai),
    ("ulang", Tok::Ulang),
    ("ul", Tok::Ulang),
    ("henti", Tok::Henti),
    ("hentikan", Tok::Henti),
    ("hnt", Tok::Henti),
    ("lanjut", Tok::Lanjut),
    ("lanjutkan", Tok::Lanjut),
    ("lnj", Tok::Lanjut),
    ("prk", Tok::Tegas),
    ("dan", Tok::Dan),
    ("dn", Tok::Dan),
    ("atau", Tok::Atau),
    ("at", Tok::Atau),
    ("bukan", Tok::Bukan),
    ("bk", Tok::Bukan),
    ("bkn", Tok::Bukan),
    ("benar", Tok::Benar),
    ("bnr", Tok::Benar),
    ("bn", Tok::Benar),
    ("salah", Tok::Salah),
    ("slh", Tok::Salah),
    ("sl", Tok::Salah),
    ("struk", Tok::Struk),
    ("str", Tok::Struk),
    ("enumerasi", Tok::Enumerasi),
    ("enm", Tok::Enumerasi),
    ("cocok", Tok::Cocok),
    ("cck", Tok::Cocok),
    ("kasus", Tok::Kasus),
    ("kss", Tok::Kasus),
    ("lainnya", Tok::Lainnya),
    ("lny", Tok::Lainnya),
    ("tegas", Tok::Tegas),
    ("tgs", Tok::Tegas),
    ("perlu", Tok::Perlu),
    ("prl", Tok::Perlu),
    ("jamin", Tok::Jamin),
    ("jmn", Tok::Jamin),
    ("invarian", Tok::Invarian),
    ("inv", Tok::Invarian),
    ("impor", Tok::Impor),
    ("imp", Tok::Impor),
    ("takaman", Tok::Takaman),
    ("ta", Tok::Takaman),
    ("unsafe", Tok::Takaman),
    ("sebagai", Tok::Sebagai),
    ("sbg", Tok::Sebagai),
    ("luar", Tok::Luar),
    ("lr", Tok::Luar),
    ("jenis", Tok::Jenis),
    ("jns", Tok::Jenis),
    // Sinonim bergaya PR #1 agar program lama/asing tetap terbaca.
    ("tp", Tok::Tetap),
    ("ub", Tok::Ubah),
    ("kmb", Tok::Kembali),
    ("jk", Tok::Jika),
    ("ln", Tok::Selain),
    ("lainnya", Tok::Selain),
    ("ht", Tok::Henti),
    ("syarat", Tok::Perlu),
    ("syr", Tok::Perlu),
    ("periksa", Tok::Tegas),
    ("bentuk", Tok::Struk),
    ("bt", Tok::Struk),
    ("pilihan", Tok::Enumerasi),
    ("plh", Tok::Enumerasi),
];

pub fn kata_kunci(kata: &str) -> Option<Tok> {
    KATA_KUNCI
        .iter()
        .find(|(k, _)| *k == kata)
        .map(|(_, t)| t.clone())
}

pub struct HasilLentik {
    pub token: Vec<(Tok, Span)>,
    pub diag: Vec<Diagnostik>,
}

pub fn lentik(sumber: &str, nama_berkas: &str) -> HasilLentik {
    let karakter: Vec<char> = sumber.chars().collect();
    let mut i = 0usize;
    let mut baris = 1u32;
    let mut kol = 1u32;
    let mut token = Vec::new();
    let mut diag = Vec::new();

    macro_rules! maju {
        () => {{
            let c = karakter[i];
            i += 1;
            if c == '\n' {
                baris += 1;
                kol = 1;
            } else {
                kol += 1;
            }
            c
        }};
    }
    macro_rules! lihat {
        ($n:expr) => {
            karakter.get(i + $n).copied()
        };
    }

    while i < karakter.len() {
        let c = karakter[i];
        if c.is_whitespace() {
            maju!();
            continue;
        }
        if c == '#' {
            while i < karakter.len() && karakter[i] != '\n' {
                i += 1;
            }
            continue;
        }
        if c == '/' && lihat!(1) == Some('/') {
            while i < karakter.len() && karakter[i] != '\n' {
                maju!();
            }
            continue;
        }
        if c == '/' && lihat!(1) == Some('*') {
            let (b0, k0) = (baris, kol);
            maju!();
            maju!();
            let mut kedalaman = 1;
            while i < karakter.len() && kedalaman > 0 {
                if karakter[i] == '/' && lihat!(1) == Some('*') {
                    maju!();
                    maju!();
                    kedalaman += 1;
                } else if karakter[i] == '*' && lihat!(1) == Some('/') {
                    maju!();
                    maju!();
                    kedalaman -= 1;
                } else {
                    maju!();
                }
            }
            if kedalaman > 0 {
                diag.push(Diagnostik::galat(
                    nama_berkas,
                    Span::baru(b0, k0, 2),
                    "komentar `/*` tidak ditutup dengan `*/`".into(),
                    "E0002",
                ));
            }
            continue;
        }

        let mulai = Span::baru(baris, kol, 1);

        if c.is_ascii_alphabetic() || c == '_' {
            let awal = i;
            while i < karakter.len() && (karakter[i].is_ascii_alphanumeric() || karakter[i] == '_')
            {
                maju!();
            }
            let kata: String = karakter[awal..i].iter().collect();
            let sp = Span::baru(mulai.baris, mulai.kol, (i - awal) as u32);
            match kata_kunci(&kata) {
                Some(t) => token.push((t, sp)),
                None => token.push((Tok::Ident(kata), sp)),
            }
            continue;
        }

        if c.is_ascii_digit() {
            let awal = i;
            let mut basis = 10u32;
            let mut heks = false;
            if c == '0' {
                match lihat!(1) {
                    Some('x') | Some('X') => {
                        basis = 16;
                        heks = true;
                        maju!();
                        maju!();
                        while i < karakter.len()
                            && (karakter[i].is_ascii_hexdigit() || karakter[i] == '_')
                        {
                            maju!();
                        }
                    }
                    Some('b') | Some('B') => {
                        basis = 2;
                        maju!();
                        maju!();
                        while i < karakter.len()
                            && (karakter[i] == '0' || karakter[i] == '1' || karakter[i] == '_')
                        {
                            maju!();
                        }
                    }
                    Some('o') | Some('O') => {
                        basis = 8;
                        maju!();
                        maju!();
                        while i < karakter.len()
                            && (('0'..='7').contains(&karakter[i]) || karakter[i] == '_')
                        {
                            maju!();
                        }
                    }
                    _ => {}
                }
            }
            if !heks {
                while i < karakter.len() && (karakter[i].is_ascii_digit() || karakter[i] == '_') {
                    maju!();
                }
                // pecahan: 1.5 tetapi bukan 1..3
                if lihat!(0) == Some('.') && lihat!(1).map(|c| c.is_ascii_digit()).unwrap_or(false)
                {
                    maju!();
                    while i < karakter.len() && (karakter[i].is_ascii_digit() || karakter[i] == '_')
                    {
                        maju!();
                    }
                    if lihat!(0) == Some('e') || lihat!(0) == Some('E') {
                        maju!();
                        if lihat!(0) == Some('+') || lihat!(0) == Some('-') {
                            maju!();
                        }
                        while i < karakter.len() && karakter[i].is_ascii_digit() {
                            maju!();
                        }
                    }
                    let teks: String = karakter[awal..i].iter().filter(|c| **c != '_').collect();
                    let sp = Span::baru(mulai.baris, mulai.kol, (i - awal) as u32);
                    match teks.parse::<f64>() {
                        Ok(v) => token.push((Tok::Pecah(v), sp)),
                        Err(_) => diag.push(Diagnostik::galat(
                            nama_berkas,
                            sp,
                            format!("angka pecahan `{}` tidak sah", teks),
                            "E0003",
                        )),
                    }
                    continue;
                }
            }
            let mentah: String = karakter[awal..i].iter().filter(|c| **c != '_').collect();
            let berawalan = mentah.len() > 1
                && (mentah.starts_with("0b")
                    || mentah.starts_with("0B")
                    || mentah.starts_with("0o")
                    || mentah.starts_with("0O"));
            let digit = if heks || berawalan {
                &mentah[2..]
            } else {
                &mentah[..]
            };
            let sp = Span::baru(mulai.baris, mulai.kol, (i - awal) as u32);
            match i128::from_str_radix(digit, basis) {
                Ok(v) => token.push((Tok::Bil(v), sp)),
                Err(_) => diag.push(Diagnostik::galat(
                    nama_berkas,
                    sp,
                    format!("angka bulat `{}` di luar jangkauan bil (i64)", mentah),
                    "E0004",
                )),
            }
            continue;
        }

        if c == '\'' {
            maju!();
            if i >= karakter.len() {
                diag.push(Diagnostik::galat(
                    nama_berkas,
                    mulai,
                    "huruf tidak ditutup dengan `'`".into(),
                    "E0008",
                ));
                continue;
            }
            let nilaich = if karakter[i] == '\\' {
                maju!();
                if i >= karakter.len() {
                    continue;
                }
                match maju!() {
                    'n' => b'\n',
                    't' => b'\t',
                    'r' => b'\r',
                    '0' => 0,
                    '\\' => b'\\',
                    '\'' => b'\'',
                    '"' => b'"',
                    lain => {
                        diag.push(Diagnostik::galat(
                            nama_berkas,
                            mulai,
                            format!("escape `\\{}` tidak dikenal", lain),
                            "E0006",
                        ));
                        lain as u8
                    }
                }
            } else {
                let ch = maju!();
                if ch.len_utf8() > 1 {
                    diag.push(Diagnostik::galat(
                        nama_berkas,
                        mulai,
                        "huruf harus satu byte ASCII".into(),
                        "E0009",
                    ));
                }
                ch as u8
            };
            if i < karakter.len() && karakter[i] == '\'' {
                maju!();
            } else {
                diag.push(Diagnostik::galat(
                    nama_berkas,
                    mulai,
                    "huruf tidak ditutup dengan `'`".into(),
                    "E0008",
                ));
            }
            token.push((Tok::Char(nilaich), Span::baru(mulai.baris, mulai.kol, 3)));
            continue;
        }

        if c == '"' {
            maju!();
            let mut isi = String::new();
            let awal_kol = mulai.kol;
            let mut tutup = false;
            while i < karakter.len() {
                let ch = karakter[i];
                if ch == '"' {
                    maju!();
                    tutup = true;
                    break;
                }
                if ch == '\n' {
                    break;
                }
                if ch == '\\' {
                    maju!();
                    if i >= karakter.len() {
                        break;
                    }
                    let esc = maju!();
                    match esc {
                        'n' => isi.push('\n'),
                        't' => isi.push('\t'),
                        'r' => isi.push('\r'),
                        '0' => isi.push('\0'),
                        '\\' => isi.push('\\'),
                        '"' => isi.push('"'),
                        'u' => {
                            if lihat!(0) == Some('{') {
                                maju!();
                                let mut hex = String::new();
                                while i < karakter.len() && karakter[i] != '}' {
                                    hex.push(maju!());
                                }
                                if i < karakter.len() {
                                    maju!();
                                }
                                match u32::from_str_radix(&hex, 16).ok().and_then(char::from_u32) {
                                    Some(ch2) => isi.push(ch2),
                                    None => diag.push(Diagnostik::galat(
                                        nama_berkas,
                                        Span::baru(baris, kol, 1),
                                        format!("escape `\\u{{{}}}` tidak sah", hex),
                                        "E0005",
                                    )),
                                }
                            }
                        }
                        lain => {
                            diag.push(Diagnostik::galat(
                                nama_berkas,
                                Span::baru(baris, kol, 1),
                                format!("escape `\\{}` tidak dikenal", lain),
                                "E0006",
                            ));
                            isi.push(lain);
                        }
                    }
                    continue;
                }
                isi.push(maju!());
            }
            if !tutup {
                diag.push(Diagnostik::galat(
                    nama_berkas,
                    Span::baru(mulai.baris, awal_kol, 1),
                    "teks tidak ditutup dengan `\"`".into(),
                    "E0007",
                ));
            }
            let panjang = kol.saturating_sub(awal_kol);
            token.push((Tok::Teks(isi), Span::baru(mulai.baris, awal_kol, panjang)));
            continue;
        }

        // operator / tanda baca
        let (tok, lebar) = match (c, lihat!(1), lihat!(2)) {
            ('-', Some('>'), _) => (Tok::Panah, 2),
            ('.', Some('.'), Some('=')) => (Tok::Titik2Sama, 3),
            ('.', Some('.'), _) => (Tok::Titik2, 2),
            (':', Some(':'), _) => (Tok::DuaTitik2, 2),
            ('=', Some('='), _) => (Tok::SamaSama, 2),
            ('!', Some('='), _) => (Tok::TidakSama, 2),
            ('<', Some('='), _) => (Tok::KecilSama, 2),
            ('>', Some('='), _) => (Tok::BesarSama, 2),
            ('&', Some('&'), _) => (Tok::Dan, 2),
            ('|', Some('|'), _) => (Tok::Atau, 2),
            ('+', _, _) => (Tok::Plus, 1),
            ('-', _, _) => (Tok::Minus, 1),
            ('*', _, _) => (Tok::Bintang, 1),
            ('/', _, _) => (Tok::GarisMiring, 1),
            ('%', _, _) => (Tok::Persen, 1),
            ('<', _, _) => (Tok::Kecil, 1),
            ('>', _, _) => (Tok::Besar, 1),
            ('&', _, _) => (Tok::Amp, 1),
            ('!', _, _) => (Tok::TandaSeru, 1),
            ('(', _, _) => (Tok::LPar, 1),
            (')', _, _) => (Tok::RPar, 1),
            ('[', _, _) => (Tok::LBracket, 1),
            (']', _, _) => (Tok::RBracket, 1),
            ('{', _, _) => (Tok::LBrace, 1),
            ('}', _, _) => (Tok::RBrace, 1),
            (',', _, _) => (Tok::Koma, 1),
            (';', _, _) => (Tok::TitikKoma, 1),
            ('.', _, _) => (Tok::Titik, 1),
            (':', _, _) => (Tok::TitikDua, 1),
            ('=', _, _) => (Tok::Sama, 1),
            _ => {
                diag.push(Diagnostik::galat(
                    nama_berkas,
                    mulai,
                    format!("karakter `{}` tidak dikenal", c),
                    "E0001",
                ));
                maju!();
                continue;
            }
        };
        for _ in 0..lebar {
            maju!();
        }
        token.push((tok, Span::baru(mulai.baris, mulai.kol, lebar)));
    }

    token.push((Tok::Selesai, Span::baru(baris, kol, 1)));
    HasilLentik { token, diag }
}
