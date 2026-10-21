"""Gera dados/habilidades.json: as 96 habilidades do jogo, na ordem dos
bytes do save (0x700), com nome e descrição em português (mes_file_por,
mensagens 1-96 e 101-196) e o modo em que são usadas.

Uso: python3 -I ferramentas/habilidades.py <textos/mes_file_por.json> dados/habilidades.json
"""
import json
import sys

# 0-60 campanha, 61-80 Mercenários, 81-95 Caça ao Agente (pelas descrições).
def modo(i):
    return 0 if i <= 60 else 1 if i <= 80 else 2

INFINITAS = range(28, 36)


def limpa(t):
    return ' '.join(t.replace('\r\n', ' ').split())


def main(origem, destino):
    m = json.load(open(origem, encoding='utf-8'))
    lista = [{'nome': limpa(m[str(i + 1)]), 'descricao': limpa(m[str(i + 101)]), 'modo': modo(i),
              'infinita': i in INFINITAS} for i in range(96)]
    assert all(h['nome'] and h['descricao'] for h in lista)
    assert [h['nome'] for h in lista[28:36]][0] == 'Pistola infinita'
    with open(destino, 'w', encoding='utf-8') as f:
        json.dump({'habilidades': lista}, f, ensure_ascii=False, indent=1)
    print(destino, len(lista), 'habilidades')


if __name__ == '__main__':
    main(*sys.argv[1:3])
