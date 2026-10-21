//! Pacote STFS "CON" do Xbox 360 (o arquivo do save) e a assinatura RSA.
//!
//! O save do RE6 é um pacote CON com um nível de hash e um arquivo dentro,
//! o `savedata.bin`. Ao gravar: os hashes SHA-1 dos blocos, da tabela e do
//! cabeçalho são refeitos e o pacote é assinado de novo (keyvault do console
//! ou a chave interna do programa).

use num_bigint::BigUint;
use sha1::{Digest, Sha1};
use std::fs;
use std::path::Path;

pub type Result<T> = std::result::Result<T, String>;

const VD: usize = 0x379;
const PKG_PERFIL: (usize, usize) = (0x371, 8);
const PKG_DEVICE: (usize, usize) = (0x3FD, 0x14);
const PKG_CONSOLE: (usize, usize) = (0x36C, 5);
const OFF_TIPO: usize = 0x344;
const OFF_TITLE_ID: usize = 0x360;
const OFF_VERSAO_META: usize = 0x348;
const OFF_NOMES: usize = 0x411;
const OFF_JOGO: usize = 0x1691;
const OFF_TAM_MINI: usize = 0x1712;
const OFF_MINI: usize = 0x171A;
const PNG: &[u8] = b"\x89PNG\r\n\x1a\n";

/// Title ID do Resident Evil 6 e o tipo "save de jogo" no cabeçalho.
pub const TITLE_ID_RE6: u32 = 0x43430819;
const TIPO_SAVE: u32 = 1;

const DIGEST_INFO: [u8; 15] = [0x30, 0x21, 0x30, 0x09, 0x06, 0x05, 0x2b, 0x0e, 0x03, 0x02, 0x1a, 0x05, 0x00, 0x04, 0x14];

pub fn sha1(d: &[u8]) -> [u8; 20] {
    Sha1::digest(d).into()
}

pub fn be32(d: &[u8], o: usize) -> u32 {
    u32::from_be_bytes(d[o..o + 4].try_into().unwrap())
}

pub fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02X}")).collect()
}

/// Números do XeCrypt ficam em qwords na ordem inversa.
fn qwrev(b: &[u8]) -> Vec<u8> {
    b.chunks(8).rev().flatten().copied().collect()
}

fn fixo_be(n: &BigUint, len: usize) -> Vec<u8> {
    let b = n.to_bytes_be();
    let mut out = vec![0u8; len.saturating_sub(b.len())];
    out.extend_from_slice(&b);
    out
}

// ---------------------------------------------------------------- assinatura

/// Chave RSA e certificado do console.
pub struct ChaveConsole {
    cert: Vec<u8>,
    n: BigUint,
    d: BigUint,
    pub console: String,
}

impl ChaveConsole {
    /// Lê o keyvault decifrado (kv.bin).
    pub fn carrega(p: &Path) -> Result<Self> {
        let kv = fs::read(p).map_err(|e| format!("keyvault: {e}"))?;
        if kv.len() < 0x4000 {
            return Err("keyvault inválido".into());
        }
        let cert = kv[0x9C8..0x9C8 + 0x1A8].to_vec();
        let k = &kv[0x298..0x298 + 0x1D0];
        let n = qwrev(&k[0x10..0x90]);
        let p_ = qwrev(&k[0x90..0xD0]);
        let q = qwrev(&k[0xD0..0x110]);
        Self::de_componentes(&cert, &n, &p_, &q)
    }

    /// Chave montada a partir das partes (certificado, n, p e q em
    /// big-endian comum). Confere p*q == n e que o módulo do certificado é
    /// o mesmo da chave.
    pub fn de_componentes(cert: &[u8], n: &[u8], p: &[u8], q: &[u8]) -> Result<Self> {
        if cert.len() != 0x1A8 || u16::from_be_bytes([cert[0], cert[1]]) != 0x1A8 {
            return Err("certificado inválido".into());
        }
        let n = BigUint::from_bytes_be(n);
        let (p_, q) = (BigUint::from_bytes_be(p), BigUint::from_bytes_be(q));
        if &p_ * &q != n || BigUint::from_bytes_be(&qwrev(&cert[0x28..0xA8])) != n {
            return Err("chave inválida".into());
        }
        let e = BigUint::from(be32(cert, 0x24));
        let um = BigUint::from(1u32);
        let d = e.modinv(&((&p_ - &um) * (&q - &um))).ok_or("chave inválida")?;
        let console = String::from_utf8_lossy(&cert[7..0x12]).into_owned();
        Ok(Self { cert: cert.to_vec(), n, d, console })
    }

    fn assina(&self, dados: &[u8]) -> Vec<u8> {
        let m = bloco_pkcs1(dados);
        let mut s = fixo_be(&BigUint::from_bytes_be(&m).modpow(&self.d, &self.n), 0x80);
        s.reverse();
        s
    }
}

/// Bloco PKCS#1 v1.5 de 0x80 bytes com o SHA-1 dos dados.
fn bloco_pkcs1(dados: &[u8]) -> Vec<u8> {
    let mut m = vec![0x00, 0x01];
    m.extend(std::iter::repeat_n(0xFF, 0x80 - 3 - DIGEST_INFO.len() - 20));
    m.push(0x00);
    m.extend_from_slice(&DIGEST_INFO);
    m.extend_from_slice(&sha1(dados));
    m
}

/// Confere a assinatura do pacote com a chave pública do próprio certificado.
pub fn assinatura_ok(pkg: &[u8]) -> bool {
    let cert = &pkg[4..4 + 0x1A8];
    let e = BigUint::from(be32(cert, 0x24));
    let n = BigUint::from_bytes_be(&qwrev(&cert[0x28..0xA8]));
    if n == BigUint::from(0u32) {
        return false;
    }
    let mut sig = pkg[0x1AC..0x22C].to_vec();
    sig.reverse();
    let m = fixo_be(&BigUint::from_bytes_be(&sig).modpow(&e, &n), 0x80);
    m == bloco_pkcs1(&pkg[0x22C..0x344])
}

// ---------------------------------------------------------------- metadados

/// O que o cabeçalho do pacote diz sobre o save.
pub struct Metadados {
    pub nome: String,
    pub jogo: String,
    pub profile_id: String,
    pub device_id: String,
    pub console_id: String,
    pub miniatura: Option<Vec<u8>>,
}

fn utf16_fixo(d: &[u8], o: usize) -> String {
    let Some(b) = d.get(o..o + 0x100) else { return String::new() };
    let u: Vec<u16> = b.chunks(2).map(|c| u16::from_be_bytes([c[0], c[1]])).take_while(|&c| c != 0).collect();
    String::from_utf16_lossy(&u).trim().to_string()
}

pub fn metadados(pkg: &[u8]) -> Metadados {
    let versao = be32(pkg, OFF_VERSAO_META);
    let max = if versao >= 2 { 0x3D00 } else { 0x4000 };
    let tam = be32(pkg, OFF_TAM_MINI) as usize;
    let miniatura = (tam > 0 && tam <= max)
        .then(|| pkg.get(OFF_MINI..OFF_MINI + tam))
        .flatten()
        .filter(|b| b.starts_with(PNG))
        .map(<[u8]>::to_vec);
    let ((po, pl), (dof, dl), (co, cl)) = (PKG_PERFIL, PKG_DEVICE, PKG_CONSOLE);
    Metadados {
        nome: (0..9).map(|i| utf16_fixo(pkg, OFF_NOMES + i * 0x100)).find(|s| !s.is_empty()).unwrap_or_default(),
        jogo: utf16_fixo(pkg, OFF_JOGO),
        profile_id: hex(&pkg[po..po + pl]),
        device_id: hex(&pkg[dof..dof + dl]),
        console_id: hex(&pkg[co..co + cl]),
        miniatura,
    }
}

// ---------------------------------------------------------------- STFS

/// Pacote STFS com um nível de hash (saves pequenos).
#[derive(Clone)]
pub struct Stfs {
    pub d: Vec<u8>,
    base: usize,
    shift: u32,
    tabela: usize,
}

impl Stfs {
    /// Abre um save do RE6: pacote CON, Title ID 43430819, tipo save de jogo.
    pub fn novo(d: Vec<u8>) -> Result<Self> {
        if d.len() < 0x1000 || !matches!(&d[..4], b"CON " | b"LIVE" | b"PIRS") {
            return Err("não é um save do Xbox 360 (pacote CON)".into());
        }
        if &d[..4] != b"CON " {
            return Err("este pacote é LIVE/PIRS; o editor só abre saves assinados por console (CON)".into());
        }
        let title = be32(&d, OFF_TITLE_ID);
        if title != TITLE_ID_RE6 {
            return Err(format!("não é um save do Resident Evil 6 (Title ID {title:08X}, esperado {TITLE_ID_RE6:08X})"));
        }
        if be32(&d, OFF_TIPO) != TIPO_SAVE {
            return Err("o pacote é do Resident Evil 6, mas não é um save de jogo".into());
        }
        let tam_cab = be32(&d, 0x340) as usize;
        let sep = d[VD + 2];
        let base = (tam_cab + 0xFFF) & 0xF000;
        let shift = if base == 0xB000 || sep & 1 == 0 { 1 } else { 0 };
        // a tabela de hash ativa é a segunda cópia quando o bit 1 está ligado
        let tabela = base + if sep & 2 != 0 { 0x1000 } else { 0 };
        if be32(&d, VD + 0x1C) >= 0xAA {
            return Err("pacote grande demais (mais de um nível de hash)".into());
        }
        if d.len() < tabela + 0x1000 {
            return Err("pacote truncado".into());
        }
        Ok(Self { d, base, shift, tabela })
    }

    fn off(&self, b: usize) -> usize {
        self.base + ((b + (((b + 0xAA) / 0xAA) << self.shift)) << 12)
    }

    fn entrada(&self, b: usize) -> usize {
        self.tabela + b * 0x18
    }

    fn proximo(&self, b: usize) -> usize {
        let e = self.entrada(b);
        let x = &self.d[e + 0x15..e + 0x18];
        (x[0] as usize) << 16 | (x[1] as usize) << 8 | x[2] as usize
    }

    fn bloco(&self, b: usize) -> Result<&[u8]> {
        if b >= 0xAA {
            return Err("bloco fora da tabela de hash (pacote corrompido)".into());
        }
        let o = self.off(b);
        self.d.get(o..o + 0x1000).ok_or_else(|| "bloco fora do pacote".to_string())
    }

    fn bloco_diretorio(&self) -> usize {
        u32::from_le_bytes([self.d[VD + 5], self.d[VD + 6], self.d[VD + 7], 0]) as usize
    }

    fn blocos_do_arquivo(&self) -> Result<(Vec<usize>, usize)> {
        let ft = self.bloco_diretorio();
        for ent in self.bloco(ft)?.chunks(0x40) {
            let len = (ent[0x28] & 0x3F) as usize;
            if len > 0 && &ent[..len] == b"savedata.bin" {
                let mut b = u32::from_le_bytes([ent[0x2F], ent[0x30], ent[0x31], 0]) as usize;
                let tam = be32(ent, 0x34) as usize;
                let mut blocos = Vec::new();
                for _ in 0..tam.div_ceil(0x1000) {
                    self.bloco(b)?;
                    blocos.push(b);
                    b = self.proximo(b);
                }
                return Ok((blocos, tam));
            }
        }
        Err("savedata.bin não encontrado no pacote".into())
    }

    pub fn le(&self) -> Result<Vec<u8>> {
        let (blocos, tam) = self.blocos_do_arquivo()?;
        let mut out = Vec::with_capacity(blocos.len() * 0x1000);
        for b in blocos {
            out.extend_from_slice(self.bloco(b)?);
        }
        out.truncate(tam);
        Ok(out)
    }

    /// Grava o savedata.bin e refaz os hashes (blocos, diretório, tabela e cabeçalho).
    pub fn grava(&mut self, conteudo: &[u8]) -> Result<()> {
        let (blocos, tam) = self.blocos_do_arquivo()?;
        if conteudo.len() != tam {
            return Err("o tamanho do arquivo interno não pode mudar".into());
        }
        for (i, &b) in blocos.iter().enumerate() {
            let parte = &conteudo[i * 0x1000..((i + 1) * 0x1000).min(tam)];
            let o = self.off(b);
            self.d[o..o + parte.len()].copy_from_slice(parte);
            let h = sha1(&self.d[o..o + 0x1000]);
            let e = self.entrada(b);
            self.d[e..e + 20].copy_from_slice(&h);
        }
        let ft = self.bloco_diretorio();
        let h = sha1(self.bloco(ft)?);
        let e = self.entrada(ft);
        self.d[e..e + 20].copy_from_slice(&h);
        let topo = sha1(&self.d[self.tabela..self.tabela + 0x1000]);
        self.d[VD + 8..VD + 0x1C].copy_from_slice(&topo);
        let cab = sha1(&self.d[0x344..self.base]);
        self.d[0x32C..0x340].copy_from_slice(&cab);
        Ok(())
    }

    pub fn assina(&mut self, k: &ChaveConsole) {
        self.d[4..4 + 0x1A8].copy_from_slice(&k.cert);
        let s = k.assina(&self.d[0x22C..0x344]);
        self.d[0x1AC..0x22C].copy_from_slice(&s);
    }

    pub fn hashes_ok(&self) -> bool {
        let confere = |b: usize| {
            let e = self.entrada(b);
            self.bloco(b).map(|x| sha1(x)[..] == self.d[e..e + 20]).unwrap_or(false)
        };
        let arquivo = matches!(self.blocos_do_arquivo(), Ok((bl, _)) if bl.iter().all(|&b| confere(b)));
        arquivo
            && confere(self.bloco_diretorio())
            && sha1(&self.d[self.tabela..self.tabela + 0x1000])[..] == self.d[VD + 8..VD + 0x1C]
            && sha1(&self.d[0x344..self.base])[..] == self.d[0x32C..0x340]
    }
}

#[cfg(test)]
mod testes {
    use super::*;
    use crate::sistema::chave_interna as c;

    #[test]
    fn chave_interna_assina_e_confere() {
        let k = ChaveConsole::de_componentes(&c::CERT, &c::N, &c::P, &c::Q).unwrap();
        let mut pkg = vec![0u8; 0x1000];
        pkg[..4].copy_from_slice(b"CON ");
        pkg[0x22C..0x344].iter_mut().enumerate().for_each(|(i, b)| *b = i as u8);
        pkg[4..4 + 0x1A8].copy_from_slice(&k.cert);
        let s = k.assina(&pkg[0x22C..0x344]);
        pkg[0x1AC..0x22C].copy_from_slice(&s);
        assert!(assinatura_ok(&pkg));
        pkg[0x300] ^= 1;
        assert!(!assinatura_ok(&pkg));
    }
}
