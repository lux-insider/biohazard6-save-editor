"""Monta as imagens da interface a partir das artes extraídas da ISO do RE6
(pasta "RE6 - Integração Visual"), sempre em PNG, sem perda.

Uso: python3 ferramentas/artes.py "<pasta RE6 - Integração Visual>"
(precisa de Pillow e fontTools)

As texturas do jogo já vêm comprimidas pelo próprio jogo (DXT, em blocos de
4x4); por isso o editor não acrescenta outra compressão com perda (JPEG) e
guarda os retratos e os ícones no tamanho em que estão no jogo. Só os bustos
da lista de personagens e o elenco são montagens reduzidas, no tamanho em que
aparecem. As fontes ficam só com as letras usadas em português.

A fonte do jogo (ui/fontes/RE6-Mensagem.ttf) é gerada à parte, por
ferramentas/fonte_jogo.py.
"""
import os, sys
from PIL import Image

RAIZ = sys.argv[1]
ISO = os.path.join(RAIZ, 'artes', 'iso', 'data', 'id')
DLC = os.path.join(RAIZ, 'dlc', '3-Mode Pack', 'artes', 'data', 'id', 'jpn', 'extra')
UI = os.path.join(os.path.dirname(os.path.abspath(__file__)), '..', 'ui', 'imagens')


def abre(rel):
    return Image.open(os.path.join(ISO, rel)).convert('RGBA')


def salva(img, rel):
    caminho = os.path.join(UI, rel)
    os.makedirs(os.path.dirname(caminho), exist_ok=True)
    img.save(caminho, optimize=True)


# Logo: o "6" e a palavra BIOHAZARD da tela final (endlogo).
logo = abre('title/endlogo_BM_NOMIP.png')
seis = logo.crop(logo.getchannel('A').crop((0, 0, 330, 512)).getbbox())
salva(seis, 'seis.png')
palavra = logo.crop((360, 0, 1024, 140))
salva(palavra.crop(palavra.getchannel('A').point(lambda a: 255 if a > 40 else 0).getbbox()), 'biohazard.png')
# Tela inicial: o fundo azul de teias da tela de título do jogo (title01).
salva(abre('title/title01_BM_NOMIP.png').convert('RGB'), 'fundo_titulo.png')

# Ícone da janela: o "6" num quadrado.
lado = max(seis.size)
q = Image.new('RGBA', (lado, lado))
q.alpha_composite(seis, ((lado - seis.size[0]) // 2, (lado - seis.size[1]) // 2))
salva(q.resize((256, 256), Image.LANCZOS), 'icone.png')
q.resize((256, 256), Image.LANCZOS).save(os.path.join(UI, 'icone.ico'), sizes=[(16, 16), (32, 32), (48, 48), (64, 64), (128, 128), (256, 256)])

# Ícones dos itens: o atlas do HUD tem 10x10 células de 102,4 px; cada uma
# vai para uma célula inteira de 102 px (sem redimensionar).
atlas = abre('cockpit/itemicon_BM_NOMIP.png')
C = 102
saida = Image.new('RGBA', (C * 10, C * 10))
for i in range(100):
    x, y = int(i % 10 * 102.4), int(i // 10 * 102.4)
    saida.alpha_composite(atlas.crop((x, y, x + C, y + C)), (i % 10 * C, i // 10 * C))
salva(saida, 'itens.png')

# Retratos dos 8 personagens (ao fundo do inventário), no tamanho original.
PERS = [(0, 'leon'), (1, 'helena'), (2, 'chris'), (3, 'piers'), (4, 'jake'),
        (5, 'sherry'), (6, 'ada'), (29, 'agente')]
originais = [abre('jpn/extra/characters%02d_BM_NOMIP.png' % n) for n, _ in PERS]
for r, (_, nome) in zip(originais, PERS):
    salva(r, 'retratos/%s.png' % nome)


def cabeca(r):
    """Topo e centro da cabeça: a primeira faixa de linhas com tinta."""
    a = r.getchannel('A').point(lambda v: 255 if v > 80 else 0)
    topo = a.getbbox()[1]
    caixa = a.crop((0, topo, r.width, topo + 90)).getbbox()
    return topo, (caixa[0] + caixa[2]) // 2


# Bustos para a lista de personagens: cabeça e ombros de cada retrato,
# 288x128 cada, um embaixo do outro (a cabeça fica a 62% da largura).
BW, BH, ALT = 288, 128, 470
bustos = Image.new('RGBA', (BW, BH * 8))
for i, r in enumerate(originais):
    topo, cx = cabeca(r)
    larg = ALT * BW // BH
    x0, y0 = cx - int(larg * 0.64), max(0, topo - 30)
    bustos.alpha_composite(r.crop((x0, y0, x0 + larg, y0 + ALT)).resize((BW, BH), Image.LANCZOS), (0, i * BH))
salva(bustos, 'bustos.png')

# Elenco para a aba Save: os 8 lado a lado, sobre o fundo do editor.
# Jake e Sherry, Chris e Piers, Leon e Helena ao centro; Ada e o Agente nas pontas.
ordem = ['ada', 'jake', 'sherry', 'chris', 'piers', 'leon', 'helena', 'agente']
EH, PASSO = 560, 205
elenco = Image.new('RGBA', (PASSO * 7 + EH, EH), (4, 6, 12, 255))
for k, nome in enumerate(ordem):
    r = originais[[n for _, n in PERS].index(nome)].resize((EH, EH), Image.LANCZOS)
    elenco.alpha_composite(r, (k * PASSO, 0))
salva(elenco.convert('RGB'), 'elenco.png')

# Fontes: só as letras do português e os símbolos da interface.
try:
    from fontTools import subset
except ImportError:
    subset = None
    print('fontTools ausente: fontes mantidas como estão')
if subset:
    TEXTO = ''.join(chr(c) for c in range(0x20, 0x7F)) + 'ÀÁÂÃÇÉÊÍÓÔÕÚÜàáâãçéêíóôõúü·…–—“”‘’ºª°✕✔▼▲●'
    FONTES = '/usr/share/fonts/truetype/noto/'
    for f in ['NotoSans-Regular.ttf']:
        op = subset.Options(); op.layout_features = ['kern', 'liga']; op.name_IDs = ['*']; op.notdef_outline = True
        s = subset.Subsetter(op); fonte = subset.load_font(FONTES + f, op)
        s.populate(text=TEXTO); s.subset(fonte)
        subset.save_font(fonte, os.path.join(UI, '..', 'fontes', f), op)
print('ok')
