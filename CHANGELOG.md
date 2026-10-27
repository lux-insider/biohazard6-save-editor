# Histórico de versões

Todas as mudanças relevantes do BIOHAZARD 6 SAVE EDITOR, da mais nova para a
mais antiga. As versões seguem o [Versionamento Semântico](https://semver.org/lang/pt-BR/).

---

## 1.0.0 — 2026-10-21

Primeira versão.

### Saves
- **Xbox 360 e PC (Steam)**, com detecção automática da plataforma. No Xbox,
  só abre saves do Resident Evil 6 (Title ID `43430819`) em pacotes CON; no
  PC, o `savedata.bin` da Steam (`Steam\userdata\<conta>\221040\remote\`).
  Quando a Steam tem o save do RE6, o diálogo de abrir já começa nessa pasta.
- **Inventário** dos 8 personagens (Leon, Helena, Chris, Piers, Jake, Sherry,
  Ada e Agente): armas do personagem (obtida ou não e balas no pente),
  comprimidos, spray, granadas, maleta (munição, ervas e granadas) e vida
  atual. Em cada espaço, o editor só oferece o que o jogo aceita ali.
- **Itens no máximo do jogo:** granadas e bomba remota até 5, comprimidos de
  cura até 12, spray 1 e ervas 1 por espaço. Acima disso o jogo zera o item
  ou trava o uso. Armas e munição aceitam até 999.
- **Pontos de habilidade** (até 9.999.999) e data do save.
- **Habilidades:** as 96 do jogo (campanha, Mercenários e Caça ao Agente),
  com nome e descrição originais. Compra individual, de um modo inteiro ou só
  das 8 de munição infinita.
- **Só no Xbox:** nome do save, perfil, console e dispositivo.

### Consulta
- **Arquivos:** os 80 textos da história do jogo (20 por campanha), só para
  leitura.

### Visual
- Tudo vem dos arquivos do *BIOHAZARD 6*, em PNG sem perda: logo, tela de
  título, elenco, retratos dos personagens no tamanho original e os ícones
  dos itens.
- Fonte de mensagens do jogo convertida e embutida.
- Interface nativa em Slint, sem navegador nem WebView.

### Assinatura e segurança
- O save do Xbox 360 sai sempre assinado e pronto para o console, com a chave
  interna do programa ou com o `console/kv.bin` do seu console, se existir.
- **Gravação segura:** grava numa cópia, lê de volta e confere; só então troca
  pelo original. Backup automático na pasta `backups/` antes de cada gravação.
- No save de PC, o editor muda só os campos editados e a verificação; a marca
  do computador no cabeçalho fica intacta.

### Validação
- **No console:** o save editado carrega normalmente no Xbox 360; as
  habilidades compradas pelo editor aparecem no menu do jogo e podem ser
  equipadas, inclusive as 8 de munição infinita; os pontos gravados são os
  que o jogo mostra; o inventário editado aparece no jogo.
- **No PC:** formato conferido em dois saves reais da Steam. Abrir e salvar
  sem alterações devolve o arquivo idêntico byte a byte. O save de PC ainda
  não foi testado dentro do jogo.
- Os máximos dos itens foram tirados do executável do jogo (`default.xex`).
