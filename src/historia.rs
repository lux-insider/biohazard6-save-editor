//! Os 80 arquivos da história (dados/historia.json, gerado por
//! ferramentas/historia.py a partir dos textos do jogo em português).
//! Um arquivo por emblema de serpente, 20 por campanha. Só leitura: nada
//! disso vai para o save.

use serde::Deserialize;

#[derive(Deserialize)]
pub struct Arquivo {
    pub campanha: String,
    pub titulo: String,
    pub paragrafos: Vec<String>,
}

/// Campanhas na ordem do jogo; o índice serve também para o busto
/// (Leon 0, Chris 2, Jake 4, Ada 6 em ui/imagens/bustos.png).
pub const CAMPANHAS: [&str; 4] = ["Leon", "Chris", "Jake", "Ada"];

pub fn carrega() -> Vec<Arquivo> {
    #[derive(Deserialize)]
    struct Dados {
        arquivos: Vec<Arquivo>,
    }
    serde_json::from_str::<Dados>(include_str!("../dados/historia.json")).expect("dados/historia.json inválido").arquivos
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn oitenta_arquivos_vinte_por_campanha() {
        let a = carrega();
        assert_eq!(a.len(), 80);
        for (i, c) in CAMPANHAS.iter().enumerate() {
            assert!(a[i * 20..(i + 1) * 20].iter().all(|x| x.campanha == *c));
        }
        assert_eq!(a[32].titulo, "O arquivo de Chris");
        assert!(a.iter().all(|x| !x.titulo.is_empty() && !x.paragrafos.is_empty()));
        // sem marcas de formatação do jogo no texto
        assert!(a.iter().flat_map(|x| &x.paragrafos).all(|p| !p.contains('<') && !p.contains('\r')));
    }
}
