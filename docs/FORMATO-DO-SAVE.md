# Formato do save do Resident Evil 6

Referência técnica do que o editor lê e grava. Os valores vêm de
`src/save.rs` e `src/pacote.rs`. Os campos do Xbox 360 foram conferidos no
console; os do PC, em saves reais da Steam.

## Visão geral

O Xbox 360 e o PC guardam o mesmo arquivo, o `savedata.bin`, sem
criptografia. O que muda é o envelope, a ordem dos bytes e a posição dos
campos.

| | Xbox 360 | PC (Steam) |
|---|---|---|
| Arquivo | pacote STFS **CON** com o `savedata.bin` dentro | o próprio `savedata.bin` |
| Pasta | `Content\<perfil>\43430819\00000001\` | `Steam\userdata\<conta>\221040\remote\` |
| Tamanho do `savedata.bin` | 24.576 bytes (`0x6000`) | 39.416 bytes nos saves conhecidos |
| Ordem dos bytes | big-endian | little-endian |
| Identificador (`0x00`) | `0x12071800` | `0x12071801` |
| Integridade ao gravar | verificação + hashes SHA-1 do pacote + assinatura RSA | verificação |

Title ID no Xbox 360: `43430819`. ID na Steam: `221040`.

## savedata.bin

### Cabeçalho

| Offset | Tamanho | Campo |
|---|---|---|
| `0x00` | 4 | identificador |
| `0x04` | 4 | tamanho do arquivo |
| `0x08` | 4 | verificação |
| `0x10` (Xbox) / `0x20` (PC) | 24 | data e hora do save: seis u32 (ano, mês, dia, hora, minuto, segundo) |

No PC, os bytes de `0x0C` a `0x1F` guardam uma marca do computador ou da
conta. Ela fica fora da verificação e o editor nunca mexe nela.

**Verificação:** soma de 32 bits (com estouro) de todas as palavras u32, na
ordem de bytes da plataforma, de `0x0C` (Xbox) ou `0x18` (PC) até o fim do
arquivo.

### Campos editáveis

| Campo | Xbox 360 | PC | Formato |
|---|---|---|---|
| Pontos de habilidade | `0x6F8` | `0x70C` | u32, até 9.999.999 |
| Habilidades | `0x700` | `0x714` | um byte por número de habilidade (ver abaixo) |
| Inventário | `0x4AF4` | `0x84EC` | 8 blocos de `0x70` bytes |

### Habilidades

Um byte por número de habilidade, de 0 a 96. O número 0 não é usado: a
habilidade **N** fica em `início + N`, ou seja, de `0x701` a `0x760` no Xbox
e de `0x715` a `0x774` no PC. A ordem é a dos nomes do jogo (1 = Armas de
fogo Nv. 1).

| Valor | Significado |
|---|---|
| `00` | não disponível |
| `01` | à venda |
| `03` | comprada |

Num save novo do console, as habilidades à venda são a 1, 4, 7, 10, 13 e 18:
o nível 1 de Armas de fogo, Corpo a corpo, Defesa, Golpe crítico, Perfuração
e Caçador de zumbis.

### Inventário

Ordem dos blocos: Leon, Helena, Chris, Piers, Jake, Sherry, Ada, Agente. O
primeiro byte de cada bloco é o número do personagem (0 a 7).

Cada bloco de `0x70` bytes tem 16 bytes de cabeçalho (personagem, vida atual
e vida máxima em u16, e outros campos) e 24 espaços de 4 bytes:

| Campo | Formato |
|---|---|
| item | u16 = tipo × 256 + número |
| quantidade | u16 |

No Xbox, os bytes do espaço ficam `tipo, número, quantidade`; no PC, por ser
little-endian, `número, tipo, quantidade (byte baixo primeiro)`.

| Espaço | Uso |
|---|---|
| 0–5 | armas do personagem; a quantidade é a munição no pente |
| 7 | comprimidos |
| 8 | spray |
| 9–10 | granadas |
| 15 em diante | maleta |

Valores especiais:

| Valor | Significado |
|---|---|
| quantidade `FFFE` | arma ainda não obtida |
| quantidade `FFFF` | arma obtida, sem contagem |
| item `0000`, quantidade `FFFE` | espaço vazio |
| item `0000`, quantidade `FFFF` | espaço fechado |

O nome de cada item vem dos textos do próprio jogo: mensagem
`(tipo − 1) × 100 + número`.

### Restrições do jogo

- Uma arma que não pertence ao personagem, colocada nos espaços 0–5, faz o
  jogo trocar o inventário pelo padrão do capítulo.
- Na maleta, uma arma de outro personagem aparece, mas só pode ser descartada.
- Itens acima do máximo somem ou travam no jogo: as granadas zeram, os
  comprimidos não podem ser usados e as ervas do cenário não viram
  comprimido. Os máximos estão no executável (`default.xex`, função em
  `0x824DDFA8` na versão do disco, tabela montada pelo construtor em
  `0x83103240`):

  | Item | Máximo |
  |---|---|
  | Granada, incendiária, de luz e bomba remota (`01 13`–`01 16`) | 5 |
  | Comprimido de cura (`01 1D`) | 12 |
  | Spray de primeiros socorros (`01 20`) | 1 |
  | Ervas (tipo 3) | 1 por espaço |

  Armas e munição aceitam até 999 sem problema no jogo.

O editor só oferece, em cada espaço, o que o jogo aceita.

## Pacote CON (Xbox 360)

O mesmo formato do Resident Evil 5: pacote STFS do tipo CON, assinado pelo
console. Ao gravar, o editor:

1. recalcula a verificação do `savedata.bin`;
2. regrava os blocos e refaz os hashes SHA-1 dos blocos, da tabela de hashes,
   do diretório e do cabeçalho;
3. assina o cabeçalho com RSA (PKCS#1 v1.5, SHA-1), com o `console/kv.bin`,
   se existir, ou com a chave interna do programa.

Pacotes LIVE e PIRS são recusados.

## Gravação segura

1. Montagem do arquivo em memória e conferência completa.
2. Cópia do arquivo atual para `backups/`.
3. Gravação numa cópia ao lado do destino (`<arquivo>.gravando`), conferência
   e só então troca de nome sobre o original.

Se qualquer etapa falhar, o original não é tocado.
