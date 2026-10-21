//! As 96 habilidades do jogo (dados/habilidades.json, gerado por
//! ferramentas/habilidades.py): nome e descrição em português, o modo em
//! que são usadas e se é de munição infinita. Mesma ordem dos bytes do save.

use serde::Deserialize;

#[derive(Deserialize)]
pub struct Habilidade {
    pub nome: String,
    pub descricao: String,
    /// 0 campanha, 1 Mercenários, 2 Caça ao Agente
    pub modo: u8,
    pub infinita: bool,
}

pub const MODOS: [&str; 3] = ["CAMPANHA", "MERCENÁRIOS", "CAÇA AO AGENTE"];

pub fn carrega() -> Vec<Habilidade> {
    #[derive(Deserialize)]
    struct Dados {
        habilidades: Vec<Habilidade>,
    }
    serde_json::from_str::<Dados>(include_str!("../dados/habilidades.json")).expect("dados/habilidades.json inválido").habilidades
}

#[cfg(test)]
mod testes {
    use super::*;
    use crate::save::HABILIDADES;

    #[test]
    fn noventa_e_seis_e_as_oito_infinitas() {
        let h = carrega();
        assert_eq!(h.len(), HABILIDADES);
        let inf: Vec<usize> = (0..h.len()).filter(|&i| h[i].infinita).collect();
        assert_eq!(inf, (28..36).collect::<Vec<_>>());
        assert_eq!(h[28].nome, "Pistola infinita");
        assert_eq!(h[35].nome, "Balestra infinita");
        assert!(h.iter().all(|x| (x.modo as usize) < MODOS.len()));
    }
}
