//! Onde ficam o keyvault, os backups e por onde os diálogos começam.
use crate::pacote::ChaveConsole;
use std::path::{Path, PathBuf};

pub fn home() -> PathBuf {
    std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE")).map(PathBuf::from).unwrap_or_else(|| ".".into())
}

pub fn pasta_do_exe() -> PathBuf {
    std::env::current_exe().ok().and_then(|p| p.parent().map(Path::to_path_buf)).unwrap_or_else(|| ".".into())
}

/// Chave que assina o save: `console/kv.bin` ao lado do programa, se
/// existir; senão a chave interna do programa (o save sempre sai assinado).
pub fn chave() -> Result<ChaveConsole, String> {
    let local = kv_local();
    if local.is_file() {
        return ChaveConsole::carrega(&local);
    }
    use super::chave_interna as c;
    ChaveConsole::de_componentes(&c::CERT, &c::N, &c::P, &c::Q)
}

/// O keyvault opcional do usuário: `console/kv.bin` ao lado do programa.
pub fn kv_local() -> PathBuf {
    pasta_do_exe().join("console").join("kv.bin")
}

/// Como a chave em uso aparece na tela.
pub fn descricao_chave() -> String {
    if kv_local().is_file() {
        match chave() {
            Ok(k) => format!("a chave do seu console ({})", k.console),
            Err(e) => format!("(chave indisponível: {e})"),
        }
    } else {
        "a chave interna do programa".into()
    }
}

fn gravavel(p: &Path) -> bool {
    let t = p.join(".teste-escrita");
    let ok = std::fs::write(&t, b"").is_ok();
    let _ = std::fs::remove_file(&t);
    ok
}

/// Backups ao lado do programa; se a pasta não for gravável, na pasta de
/// dados do usuário.
pub fn pasta_backups() -> Result<PathBuf, String> {
    let local = pasta_do_exe().join("backups");
    if std::fs::create_dir_all(&local).is_ok() && gravavel(&local) {
        return Ok(local);
    }
    let dados = std::env::var_os("APPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(|| home().join(".local").join("share"))
        .join("biohazard6-save-editor")
        .join("backups");
    std::fs::create_dir_all(&dados).map_err(|e| format!("backup: {e}"))?;
    Ok(dados)
}

/// Onde os diálogos começam: a pasta do save aberto, a pasta do save de PC
/// na Steam (se houver) ou a pasta pessoal.
pub fn pasta_inicial(atual: Option<&Path>) -> PathBuf {
    if let Some(p) = atual.and_then(Path::parent) {
        return p.into();
    }
    let h = home();
    pasta_save_steam(&h).unwrap_or(h)
}

/// ID do RE6 na Steam.
const APP_STEAM: &str = "221040";

/// Pastas `userdata` da Steam: Windows (Program Files, com e sem x86),
/// Linux (nativo, Flatpak e Snap).
fn userdata_steam(h: &Path) -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = ["ProgramFiles(x86)", "ProgramFiles"]
        .iter()
        .filter_map(std::env::var_os)
        .map(|p| PathBuf::from(p).join("Steam").join("userdata"))
        .collect();
    v.push(PathBuf::from(r"C:\Program Files (x86)\Steam\userdata"));
    v.push(h.join(".steam").join("steam").join("userdata"));
    v.push(h.join(".local").join("share").join("Steam").join("userdata"));
    v.push(h.join(".var").join("app").join("com.valvesoftware.Steam").join(".local").join("share").join("Steam").join("userdata"));
    v.push(h.join("snap").join("steam").join("common").join(".local").join("share").join("Steam").join("userdata"));
    v
}

/// A pasta do save do RE6 na Steam (`userdata/<conta>/221040/remote`). Com
/// várias contas que têm o jogo, a `userdata`; sem nenhuma, nada (quem só
/// edita save do Xbox continua começando na pasta pessoal).
fn pasta_save_steam(h: &Path) -> Option<PathBuf> {
    userdata_steam(h).into_iter().filter(|p| p.is_dir()).find_map(|userdata| {
        let saves: Vec<PathBuf> = std::fs::read_dir(&userdata)
            .ok()?
            .filter_map(|e| e.ok().map(|e| e.path().join(APP_STEAM).join("remote")))
            .filter(|p| p.is_dir())
            .collect();
        match saves.as_slice() {
            [] => None,
            [um] => Some(um.clone()),
            _ => Some(userdata),
        }
    })
}
