"""Gera dados/historia.json: os 80 arquivos da história (um por emblema de
serpente, 20 por campanha), tirados de mes_file_por (textos do jogo, em
português). Títulos nas mensagens 3800-3879; o texto de cada um fica 86
mensagens depois. As quebras de linha do jogo (largura fixa da tela) viram
parágrafos corridos.

Uso: python3 -I ferramentas/historia.py <textos/mes_file_por.json> dados/historia.json
"""
import json
import re
import sys

CAMPANHAS = ['Leon', 'Chris', 'Jake', 'Ada']
TITULO, TEXTO, QTD = 3800, 3886, 80


def paragrafos(t):
    t = re.sub(r'</?SIZE[^>]*>', '', t).replace('\r\n', '\n')
    saida = []
    for p in re.split(r'\n\s*\n', t):
        p = ' '.join(l.strip() for l in p.split('\n') if l.strip())
        if p:
            saida.append(p)
    return saida


def main(origem, destino):
    m = json.load(open(origem, encoding='utf-8'))
    arquivos = []
    for i in range(QTD):
        titulo = m[str(TITULO + i)].strip()
        corpo = paragrafos(m[str(TEXTO + i)])
        assert titulo and corpo, i
        arquivos.append({'campanha': CAMPANHAS[i // 20], 'titulo': titulo, 'paragrafos': corpo})
    with open(destino, 'w', encoding='utf-8') as f:
        json.dump({'arquivos': arquivos}, f, ensure_ascii=False, indent=1)
    print(destino, len(arquivos), 'arquivos')


if __name__ == '__main__':
    main(*sys.argv[1:3])
