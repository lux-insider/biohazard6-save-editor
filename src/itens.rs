//! Catálogo dos itens do inventário (dados/itens.json): código do save,
//! nome em português (texto do próprio jogo, mes_item_por) e o ícone no
//! atlas do HUD (ui/imagens/itens.png, 10x10 células).

use serde::Deserialize;

#[derive(Deserialize, Clone)]
pub struct Item {
    pub tipo: u8,
    pub numero: u8,
    pub nome: String,
    pub icone: i32,
}

pub const CLASSES: [&str; 3] = ["Itens", "Munição", "Ervas"];

/// Itens do tipo 1 que funcionam num espaço livre (itens e maleta): granadas,
/// bomba remota, comprimido e spray. As armas não: testado no console, uma
/// arma de fora da lista do personagem fica na maleta sem uso (só dá para
/// descartar) e, num espaço de arma, faz o jogo trocar o inventário inteiro
/// pelo kit padrão do capítulo.
pub fn usavel_em_espaco_livre(tipo: u8, numero: u8) -> bool {
    tipo != 1 || matches!(numero, 0x13..=0x16 | 0x1D | 0x20)
}

pub struct Itens {
    pub lista: Vec<Item>,
}

impl Itens {
    pub fn carrega() -> Self {
        let lista = serde_json::from_str(include_str!("../dados/itens.json")).expect("dados/itens.json inválido");
        Self { lista }
    }

    /// Item pelo código do save (o tipo sem a marca do nibble alto).
    pub fn busca(&self, classe: u8, numero: u8) -> Option<&Item> {
        self.lista.iter().find(|i| i.tipo == classe && i.numero == numero)
    }

    /// O que o editor oferece para um espaço livre desta classe.
    pub fn da_classe(&self, classe: u8) -> Vec<&Item> {
        self.lista.iter().filter(|i| i.tipo == classe && usavel_em_espaco_livre(i.tipo, i.numero)).collect()
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn catalogo_carrega_e_acha_a_magnum() {
        let i = Itens::carrega();
        assert_eq!(i.busca(1, 7).unwrap().nome, "Lightning Hawk");
        assert_eq!(i.busca(2, 6).unwrap().nome, "Munição Magnum .50");
        assert!(i.lista.iter().all(|x| (1..=3).contains(&x.tipo)));
        // espaço livre: sem armas (lança-foguetes 01 1B fica de fora)
        let itens: Vec<u8> = i.da_classe(1).iter().map(|x| x.numero).collect();
        assert_eq!(itens, vec![0x13, 0x14, 0x15, 0x16, 0x1D, 0x20]);
    }
}
