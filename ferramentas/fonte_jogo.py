"""Gera ui/fontes/RE6-Mensagem.ttf a partir da fonte de mensagens do jogo.

A fonte do RE6 é de bitmap: mes_font (GFD, a tabela dos glifos) e as
texturas mes_font_00/01 (o glifo fica no alfa). Cada glifo é ampliado 8x,
contornado em curvas (potrace) e gravado como TrueType. Só entram os
caracteres latinos e os símbolos que o editor usa.

Uso: python fonte_jogo.py <mes_font.gfd> <mes_font_00.png> <mes_font_01.png> <saida.ttf>
(precisa de fontTools, Pillow, numpy e potracer)
"""
import struct
import sys

import numpy as np
import potrace
from fontTools.fontBuilder import FontBuilder
from fontTools.pens.cu2quPen import Cu2QuPen
from fontTools.pens.reverseContourPen import ReverseContourPen
from fontTools.pens.ttGlyphPen import TTGlyphPen
from PIL import Image

ESCALA = 8          # ampliação antes de contornar
UPM = 1152          # unidades por em = 36 px * 32
PX = UPM // 36      # unidades por pixel do glifo original

EXTRAS = "…·–—“”‘’€←→▲▼★"


def le_gfd(caminho):
    d = open(caminho, 'rb').read()
    assert d[:4] == b'\0DFG', 'não é GFD'
    n = struct.unpack('>I', d[0x1C:0x20])[0]
    nl = struct.unpack('>I', d[0x30:0x34])[0]
    p = 0x34 + nl + 1
    assert p + n * 16 <= len(d), 'GFD truncado'
    for i in range(n):
        c, w2, w3, _ = struct.unpack('>4I', d[p + i * 16:p + i * 16 + 16])
        yield c, w2 >> 24, (w2 >> 12) & 0xFFF, w2 & 0xFFF, (w3 >> 12) & 0xFFF, w3 & 0xFFF


def quer(c):
    return 0x20 <= c < 0x7F or 0xA0 <= c <= 0x17F or chr(c) in EXTRAS


def contorna(alfa, pen, base, dx):
    """Desenha o glifo (alfa em 0..255, h x w) na caneta, em unidades da fonte."""
    h, w = alfa.shape
    img = Image.fromarray(alfa).resize((w * ESCALA, h * ESCALA), Image.BICUBIC)
    bits = np.asarray(img) >= 128
    if not bits.any():
        return
    # o potracer considera tinta o que é falso (preto), então vai invertido
    caminho = potrace.Bitmap(~bits).trace(turdsize=4, alphamax=1.0, opticurve=True, opttolerance=0.2)
    u = PX / ESCALA

    def pt(p):
        return (round((p.x + dx * ESCALA) * u), round((base * ESCALA - p.y) * u))

    for curva in caminho:
        pen.moveTo(pt(curva.start_point))
        for s in curva.segments:
            if s.is_corner:
                pen.lineTo(pt(s.c))
                pen.lineTo(pt(s.end_point))
            else:
                pen.curveTo(pt(s.c1), pt(s.c2), pt(s.end_point))
        pen.closePath()


def main(gfd, tex0, tex1, saida):
    texturas = [np.asarray(Image.open(t).getchannel('A')) for t in (tex0, tex1)]
    glifos = {c: g for c, *g in le_gfd(gfd) if quer(c)}

    # linha de base: último pixel com tinta do "H"
    t, x, y, w, h = glifos[ord('H')]
    col = texturas[t][y:y + h, x:x + w].max(axis=1)
    base = int(np.nonzero(col > 128)[0].max()) + 1

    nomes, cmap, metricas, desenhos = ['.notdef'], {}, {'.notdef': (UPM // 2, 0)}, {}
    vazio = TTGlyphPen(None)
    desenhos['.notdef'] = vazio.glyph()
    for c in sorted(glifos):
        t, x, y, w, h = glifos[c]
        nome = 'uni%04X' % c
        pen = TTGlyphPen(None)
        alfa = texturas[t][y:y + h, x:x + w]
        tinta = np.nonzero(alfa.max(axis=0) > 60)[0]
        if c == 0x20 or not len(tinta):
            avanco, dx = 8, 0
        elif chr(c).isdigit():
            # algarismos com a mesma largura, centralizados
            avanco = 16
            dx = (avanco - (tinta.max() - tinta.min() + 1)) / 2 - tinta.min()
        elif chr(c) == 'j':
            # o gancho do j entra embaixo da letra anterior
            avanco = int(tinta.max() - tinta.min() + 1) - 2
            dx = -3 - int(tinta.min())
        else:
            # espaçamento regular: 1 pixel de cada lado da tinta
            avanco = int(tinta.max() - tinta.min() + 1) + 2
            dx = 1 - int(tinta.min())
        if c != 0x20:
            contorna(alfa, ReverseContourPen(Cu2QuPen(pen, 1.0)), base, dx)
        nomes.append(nome)
        cmap[c] = nome
        desenhos[nome] = pen.glyph()
        metricas[nome] = (avanco * PX, round((dx + (int(tinta.min()) if len(tinta) else 0)) * PX))

    fb = FontBuilder(UPM, isTTF=True)
    fb.setupGlyphOrder(nomes)
    fb.setupCharacterMap(cmap)
    fb.setupGlyf(desenhos)
    fb.setupHorizontalMetrics(metricas)
    asc, desc = base * PX, -(36 - base) * PX
    fb.setupHorizontalHeader(ascent=asc, descent=desc)
    fb.setupOS2(sTypoAscender=asc, sTypoDescender=desc, usWinAscent=asc, usWinDescent=-desc)
    fb.setupNameTable({'familyName': 'RE6 Mensagem', 'styleName': 'Regular'})
    fb.setupPost()
    fb.save(saida)
    print(saida, len(nomes), 'glifos, base', base)


if __name__ == '__main__':
    main(*sys.argv[1:5])
