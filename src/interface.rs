//! Controlador: liga a interface Slint (ui/*.slint) ao save.
//!
//! A tela só mostra as propriedades do `Ponte` e chama os callbacks dele.
//! Aqui fica o estado da edição (os 8 personagens) e as regras de cada
//! espaço do inventário.

use crate::habilidades::{self, Habilidade, MODOS};
use crate::historia::{self, Arquivo, CAMPANHAS};
use crate::itens::{Itens, CLASSES};
use crate::pacote;
use crate::save::{grupo_do_slot, Edicao, Plataforma, Save, Slot, COMPRADA, HABILIDADES, NAO_COMPRADA, NAO_OBTIDA, PERSONAGENS, PONTOS_MAX, SEM_CONTAGEM, SLOTS};
use crate::sistema::{dialogos, pastas};
use crate::{ArquivoLinha, HabilidadeInfo, ItemOpcao, JanelaPrincipal, PersonagemInfo, Ponte, SlotInfo};
use slint::{ComponentHandle, ModelRc, SharedString, VecModel};
use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::Duration;

/// Quantidade acima disso ganha um aviso (o jogo dá bem menos).
const QTD_ALTA: u16 = 99;
/// Maior quantidade aceita (FFFE e FFFF são marcas do jogo).
const QTD_MAX: u16 = 999;

/// Máximo que o jogo aceita de um item (tirado do executável do jogo).
/// Acima disso o jogo zera o item ou trava o uso. Armas e munição ficam de
/// fora: com 999 o jogo funciona normalmente.
pub fn limite_item(classe: u8, numero: u8) -> Option<u16> {
    match (classe, numero) {
        (1, 0x13..=0x16) => Some(5), // granadas e bomba remota
        (1, 0x1D) => Some(12),       // comprimido de cura
        (1, 0x20) => Some(1),        // spray de primeiros socorros
        (3, _) => Some(1),           // ervas: uma por espaço
        _ => None,
    }
}

/// Armas corpo a corpo: não têm pente (o jogo grava quantidade 0).
pub fn corpo_a_corpo(numero: u8) -> bool {
    matches!(numero, 0x00 | 0x18 | 0x19 | 0x1A)
}

/// Ação que espera confirmação (há alterações não gravadas).
enum Pendente {
    Abrir,
    Recarregar,
    Sair,
}

struct Estado {
    itens: Itens,
    habilidades: Vec<Habilidade>,
    /// Modo mostrado na aba Habilidades (0 campanha, 1 Mercenários, 2 Caça ao Agente).
    modo_hab: u8,
    save: Option<Save>,
    original: Edicao,
    atual: Edicao,
    personagem: usize,
    slot: Option<usize>,
    /// Itens oferecidos no editor (tipo, número), na ordem da lista.
    opcoes: Vec<(u8, u8)>,
    classe: u8,
    pendente: Option<Pendente>,
    avisos: u64,
}

type Ctx = Rc<RefCell<Estado>>;

pub fn iniciar(ui: &JanelaPrincipal, inicial: Option<PathBuf>) {
    let ctx: Ctx = Rc::new(RefCell::new(Estado {
        itens: Itens::carrega(),
        habilidades: habilidades::carrega(),
        modo_hab: 0,
        save: None,
        original: Edicao { pontos: 0, habilidades: [0; HABILIDADES], personagens: Vec::new() },
        atual: Edicao { pontos: 0, habilidades: [0; HABILIDADES], personagens: Vec::new() },
        personagem: 0,
        slot: None,
        opcoes: Vec::new(),
        classe: 1,
        pendente: None,
        avisos: 0,
    }));
    let p = ui.global::<Ponte>();
    p.set_titulo(concat!("BIOHAZARD 6 SAVE EDITOR ", env!("CARGO_PKG_VERSION")).into());
    p.set_classes(ModelRc::new(VecModel::from(CLASSES.iter().map(|c| SharedString::from(*c)).collect::<Vec<_>>())));
    p.set_habilidade_modos(ModelRc::new(VecModel::from(MODOS.iter().map(|c| SharedString::from(*c)).collect::<Vec<_>>())));

    // Aba Arquivos: a lista é fixa; o texto só vai para a tela quando escolhido.
    let arquivos = historia::carrega();
    let linhas = linhas_arquivos(&arquivos);
    p.set_arquivos(ModelRc::new(VecModel::from(linhas.iter().map(|(l, _)| l.clone()).collect::<Vec<_>>())));
    // começa no primeiro arquivo (a linha 0 é o cabeçalho da campanha do Leon)
    mostrar_arquivo(ui, &arquivos, 1, 0);
    {
        let fraca = ui.as_weak();
        p.on_arquivo_escolher(move |i| {
            let Some(ui) = fraca.upgrade() else { return };
            let Some(&(_, Some(n))) = usize::try_from(i).ok().and_then(|i| linhas.get(i)) else { return };
            mostrar_arquivo(&ui, &arquivos, i, n);
        });
    }

    macro_rules! liga {
        ($metodo:ident, |$ui:ident, $ctx:ident $(, $arg:ident : $tipo:ty)*| $corpo:expr) => {{
            let fraca = ui.as_weak();
            let ctx = ctx.clone();
            p.$metodo(move |$($arg: $tipo),*| {
                let Some($ui) = fraca.upgrade() else { return };
                let $ctx = &ctx;
                $corpo
            });
        }};
    }

    liga!(on_abrir, |ui, ctx| pedir(&ui, ctx, Pendente::Abrir));
    liga!(on_recarregar, |ui, ctx| pedir(&ui, ctx, Pendente::Recarregar));
    liga!(on_sair, |ui, ctx| pedir(&ui, ctx, Pendente::Sair));
    liga!(on_salvar, |ui, ctx| salvar(&ui, ctx, false));
    liga!(on_salvar_como, |ui, ctx| salvar(&ui, ctx, true));
    liga!(on_desfazer, |ui, ctx| {
        {
            let mut e = ctx.borrow_mut();
            e.atual = e.original.clone();
        }
        mostrar(&ui, ctx);
        aviso(&ui, ctx, "Alterações desfeitas.", 0);
    });
    liga!(on_escolher_personagem, |ui, ctx, i: i32| {
        {
            let mut e = ctx.borrow_mut();
            e.personagem = (i.max(0) as usize).min(PERSONAGENS.len() - 1);
            e.slot = None;
        }
        mostrar(&ui, ctx);
    });
    liga!(on_escolher_slot, |ui, ctx, i: i32| {
        {
            let mut e = ctx.borrow_mut();
            e.slot = (0..SLOTS as i32).contains(&i).then_some(i as usize);
            let s = e.slot.map(|s| e.atual.personagens[e.personagem].slots[s]);
            if let Some(s) = s.filter(|s| !s.vazio()) {
                e.classe = s.classe().clamp(1, 3);
            }
        }
        mostrar(&ui, ctx);
    });
    liga!(on_vida_editada, |ui, ctx, t: SharedString| {
        let mut e = ctx.borrow_mut();
        let p = e.personagem;
        let max = e.atual.personagens[p].vida_max;
        let ok = match t.trim().parse::<u16>() {
            Ok(v) if v >= 1 && v <= max => {
                e.atual.personagens[p].vida = v;
                true
            }
            _ => false,
        };
        drop(e);
        ui.global::<Ponte>().set_vida_invalida(!ok);
        mostrar_sem_campos(&ui, ctx);
    });
    liga!(on_pontos_editados, |ui, ctx, t: SharedString| {
        let limpo: String = t.chars().filter(|c| !matches!(c, '.' | ' ')).collect();
        let v = limpo.parse::<u32>().ok().filter(|v| *v <= PONTOS_MAX);
        if let Some(v) = v {
            ctx.borrow_mut().atual.pontos = v;
        }
        ui.global::<Ponte>().set_pontos_invalido(v.is_none());
        mostrar_sem_campos(&ui, ctx);
    });
    liga!(on_trocar_classe, |ui, ctx, i: i32| {
        ctx.borrow_mut().classe = (i.clamp(0, 2) + 1) as u8;
        mostrar(&ui, ctx);
    });
    liga!(on_trocar_item, |ui, ctx, i: i32| {
        mudar_slot(ctx, |s, e| {
            if let Some(&(tipo, numero)) = e.opcoes.get(i as usize) {
                let novo = s.vazio() || s.classe() != tipo || s.numero != numero;
                if novo {
                    let qtd = if tipo == 1 && corpo_a_corpo(numero) { 0 } else { 1 };
                    *s = Slot { tipo, numero, qtd };
                }
            }
        });
        mostrar(&ui, ctx);
    });
    liga!(on_qtd_editada, |ui, ctx, t: SharedString| {
        let limite = {
            let e = ctx.borrow();
            e.slot.and_then(|s| {
                let s = e.atual.personagens[e.personagem].slots[s];
                limite_item(s.classe(), s.numero)
            })
        };
        let max = limite.unwrap_or(QTD_MAX);
        let v = t.trim().parse::<u16>().ok().filter(|v| *v <= max);
        if let Some(v) = v {
            mudar_slot(ctx, |s, _| {
                if !s.vazio() && !s.fechado() {
                    s.qtd = v;
                }
            });
        }
        let p = ui.global::<Ponte>();
        p.set_editor_qtd_invalida(v.is_none());
        p.set_editor_qtd_aviso(match v {
            None if limite.is_some() => format!("O jogo aceita no máximo {max}; acima disso ele zera o item.").into(),
            None => format!("Use um número de 0 a {QTD_MAX}.").into(),
            Some(v) if v > QTD_ALTA => "Bem acima do que o jogo costuma dar; confira no jogo.".into(),
            _ => SharedString::new(),
        });
        mostrar_sem_campos(&ui, ctx);
    });
    liga!(on_marcar_obtida, |ui, ctx, sim: bool| {
        let original = {
            let e = ctx.borrow();
            e.slot.map(|s| e.original.personagens[e.personagem].slots[s])
        };
        mudar_slot(ctx, |s, _| {
            if !sim {
                s.qtd = NAO_OBTIDA;
            } else if let Some(o) = original.filter(|o| o.numero == s.numero && o.qtd != NAO_OBTIDA) {
                s.qtd = o.qtd;
            } else {
                s.qtd = 0;
            }
        });
        mostrar(&ui, ctx);
    });
    liga!(on_esvaziar, |ui, ctx| {
        mudar_slot(ctx, |s, _| *s = Slot::VAZIO);
        mostrar(&ui, ctx);
    });
    liga!(on_habilidade_alternar, |ui, ctx, i: i32| {
        {
            let mut e = ctx.borrow_mut();
            let Some(i) = usize::try_from(i).ok().filter(|&i| i < HABILIDADES) else { return };
            e.atual.habilidades[i] = if e.atual.habilidades[i] == COMPRADA {
                // desmarcar: volta ao que estava no save, ou "não comprada"
                let o = e.original.habilidades[i];
                if o == COMPRADA { NAO_COMPRADA } else { o }
            } else {
                COMPRADA
            };
        }
        mostrar_sem_campos(&ui, ctx);
    });
    liga!(on_habilidade_escolher_modo, |ui, ctx, m: i32| {
        ctx.borrow_mut().modo_hab = m.clamp(0, MODOS.len() as i32 - 1) as u8;
        mostrar_sem_campos(&ui, ctx);
    });
    liga!(on_habilidade_comprar_todas, |ui, ctx| {
        {
            let mut e = ctx.borrow_mut();
            let modo = e.modo_hab;
            let quais: Vec<usize> = (0..HABILIDADES).filter(|&i| e.habilidades[i].modo == modo).collect();
            for i in quais {
                e.atual.habilidades[i] = COMPRADA;
            }
        }
        mostrar_sem_campos(&ui, ctx);
        aviso(&ui, ctx, "Todas as habilidades deste modo marcadas como compradas. Salve o arquivo para gravar.", 0);
    });
    liga!(on_habilidade_municao_infinita, |ui, ctx| {
        {
            let mut e = ctx.borrow_mut();
            let quais: Vec<usize> = (0..HABILIDADES).filter(|&i| e.habilidades[i].infinita).collect();
            for i in quais {
                e.atual.habilidades[i] = COMPRADA;
            }
        }
        mostrar_sem_campos(&ui, ctx);
        aviso(
            &ui,
            ctx,
            "As 8 habilidades de munição infinita foram marcadas. No jogo, equipe a da arma num conjunto e ative \"Permitir munição infinita\" antes de começar a campanha.",
            0,
        );
    });
    liga!(on_fechar_aviso, |ui, _ctx| ui.global::<Ponte>().set_aviso_visivel(false));
    liga!(on_confirma_resposta, |ui, ctx, sim: bool| {
        ui.global::<Ponte>().set_confirma_visivel(false);
        let pendente = ctx.borrow_mut().pendente.take();
        if let (true, Some(acao)) = (sim, pendente) {
            executar(&ui, ctx, acao);
        }
    });

    if let Some(c) = inicial {
        abrir_caminho(ui, &ctx, &c);
    }
}

/// Linhas da lista de arquivos: um cabeçalho por campanha e os 20 arquivos
/// dela. Junto, o índice do arquivo (nenhum nos cabeçalhos).
fn linhas_arquivos(arquivos: &[Arquivo]) -> Vec<(ArquivoLinha, Option<usize>)> {
    let mut v = Vec::with_capacity(arquivos.len() + CAMPANHAS.len());
    for (c, nome) in CAMPANHAS.iter().enumerate() {
        v.push((ArquivoLinha { titulo: format!("CAMPANHA DE {}", nome.to_uppercase()).into(), cabecalho: true, campanha: c as i32 }, None));
        for (n, a) in arquivos.iter().enumerate().filter(|(_, a)| a.campanha == *nome) {
            v.push((ArquivoLinha { titulo: a.titulo.as_str().into(), cabecalho: false, campanha: c as i32 }, Some(n)));
        }
    }
    v
}

fn mostrar_arquivo(ui: &JanelaPrincipal, arquivos: &[Arquivo], linha: i32, n: usize) {
    let Some(a) = arquivos.get(n) else { return };
    let c = CAMPANHAS.iter().position(|c| *c == a.campanha).unwrap_or(0);
    let p = ui.global::<Ponte>();
    p.set_arquivo_sel(linha);
    p.set_arquivo_titulo(a.titulo.as_str().into());
    p.set_arquivo_subtitulo(format!("Campanha de {} · arquivo {} de 20", a.campanha, n % 20 + 1).into());
    p.set_arquivo_campanha(c as i32);
    p.set_arquivo_paragrafos(ModelRc::new(VecModel::from(a.paragrafos.iter().map(|t| SharedString::from(t.as_str())).collect::<Vec<_>>())));
}

/// Muda o espaço escolhido do personagem atual.
fn mudar_slot(ctx: &Ctx, f: impl FnOnce(&mut Slot, &Estado)) {
    let mut e = ctx.borrow_mut();
    let Some(s) = e.slot else { return };
    let p = e.personagem;
    let mut slot = e.atual.personagens[p].slots[s];
    f(&mut slot, &e);
    e.atual.personagens[p].slots[s] = slot;
}

fn sujo(e: &Estado) -> bool {
    e.save.is_some() && e.atual != e.original
}

// ---------------------------------------------------------------- abrir

fn pedir(ui: &JanelaPrincipal, ctx: &Ctx, acao: Pendente) {
    if !sujo(&ctx.borrow()) {
        executar(ui, ctx, acao);
        return;
    }
    let p = ui.global::<Ponte>();
    p.set_confirma_titulo("Alterações não salvas".into());
    p.set_confirma_texto("Há alterações que ainda não foram gravadas no arquivo. Continuar e perder essas alterações?".into());
    p.set_confirma_sim("CONTINUAR".into());
    p.set_confirma_visivel(true);
    ctx.borrow_mut().pendente = Some(acao);
}

fn executar(ui: &JanelaPrincipal, ctx: &Ctx, acao: Pendente) {
    match acao {
        Pendente::Sair => {
            let _ = slint::quit_event_loop();
        }
        Pendente::Recarregar => {
            let caminho = ctx.borrow().save.as_ref().map(|s| s.caminho.clone());
            if let Some(c) = caminho {
                abrir_caminho(ui, ctx, &c);
            }
        }
        Pendente::Abrir => {
            let atual = ctx.borrow().save.as_ref().map(|s| s.caminho.clone());
            if let Some(c) = dialogos::abrir("Abrir save do Resident Evil 6", &pastas::pasta_inicial(atual.as_deref())) {
                abrir_caminho(ui, ctx, &c);
            }
        }
    }
}

fn abrir_caminho(ui: &JanelaPrincipal, ctx: &Ctx, caminho: &Path) {
    let s = match Save::abre(caminho) {
        Ok(s) => s,
        Err(e) => {
            aviso(ui, ctx, &format!("Não foi possível abrir: {e}"), 2);
            return;
        }
    };
    let p = ui.global::<Ponte>();
    p.set_caminho(caminho.display().to_string().into());
    p.set_data(s.interno.data().into());
    p.set_verificacao_ok(s.interno.verificacao() == s.interno.verificacao_salva());
    p.set_pc(s.plataforma() == Plataforma::Pc);
    let mini = match &s.pacote {
        Some(pkg) => {
            let m = pacote::metadados(&pkg.d);
            p.set_nome_save(if m.nome.is_empty() { "Save".into() } else { m.nome.into() });
            p.set_jogo(m.jogo.into());
            p.set_profile_id(m.profile_id.into());
            p.set_device_id(m.device_id.into());
            p.set_console_id(m.console_id.into());
            p.set_hashes_ok(pkg.hashes_ok());
            p.set_assinatura_ok(pacote::assinatura_ok(&pkg.d));
            m.miniatura.as_deref().and_then(imagem_png)
        }
        None => {
            p.set_nome_save("Save de PC (Steam)".into());
            p.set_jogo("Resident Evil 6 · PC".into());
            for f in [Ponte::set_profile_id, Ponte::set_device_id, Ponte::set_console_id] {
                f(&p, "".into());
            }
            p.set_hashes_ok(true);
            p.set_assinatura_ok(true);
            None
        }
    };
    p.set_tem_miniatura(mini.is_some());
    p.set_miniatura(mini.unwrap_or_default());
    p.set_chave(pastas::descricao_chave().into());
    {
        let mut e = ctx.borrow_mut();
        e.original = s.edicao();
        e.atual = e.original.clone();
        e.save = Some(s);
        e.slot = None;
        e.personagem = e.personagem.min(PERSONAGENS.len() - 1);
    }
    p.set_aberto(true);
    p.set_aviso_visivel(false);
    mostrar(ui, ctx);
}

/// Lado máximo da miniatura do pacote (a do Xbox tem 64 px).
const MINIATURA_MAX: u32 = 256;

/// Miniatura PNG do pacote para a tela. Recusa imagens grandes demais,
/// para um arquivo estranho não fazer o programa reservar memória à toa.
fn imagem_png(b: &[u8]) -> Option<slint::Image> {
    let mut dec = png::Decoder::new_with_limits(std::io::Cursor::new(b), png::Limits { bytes: 1 << 20 });
    dec.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    let mut r = dec.read_info().ok()?;
    if r.info().width > MINIATURA_MAX || r.info().height > MINIATURA_MAX {
        return None;
    }
    let mut buf = vec![0; r.output_buffer_size()?];
    let info = r.next_frame(&mut buf).ok()?;
    let (w, h) = (info.width, info.height);
    if info.bit_depth != png::BitDepth::Eight {
        return None;
    }
    let mut px = slint::SharedPixelBuffer::<slint::Rgba8Pixel>::new(w, h);
    let dst = px.make_mut_bytes();
    let n = (w * h) as usize;
    match info.color_type {
        png::ColorType::Rgba => dst.copy_from_slice(buf.get(..n * 4)?),
        png::ColorType::Rgb => {
            for (d, o) in dst.as_chunks_mut::<4>().0.iter_mut().zip(buf.get(..n * 3)?.as_chunks::<3>().0) {
                d[..3].copy_from_slice(o);
                d[3] = 255;
            }
        }
        _ => return None,
    }
    Some(slint::Image::from_rgba8(px))
}

// ---------------------------------------------------------------- mostrar

/// Atualiza tudo, inclusive os campos de texto do editor.
fn mostrar(ui: &JanelaPrincipal, ctx: &Ctx) {
    mostrar_sem_campos(ui, ctx);
    let e = ctx.borrow();
    let p = ui.global::<Ponte>();
    p.set_pontos(e.atual.pontos.to_string().into());
    p.set_pontos_invalido(false);
    let pers = &e.atual.personagens[e.personagem];
    p.set_vida(pers.vida.to_string().into());
    p.set_vida_invalida(false);
    p.set_editor_qtd_invalida(false);
    p.set_editor_qtd_aviso(SharedString::new());
    if let Some(s) = e.slot {
        let slot = pers.slots[s];
        p.set_editor_qtd(if slot.qtd >= NAO_OBTIDA { SharedString::new() } else { slot.qtd.to_string().into() });
        if let Some(max) = limite_item(slot.classe(), slot.numero).filter(|m| slot.qtd < NAO_OBTIDA && slot.qtd > *m) {
            p.set_editor_qtd_aviso(format!("Acima do máximo do jogo ({max}): ele zera o item. Ajuste para {max} ou menos.").into());
        } else if slot.qtd < NAO_OBTIDA && slot.qtd > QTD_ALTA {
            p.set_editor_qtd_aviso("Bem acima do que o jogo costuma dar; confira no jogo.".into());
        }
    }
}

/// Atualiza a tela sem mexer nos campos de texto (para não atrapalhar quem digita).
fn mostrar_sem_campos(ui: &JanelaPrincipal, ctx: &Ctx) {
    let mut e = ctx.borrow_mut();
    let p = ui.global::<Ponte>();
    if e.save.is_none() {
        return;
    }
    let lista: Vec<PersonagemInfo> = e
        .atual
        .personagens
        .iter()
        .enumerate()
        .map(|(i, x)| PersonagemInfo {
            nome: PERSONAGENS[i].into(),
            vida: x.vida as i32,
            vida_max: x.vida_max as i32,
            armas: x.slots[..6].iter().filter(|s| s.classe() == 1 && s.qtd != NAO_OBTIDA).count() as i32,
        })
        .collect();
    p.set_personagens(ModelRc::new(VecModel::from(lista)));
    p.set_personagem(e.personagem as i32);
    p.set_pontos_exibido(milhar(e.atual.pontos).into());

    let pers = e.atual.personagens[e.personagem].clone();
    let orig = e.original.personagens[e.personagem].clone();
    let slots: Vec<SlotInfo> = (0..SLOTS).map(|s| slot_info(&e.itens, &pers.slots[s], pers.slots[s] != orig.slots[s])).collect();
    p.set_slots(ModelRc::new(VecModel::from(slots)));
    p.set_slot(e.slot.map_or(-1, |s| s as i32));

    // Habilidades do modo escolhido
    let modo = e.modo_hab;
    let hab: Vec<HabilidadeInfo> = e
        .habilidades
        .iter()
        .enumerate()
        .filter(|(_, h)| h.modo == modo)
        .map(|(i, h)| HabilidadeInfo {
            indice: i as i32,
            nome: h.nome.as_str().into(),
            descricao: h.descricao.as_str().into(),
            comprada: e.atual.habilidades[i] == COMPRADA,
            alterada: e.atual.habilidades[i] != e.original.habilidades[i],
            infinita: h.infinita,
        })
        .collect();
    let compradas = hab.iter().filter(|h| h.comprada).count();
    p.set_habilidade_contagem(format!("{compradas} de {} compradas", hab.len()).into());
    p.set_habilidades(ModelRc::new(VecModel::from(hab)));
    p.set_habilidade_modo(modo as i32);
    p.set_infinitas_todas(e.habilidades.iter().enumerate().filter(|(_, h)| h.infinita).all(|(i, _)| e.atual.habilidades[i] == COMPRADA));

    // Editor do espaço escolhido
    if let Some(s) = e.slot {
        let slot = pers.slots[s];
        let arma = s <= 5 && slot.classe() == 1 && !slot.vazio();
        // Armas (0-5), comprimidos (7) e spray (8) são fixos do personagem.
        let fixo = !slot.vazio() && (s <= 5 || s == 7 || s == 8);
        p.set_editor_titulo(format!("{} · ESPAÇO {}", grupo_do_slot(s).to_uppercase(), s + 1).into());
        p.set_editor_dica(
            if arma {
                "Arma do personagem. Desmarque para deixar como ainda não obtida."
            } else if fixo {
                "Item fixo deste espaço: só a quantidade muda."
            } else {
                "Escolha o tipo e o item; depois a quantidade."
            }
            .into(),
        );
        p.set_editor_arma(arma);
        p.set_editor_obtida(slot.qtd != NAO_OBTIDA);
        p.set_editor_item_livre(!fixo);
        let melee = slot.classe() == 1 && corpo_a_corpo(slot.numero);
        p.set_editor_tem_qtd(!melee && !slot.vazio() && !slot.fechado() && slot.qtd != SEM_CONTAGEM && slot.qtd != NAO_OBTIDA);

        let classe = e.classe;
        let opcoes: Vec<(u8, u8)> = e.itens.da_classe(classe).iter().map(|i| (i.tipo, i.numero)).collect();
        let modelo: Vec<ItemOpcao> = opcoes
            .iter()
            .map(|&(t, n)| {
                let i = e.itens.busca(t, n).unwrap();
                ItemOpcao { nome: i.nome.clone().into(), icone: i.icone }
            })
            .collect();
        p.set_editor_classe(classe as i32 - 1);
        p.set_editor_item(opcoes.iter().position(|&(t, n)| !slot.vazio() && t == slot.classe() && n == slot.numero).map_or(-1, |i| i as i32));
        p.set_opcoes(ModelRc::new(VecModel::from(modelo)));
        e.opcoes = opcoes;
    }
    p.set_sujo(sujo(&e));
}

/// 9999999 -> "9.999.999"
fn milhar(v: u32) -> String {
    let d = v.to_string();
    let mut s = String::with_capacity(d.len() + d.len() / 3);
    for (i, c) in d.chars().enumerate() {
        if i > 0 && (d.len() - i).is_multiple_of(3) {
            s.push('.');
        }
        s.push(c);
    }
    s
}

fn slot_info(itens: &Itens, s: &Slot, alterado: bool) -> SlotInfo {
    let item = itens.busca(s.classe(), s.numero);
    let nome = if s.fechado() {
        "Fechado".to_string()
    } else if s.vazio() {
        "Vazio".to_string()
    } else {
        item.map(|i| i.nome.clone()).unwrap_or_else(|| format!("Item {:02X} {:02X}", s.tipo, s.numero))
    };
    SlotInfo {
        grupo: "".into(),
        icone: if s.vazio() || s.fechado() { -1 } else { item.map_or(-1, |i| i.icone) },
        nome: nome.into(),
        qtd: if s.vazio() || s.fechado() || s.qtd >= NAO_OBTIDA || (s.classe() == 1 && corpo_a_corpo(s.numero)) { SharedString::new() } else { s.qtd.to_string().into() },
        vazio: s.vazio(),
        fechado: s.fechado(),
        nao_obtida: !s.vazio() && s.qtd == NAO_OBTIDA,
        alterado,
    }
}

// ---------------------------------------------------------------- salvar

fn salvar(ui: &JanelaPrincipal, ctx: &Ctx, como: bool) {
    let p = ui.global::<Ponte>();
    if p.get_vida_invalida() || p.get_editor_qtd_invalida() || p.get_pontos_invalido() {
        aviso(ui, ctx, "Corrija o campo marcado em vermelho antes de salvar.", 2);
        return;
    }
    let resultado = {
        let e = ctx.borrow();
        let Some(s) = &e.save else { return };
        if !como && !sujo(&e) {
            return;
        }
        (s.plataforma() == Plataforma::Pc, gravar(s, &e.atual, como))
    };
    let (pc, resultado) = resultado;
    match resultado {
        Ok(None) => {}
        Ok(Some((destino, backup))) => {
            abrir_caminho(ui, ctx, &destino);
            let mut texto = if pc {
                format!("Save gravado e conferido.\n{}", destino.display())
            } else {
                format!("Save gravado, assinado com {} e conferido.\n{}", pastas::descricao_chave(), destino.display())
            };
            if let Some(b) = backup {
                texto.push_str(&format!("\nBackup: {}", b.display()));
            }
            aviso(ui, ctx, &texto, 1);
        }
        Err(e) => aviso(ui, ctx, &format!("Erro ao salvar: {e}"), 2),
    }
}

/// Monta e assina, confere os bytes, faz o backup e grava.
fn gravar(s: &Save, ed: &Edicao, como: bool) -> Result<Option<(PathBuf, Option<PathBuf>)>, String> {
    let chave = pastas::chave()?;
    let bytes = s.monta(ed, &chave)?;
    if !Save::confere(&bytes) {
        return Err("a conferência do arquivo montado falhou; nada foi gravado".into());
    }
    let destino = if como {
        let nome = s.caminho.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or("savedata.bin".into());
        match dialogos::salvar("Salvar save como…", &pastas::pasta_inicial(Some(&s.caminho)), &nome) {
            Some(p) => p,
            None => return Ok(None),
        }
    } else {
        s.caminho.clone()
    };
    let mut backup = None;
    if destino.is_file() {
        let pasta = pastas::pasta_backups()?;
        let nome = destino.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        let agora = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
        let b = pasta.join(format!("{nome}.{agora}.bak"));
        std::fs::copy(&destino, &b).map_err(|e| format!("backup: {e}"))?;
        backup = Some(b);
    }
    // Grava num arquivo temporário ao lado, confere e só então troca: se algo
    // falhar no meio, o save antigo continua inteiro.
    let mut temp = destino.clone().into_os_string();
    temp.push(".gravando");
    let temp = PathBuf::from(temp);
    let feito = (|| {
        std::fs::write(&temp, &bytes).map_err(|e| format!("não consegui gravar: {e}"))?;
        let relido = std::fs::read(&temp).map_err(|e| e.to_string())?;
        if relido != bytes {
            return Err("o arquivo gravado não confere com o montado".to_string());
        }
        std::fs::rename(&temp, &destino).map_err(|e| format!("não consegui substituir o arquivo: {e}"))
    })();
    if feito.is_err() {
        let _ = std::fs::remove_file(&temp);
    }
    feito?;
    Ok(Some((destino, backup)))
}

// ---------------------------------------------------------------- avisos

/// tipo: 0 informação, 1 sucesso, 2 erro. Some sozinho depois de alguns segundos.
fn aviso(ui: &JanelaPrincipal, ctx: &Ctx, texto: &str, tipo: i32) {
    let p = ui.global::<Ponte>();
    p.set_aviso(texto.into());
    p.set_aviso_tipo(tipo);
    p.set_aviso_visivel(true);
    let n = {
        let mut e = ctx.borrow_mut();
        e.avisos += 1;
        e.avisos
    };
    let fraca = ui.as_weak();
    let ctx = ctx.clone();
    slint::Timer::single_shot(Duration::from_secs(if tipo == 2 { 12 } else { 7 }), move || {
        if ctx.borrow().avisos == n {
            if let Some(ui) = fraca.upgrade() {
                ui.global::<Ponte>().set_aviso_visivel(false);
            }
        }
    });
}

#[cfg(test)]
mod testes {
    use super::{limite_item, milhar};

    #[test]
    fn separa_milhar() {
        assert_eq!(milhar(0), "0");
        assert_eq!(milhar(800), "800");
        assert_eq!(milhar(1000), "1.000");
        assert_eq!(milhar(222390), "222.390");
        assert_eq!(milhar(9999999), "9.999.999");
    }

    #[test]
    fn limites_dos_itens() {
        assert_eq!(limite_item(1, 0x13), Some(5)); // granada
        assert_eq!(limite_item(1, 0x16), Some(5)); // bomba remota
        assert_eq!(limite_item(1, 0x1D), Some(12)); // comprimido
        assert_eq!(limite_item(1, 0x20), Some(1)); // spray
        assert_eq!(limite_item(3, 0x03), Some(1)); // erva verde
        // Armas e munição aceitam 999 no jogo.
        assert_eq!(limite_item(1, 0x07), None);
        assert_eq!(limite_item(2, 0x01), None);
    }
}
