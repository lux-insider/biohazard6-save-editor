// BIOHAZARD 6 SAVE EDITOR — interface em Slint.
//
//   pacote.rs    pacote CON do Xbox 360: hashes e assinatura
//   save.rs      o savedata.bin do RE6 (Xbox 360 e PC): verificação e campos
//   itens.rs     catálogo dos itens (dados/itens.json)
//   historia.rs  os 80 arquivos da história (dados/historia.json)
//   habilidades.rs as 96 habilidades (dados/habilidades.json)
//   sistema/     pastas, chave de assinatura e diálogos de arquivo
//   interface.rs controlador: liga a interface (ui/*.slint) ao save
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
#![deny(unsafe_code)]

mod habilidades;
mod historia;
mod interface;
mod itens;
mod pacote;
mod save;
mod sistema;

slint::include_modules!();

fn main() -> Result<(), slint::PlatformError> {
    let ui = JanelaPrincipal::new()?;
    let inicial = std::env::args().skip(1).find(|a| !a.starts_with('-')).map(std::path::PathBuf::from).filter(|p| p.is_file());
    interface::iniciar(&ui, inicial);
    // --inventario: abre direto na aba Inventário
    if std::env::args().any(|a| a == "--inventario") {
        ui.global::<Ponte>().set_aba(1);
    }
    // --habilidades / --arquivos: abre direto nessas abas
    if std::env::args().any(|a| a == "--habilidades") {
        ui.global::<Ponte>().set_aba(2);
    }
    if std::env::args().any(|a| a == "--arquivos") {
        ui.global::<Ponte>().set_aba(3);
    }
    // Só no modo de desenvolvimento: escolhe personagem e espaço (para conferir a tela).
    #[cfg(debug_assertions)]
    if let Ok(v) = std::env::var("RE6_DEV_ESPACO") {
        let mut it = v.split(',').filter_map(|x| x.parse::<i32>().ok());
        let (p, s) = (it.next().unwrap_or(0), it.next().unwrap_or(0));
        ui.global::<Ponte>().invoke_escolher_personagem(p);
        ui.global::<Ponte>().invoke_escolher_slot(s);
        ui.global::<Ponte>().invoke_arquivo_escolher(s);
    }
    ui.run()
}
