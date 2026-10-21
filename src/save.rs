//! Save do Resident Evil 6: Xbox 360 e PC (Steam).
//!
//! As duas plataformas guardam o mesmo `savedata.bin`, sem criptografia; o
//! que muda é o envelope, a ordem dos bytes e alguns deslocamentos.
//!
//! * Xbox 360: pacote CON (pacote.rs) com o savedata.bin de 24576 bytes
//!   dentro, big-endian. Identificador 0x12071800.
//! * PC (Steam): o próprio arquivo é o savedata.bin (39416 bytes nos saves
//!   conhecidos), little-endian. Identificador 0x12071801. De 0x0C a 0x1F há
//!   uma marca do computador ou da conta, que fica fora da verificação e que
//!   o editor nunca mexe.
//!
//! Campos (Xbox / PC):
//! * 0x04 u32 tamanho do arquivo
//! * 0x08 verificação: soma dos u32 de 0x0C / 0x18 até o fim (módulo 2^32)
//! * 0x10 / 0x20 data e hora do save: seis u32 (ano, mês, dia, hora, minuto, segundo)
//! * 0x6F8 / 0x70C pontos de habilidade (u32; o jogo vai até 9.999.999).
//!   Confirmado no console: 800 no início e 2150 depois do capítulo 1 do Leon.
//! * 0x700 / 0x714 habilidades: um byte por número de habilidade (0 a 96); o
//!   0 não é usado e a habilidade N fica em +N. 03 = comprada, 01 = à venda.
//!   Conferido no save novo do console: as compráveis do começo (01) são as
//!   1, 4, 7, 10, 13 e 18, os níveis 1 de Armas de fogo, Corpo a corpo,
//!   Defesa, Crítico, Perfuração e Caçador de zumbis.
//! * 0x4AF4 / 0x84EC inventário: 8 blocos de 0x70 bytes, um por personagem
//!   (Leon, Helena, Chris, Piers, Jake, Sherry, Ada, Agente). Cada bloco tem
//!   16 bytes de cabeçalho (u8 personagem, u8, u16 vida, u16 vida máxima…)
//!   e 24 espaços de 4 bytes: u16 item (tipo << 8 | número) e u16 quantidade.
//!
//! Espaços (conferido em save do próprio console): 0-5 armas do personagem
//! (quantidade = balas no pente; FFFE = ainda não obtida; FFFF = obtida sem
//! contagem), 7 comprimidos, 8 spray, 9-10 granadas, 15 em diante a maleta.
//! item 0000 e quantidade FFFE = espaço vazio; FFFF = espaço fechado.

use crate::pacote::{self, Stfs};
use std::path::{Path, PathBuf};

pub type Result<T> = std::result::Result<T, String>;

pub const HABILIDADES: usize = 96;
/// Valor do byte de uma habilidade comprada.
pub const COMPRADA: u8 = 3;
/// Valor que o editor grava ao desmarcar uma habilidade que já vinha comprada.
pub const NAO_COMPRADA: u8 = 0;
/// Máximo de pontos de habilidade que o jogo mostra.
pub const PONTOS_MAX: u32 = 9_999_999;
/// Maior arquivo aceito ao abrir (o save do Xbox tem 80 KB e o do PC, 39 KB):
/// evita ler um arquivo enorme inteiro na memória por engano.
const TAM_MAX_ARQUIVO: u64 = 4 * 1024 * 1024;
pub const BLOCO: usize = 0x70;
pub const SLOTS: usize = 24;
const OFF_VERIFICACAO: usize = 0x08;

pub const PERSONAGENS: [&str; 8] = ["Leon", "Helena", "Chris", "Piers", "Jake", "Sherry", "Ada", "Agente"];

/// Quantidade que marca arma ainda não obtida.
pub const NAO_OBTIDA: u16 = 0xFFFE;
/// Quantidade sem contagem (arma corpo a corpo, espaço fechado).
pub const SEM_CONTAGEM: u16 = 0xFFFF;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Plataforma {
    Xbox,
    Pc,
}

/// Onde fica cada campo em cada plataforma.
#[derive(Clone, Copy, Debug)]
struct Mapa {
    magico: u32,
    soma: usize,
    data: usize,
    pontos: usize,
    /// Byte da habilidade 0 (não usada): a habilidade N fica em `habilidades + N`.
    habilidades: usize,
    inv: usize,
    big_endian: bool,
}

const XBOX: Mapa = Mapa { magico: 0x1207_1800, soma: 0x0C, data: 0x10, pontos: 0x6F8, habilidades: 0x700, inv: 0x4AF4, big_endian: true };
const PC: Mapa = Mapa { magico: 0x1207_1801, soma: 0x18, data: 0x20, pontos: 0x70C, habilidades: 0x714, inv: 0x84EC, big_endian: false };
/// Tamanho do savedata.bin do Xbox.
const TAMANHO_XBOX: usize = 0x6000;

impl Mapa {
    fn de(p: Plataforma) -> Self {
        match p {
            Plataforma::Xbox => XBOX,
            Plataforma::Pc => PC,
        }
    }

    /// Fim do último campo que o editor lê.
    fn fim(&self) -> usize {
        self.inv + PERSONAGENS.len() * BLOCO
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Slot {
    /// Byte do tipo como está no save (o nibble alto é uma marca do jogo).
    pub tipo: u8,
    pub numero: u8,
    pub qtd: u16,
}

impl Slot {
    pub const VAZIO: Slot = Slot { tipo: 0, numero: 0, qtd: NAO_OBTIDA };

    /// Tipo sem a marca: 1 arma/item, 2 munição, 3 erva.
    pub fn classe(&self) -> u8 {
        self.tipo & 0x0F
    }

    pub fn vazio(&self) -> bool {
        self.tipo == 0 && self.numero == 0 && self.qtd == NAO_OBTIDA
    }

    pub fn fechado(&self) -> bool {
        self.tipo == 0 && self.numero == 0 && self.qtd == SEM_CONTAGEM
    }
}

/// Onde fica cada espaço do bloco, para a tela.
pub fn grupo_do_slot(s: usize) -> &'static str {
    match s {
        0..=5 => "Armas",
        6..=14 => "Itens",
        _ => "Maleta",
    }
}

#[derive(Clone, PartialEq, Debug)]
pub struct Personagem {
    pub vida: u16,
    pub vida_max: u16,
    pub slots: [Slot; SLOTS],
}

/// O savedata.bin.
#[derive(Clone)]
pub struct Interno {
    pub d: Vec<u8>,
    pub plataforma: Plataforma,
    mapa: Mapa,
}

impl Interno {
    pub fn novo(d: Vec<u8>, plataforma: Plataforma) -> Result<Self> {
        let mapa = Mapa::de(plataforma);
        let erro = || -> String {
            match plataforma {
                Plataforma::Xbox => "o savedata.bin de dentro do pacote não tem o formato do Resident Evil 6".into(),
                Plataforma::Pc => "o arquivo não tem o formato do save do Resident Evil 6 de PC".into(),
            }
        };
        if d.len() < mapa.fim() || !d.len().is_multiple_of(4) {
            return Err(erro());
        }
        let i = Self { d, plataforma, mapa };
        let tamanho_ok = match plataforma {
            Plataforma::Xbox => i.d.len() == TAMANHO_XBOX,
            Plataforma::Pc => i.u32(4) as usize == i.d.len(),
        };
        if i.u32(0) != mapa.magico || !tamanho_ok {
            return Err(erro());
        }
        Ok(i)
    }

    fn u16(&self, o: usize) -> u16 {
        let b = [self.d[o], self.d[o + 1]];
        if self.mapa.big_endian { u16::from_be_bytes(b) } else { u16::from_le_bytes(b) }
    }

    fn u32(&self, o: usize) -> u32 {
        let b: [u8; 4] = self.d[o..o + 4].try_into().unwrap();
        if self.mapa.big_endian { u32::from_be_bytes(b) } else { u32::from_le_bytes(b) }
    }

    fn grava16(&mut self, o: usize, v: u16) {
        let b = if self.mapa.big_endian { v.to_be_bytes() } else { v.to_le_bytes() };
        self.d[o..o + 2].copy_from_slice(&b);
    }

    fn grava32(&mut self, o: usize, v: u32) {
        let b = if self.mapa.big_endian { v.to_be_bytes() } else { v.to_le_bytes() };
        self.d[o..o + 4].copy_from_slice(&b);
    }

    pub fn verificacao(&self) -> u32 {
        (self.mapa.soma..self.d.len()).step_by(4).fold(0u32, |s, o| s.wrapping_add(self.u32(o)))
    }

    pub fn verificacao_salva(&self) -> u32 {
        self.u32(OFF_VERIFICACAO)
    }

    pub fn fecha_verificacao(&mut self) {
        let v = self.verificacao();
        self.grava32(OFF_VERIFICACAO, v);
    }

    /// Data e hora do save, como o jogo gravou.
    pub fn data(&self) -> String {
        let v: Vec<u32> = (0..6).map(|i| self.u32(self.mapa.data + i * 4)).collect();
        format!("{:02}/{:02}/{:04} {:02}:{:02}:{:02}", v[2], v[1], v[0], v[3], v[4], v[5])
    }

    pub fn pontos(&self) -> u32 {
        self.u32(self.mapa.pontos)
    }

    /// Grava o valor como veio: quem limita a 9.999.999 é a tela, para um
    /// save que já tenha outro valor continuar igual se não for mexido.
    pub fn grava_pontos(&mut self, v: u32) {
        self.grava32(self.mapa.pontos, v);
    }

    /// As 96 habilidades, na ordem dos nomes do jogo (a primeira é a 1).
    pub fn habilidades(&self) -> [u8; HABILIDADES] {
        let o = self.mapa.habilidades + 1;
        let mut h = [0; HABILIDADES];
        h.copy_from_slice(&self.d[o..o + HABILIDADES]);
        h
    }

    pub fn grava_habilidades(&mut self, h: &[u8; HABILIDADES]) {
        let o = self.mapa.habilidades + 1;
        self.d[o..o + HABILIDADES].copy_from_slice(h);
    }

    pub fn personagem(&self, p: usize) -> Personagem {
        let b = self.mapa.inv + p * BLOCO;
        let mut slots = [Slot::VAZIO; SLOTS];
        for (s, slot) in slots.iter_mut().enumerate() {
            let o = b + 16 + s * 4;
            let [tipo, numero] = self.u16(o).to_be_bytes();
            *slot = Slot { tipo, numero, qtd: self.u16(o + 2) };
        }
        Personagem { vida: self.u16(b + 2), vida_max: self.u16(b + 4), slots }
    }

    pub fn grava_personagem(&mut self, p: usize, x: &Personagem) {
        let b = self.mapa.inv + p * BLOCO;
        self.grava16(b + 2, x.vida);
        for (s, slot) in x.slots.iter().enumerate() {
            let o = b + 16 + s * 4;
            self.grava16(o, u16::from_be_bytes([slot.tipo, slot.numero]));
            self.grava16(o + 2, slot.qtd);
        }
    }
}

/// O que o editor muda no save.
#[derive(Clone, PartialEq, Debug)]
pub struct Edicao {
    pub pontos: u32,
    pub habilidades: [u8; HABILIDADES],
    pub personagens: Vec<Personagem>,
}

/// O save aberto. No Xbox, o pacote e o savedata.bin de dentro; no PC, só o
/// savedata.bin.
pub struct Save {
    pub caminho: PathBuf,
    pub pacote: Option<Stfs>,
    pub interno: Interno,
}

/// Separa o save pelos primeiros bytes: pacote do Xbox ou savedata.bin de PC.
fn le_bytes(d: Vec<u8>) -> Result<(Option<Stfs>, Interno)> {
    if d.len() >= 4 && matches!(&d[..4], b"CON " | b"LIVE" | b"PIRS") {
        let pacote = Stfs::novo(d)?;
        let interno = Interno::novo(pacote.le()?, Plataforma::Xbox)?;
        Ok((Some(pacote), interno))
    } else if d.len() >= 4 && d[..4] == PC.magico.to_le_bytes() {
        Ok((None, Interno::novo(d, Plataforma::Pc)?))
    } else {
        Err("não é um save do Resident Evil 6 (nem pacote do Xbox 360 nem savedata.bin de PC)".into())
    }
}

impl Save {
    pub fn abre(caminho: &Path) -> Result<Self> {
        let tam = std::fs::metadata(caminho).map_err(|e| format!("não consegui ler o arquivo: {e}"))?.len();
        if tam > TAM_MAX_ARQUIVO {
            return Err("arquivo grande demais para ser um save do Resident Evil 6".into());
        }
        let d = std::fs::read(caminho).map_err(|e| format!("não consegui ler o arquivo: {e}"))?;
        let (pacote, interno) = le_bytes(d)?;
        Ok(Self { caminho: caminho.to_path_buf(), pacote, interno })
    }

    /// Tudo o que o editor muda, como está no arquivo.
    pub fn edicao(&self) -> Edicao {
        Edicao {
            pontos: self.interno.pontos(),
            habilidades: self.interno.habilidades(),
            personagens: (0..PERSONAGENS.len()).map(|p| self.interno.personagem(p)).collect(),
        }
    }

    pub fn plataforma(&self) -> Plataforma {
        self.interno.plataforma
    }

    /// Número do personagem gravado no cabeçalho do bloco (para conferência).
    #[cfg(test)]
    fn d_bloco(&self, p: usize) -> u8 {
        self.interno.d[self.interno.mapa.inv + p * BLOCO]
    }

    /// Aplica a edição e fecha a verificação. No Xbox, também refaz os hashes
    /// do pacote e assina; no PC, o arquivo é o próprio savedata.bin.
    pub fn monta(&self, ed: &Edicao, chave: &pacote::ChaveConsole) -> Result<Vec<u8>> {
        let pers = &ed.personagens;
        let mut interno = self.interno.clone();
        interno.grava_pontos(ed.pontos);
        interno.grava_habilidades(&ed.habilidades);
        for (p, x) in pers.iter().enumerate().take(PERSONAGENS.len()) {
            interno.grava_personagem(p, x);
        }
        interno.fecha_verificacao();
        let Some(pacote) = &self.pacote else { return Ok(interno.d) };
        let mut pkg = pacote.clone();
        pkg.grava(&interno.d)?;
        pkg.assina(chave);
        Ok(pkg.d)
    }

    #[cfg(test)]
    fn abre_bytes(b: &[u8]) -> Self {
        let (pacote, interno) = le_bytes(b.to_vec()).unwrap();
        Self { caminho: PathBuf::new(), pacote, interno }
    }

    /// Relê os bytes gravados e confere tudo: a verificação e, no Xbox, os
    /// hashes e a assinatura do pacote.
    pub fn confere(bytes: &[u8]) -> bool {
        let Ok((pacote, i)) = le_bytes(bytes.to_vec()) else { return false };
        let pacote_ok = pacote.is_none_or(|pkg| pkg.hashes_ok() && pacote::assinatura_ok(&pkg.d));
        i.verificacao() == i.verificacao_salva() && pacote_ok
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    fn xbox_vazio() -> Vec<u8> {
        let mut d = vec![0u8; TAMANHO_XBOX];
        d[..4].copy_from_slice(&XBOX.magico.to_be_bytes());
        d
    }

    fn pc_vazio() -> Vec<u8> {
        let n = 39416;
        let mut d = vec![0u8; n];
        d[..4].copy_from_slice(&PC.magico.to_le_bytes());
        d[4..8].copy_from_slice(&(n as u32).to_le_bytes());
        d
    }

    #[test]
    fn verificacao_xbox_soma_do_0x0c_ao_fim() {
        let mut d = xbox_vazio();
        d[0x10..0x14].copy_from_slice(&5u32.to_be_bytes());
        d[TAMANHO_XBOX - 4..].copy_from_slice(&0xFFFF_FFFFu32.to_be_bytes());
        let mut i = Interno::novo(d, Plataforma::Xbox).unwrap();
        i.fecha_verificacao();
        assert_eq!(i.verificacao_salva(), 4);
    }

    #[test]
    fn verificacao_pc_soma_do_0x18_ao_fim_sem_a_marca() {
        let mut d = pc_vazio();
        let n = d.len();
        d[0x10..0x18].copy_from_slice(&[0x17, 0x30, 1, 0xCC, 0x41, 0x52, 0x81, 0xA0]);
        d[0x18..0x1C].copy_from_slice(&5u32.to_le_bytes());
        d[n - 4..].copy_from_slice(&0xFFFF_FFFFu32.to_le_bytes());
        let mut i = Interno::novo(d, Plataforma::Pc).unwrap();
        i.fecha_verificacao();
        assert_eq!(i.verificacao_salva(), 4);
        assert_eq!(&i.d[8..12], &[4, 0, 0, 0]);
    }

    #[test]
    fn habilidade_n_fica_no_byte_n() {
        for (d, p, base) in [(xbox_vazio(), Plataforma::Xbox, 0x700), (pc_vazio(), Plataforma::Pc, 0x714)] {
            let mut i = Interno::novo(d, p).unwrap();
            let mut h = [0u8; HABILIDADES];
            h[0] = 1; // habilidade 1
            h[HABILIDADES - 1] = 3; // habilidade 96
            i.grava_habilidades(&h);
            assert_eq!(i.d[base], 0, "{p:?}: o byte da habilidade 0 não muda");
            assert_eq!(i.d[base + 1], 1, "{p:?}");
            assert_eq!(i.d[base + 96], 3, "{p:?}");
            assert_eq!(i.habilidades(), h);
        }
    }

    #[test]
    fn personagem_ida_e_volta_xbox() {
        let mut i = Interno::novo(xbox_vazio(), Plataforma::Xbox).unwrap();
        let mut p = i.personagem(0);
        p.vida = 750;
        p.slots[5] = Slot { tipo: 1, numero: 7, qtd: 5 };
        i.grava_personagem(0, &p);
        assert_eq!(&i.d[XBOX.inv + 2..XBOX.inv + 4], &[0x02, 0xEE]);
        assert_eq!(&i.d[XBOX.inv + 16 + 20..XBOX.inv + 16 + 24], &[1, 7, 0, 5]);
        assert_eq!(i.personagem(0), p);
    }

    #[test]
    fn personagem_ida_e_volta_pc() {
        let mut i = Interno::novo(pc_vazio(), Plataforma::Pc).unwrap();
        let mut p = i.personagem(7);
        p.vida = 750;
        p.slots[5] = Slot { tipo: 1, numero: 7, qtd: 0x0105 };
        i.grava_personagem(7, &p);
        let b = PC.inv + 7 * BLOCO;
        assert_eq!(&i.d[b + 2..b + 4], &[0xEE, 0x02]);
        assert_eq!(&i.d[b + 16 + 20..b + 16 + 24], &[7, 1, 5, 1]);
        assert_eq!(i.personagem(7), p);
    }

    #[test]
    fn pc_ida_e_volta_sem_assinar() {
        let mut d = pc_vazio();
        d[0x10..0x18].copy_from_slice(&[9; 8]);
        let mut i = Interno::novo(d, Plataforma::Pc).unwrap();
        i.fecha_verificacao();
        let s = Save::abre_bytes(&i.d);
        assert_eq!(s.plataforma(), Plataforma::Pc);
        assert!(s.pacote.is_none());
        let mut ed = s.edicao();
        ed.pontos = 123_456;
        let b = s.monta(&ed, &crate::sistema::pastas::chave().unwrap()).unwrap();
        assert_eq!(b.len(), i.d.len());
        assert!(Save::confere(&b));
        assert_eq!(&b[0x10..0x18], &[9; 8], "a marca do computador fica intacta");
        assert_eq!(&b[PC.pontos..PC.pontos + 4], &123_456u32.to_le_bytes());
        assert_eq!(Save::abre_bytes(&b).edicao(), ed);
    }

    #[test]
    fn recusa_o_que_nao_e_save() {
        assert!(le_bytes(vec![0; 64]).is_err());
        // tamanho no cabeçalho diferente do arquivo
        let mut d = pc_vazio();
        d[4] ^= 4;
        assert!(le_bytes(d).is_err());
        // curto demais para o inventário
        let mut d = pc_vazio();
        d.truncate(PC.inv);
        let n = d.len() as u32;
        d[4..8].copy_from_slice(&n.to_le_bytes());
        assert!(le_bytes(d).is_err());
        // save do Xbox sem o pacote não passa como PC
        assert!(le_bytes(xbox_vazio()).is_err());
        let mut d = pc_vazio();
        d[0x100] = 1;
        assert!(!Save::confere(&d), "verificação errada");
    }
}

/// Com o save do teste de habilidades aceito no console (RE6_SAVE_HAB_ANTES
/// e RE6_SAVE_HAB_TESTE): todas as habilidades compradas pelo editor têm que
/// dar o mesmo savedata.bin do teste. Ficam de fora o que o teste mudou à
/// mão (0x980 e a verificação) e as pontas: o teste gravou 03 de 0x700 a
/// 0x75F, e as habilidades vão de 0x701 (a 1) a 0x760 (a 96).
#[cfg(test)]
mod teste_habilidades {
    use super::*;

    #[test]
    fn todas_compradas_igual_ao_teste_do_console() {
        let (Ok(a), Ok(t)) = (std::env::var("RE6_SAVE_HAB_ANTES"), std::env::var("RE6_SAVE_HAB_TESTE")) else { return };
        let s = Save::abre(Path::new(&a)).unwrap();
        let mut ed = s.edicao();
        ed.habilidades = [COMPRADA; HABILIDADES];
        let b = s.monta(&ed, &crate::sistema::pastas::chave().unwrap()).unwrap();
        assert!(Save::confere(&b));
        let nosso = Save::abre_bytes(&b).interno.d;
        let teste = Save::abre(Path::new(&t)).unwrap().interno.d;
        let fora = |i: usize| (OFF_VERIFICACAO..OFF_VERIFICACAO + 4).contains(&i) || (0x980..0x984).contains(&i) || i == 0x700 || i == 0x760;
        let dif: Vec<usize> = (0..TAMANHO_XBOX).filter(|&i| !fora(i) && nosso[i] != teste[i]).collect();
        assert!(dif.is_empty(), "difere do teste em {dif:x?}");
    }

    /// Save do console depois do capítulo 1 do Leon (RE6_SAVE_CAP1): o jogo
    /// mostrou 2150 pontos e as habilidades do teste continuaram compradas.
    #[test]
    fn pontos_do_capitulo_1() {
        let Ok(c) = std::env::var("RE6_SAVE_CAP1") else { return };
        let s = Save::abre(Path::new(&c)).unwrap();
        assert_eq!(s.edicao().pontos, 2150);
        // o teste antigo gravou 03 de 0x700 a 0x75F: as habilidades 1 a 95
        let h = s.edicao().habilidades;
        assert_eq!(h[..HABILIDADES - 1], [COMPRADA; HABILIDADES - 1]);
        assert_eq!(s.interno.d[0x700], COMPRADA);
        assert_eq!(h[HABILIDADES - 1], 0, "a 96 o teste não alcançou");
    }
}

/// Com um save real (variável RE6_SAVE_TESTE): a mesma edição do teste
/// aceito no console (Magnum, 9 comprimidos e munição na maleta) tem que
/// sair igual ao arquivo de referência (RE6_SAVE_REFERENCIA), se houver.
#[cfg(test)]
mod teste_real {
    use super::*;

    #[test]
    fn edicao_igual_a_aceita_no_console() {
        let Ok(c) = std::env::var("RE6_SAVE_TESTE") else { return };
        let s = Save::abre(Path::new(&c)).unwrap();
        let mut ed = s.edicao();
        let p = &mut ed.personagens;
        p[0].slots[5] = Slot { tipo: 1, numero: 7, qtd: 5 };
        p[0].slots[7].qtd = 9;
        p[0].slots[18] = Slot { tipo: 2, numero: 6, qtd: 10 };
        let chave = crate::sistema::pastas::chave().unwrap();
        let b = s.monta(&ed, &chave).unwrap();
        assert!(Save::confere(&b));
        if let Ok(r) = std::env::var("RE6_SAVE_REFERENCIA") {
            assert!(std::fs::read(r).unwrap() == b, "difere da referência");
        }
        // sem mudança nenhuma: só a assinatura pode mudar
        let mesmo = s.monta(&s.edicao(), &chave).unwrap();
        assert_eq!(Save::abre_bytes(&mesmo).edicao(), s.edicao());
        assert!(Save::confere(&mesmo));
    }
}

/// Com saves reais de PC (RE6_SAVE_PC, caminhos separados por ";"): abrir e
/// gravar sem mudança devolve o arquivo idêntico, e uma edição muda só os
/// bytes do campo e a verificação.
#[cfg(test)]
mod teste_pc_real {
    use super::*;

    #[test]
    fn pc_real_ida_e_volta() {
        let Ok(lista) = std::env::var("RE6_SAVE_PC") else { return };
        let chave = crate::sistema::pastas::chave().unwrap();
        for c in lista.split(';').filter(|c| !c.is_empty()) {
            let original = std::fs::read(c).unwrap();
            let s = Save::abre(Path::new(c)).unwrap();
            assert_eq!(s.plataforma(), Plataforma::Pc);
            assert!(Save::confere(&original), "{c}: verificação do original");
            let mesmo = s.monta(&s.edicao(), &chave).unwrap();
            assert!(mesmo == original, "{c}: sem mudança tem que sair idêntico");
            for p in 0..PERSONAGENS.len() {
                assert_eq!(s.d_bloco(p), p as u8, "{c}: bloco {p}");
            }
            let mut ed = s.edicao();
            ed.pontos = 4321;
            ed.habilidades[27] = COMPRADA;
            ed.personagens[0].slots[7].qtd = 9;
            let b = s.monta(&ed, &chave).unwrap();
            assert!(Save::confere(&b));
            let dif: Vec<usize> = (0..b.len()).filter(|&i| b[i] != original[i]).collect();
            let permitido = |i: usize| {
                (8..12).contains(&i)
                    || (PC.pontos..PC.pontos + 4).contains(&i)
                    || i == PC.habilidades + 28
                    || (PC.inv + 16 + 7 * 4 + 2..PC.inv + 16 + 7 * 4 + 4).contains(&i)
            };
            assert!(dif.iter().all(|&i| permitido(i)), "{c}: mudou fora do campo: {dif:x?}");
            assert_eq!(Save::abre_bytes(&b).edicao(), ed);
        }
    }
}
