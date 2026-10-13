# PhotoCraft para Android: plano e decisões

Documento de trabalho do fork `Vinn4k/photocraft`, branch `android-mobile`. Reúne as decisões já
tomadas para levar o PhotoCraft ao Android e serve de ponto de partida para qualquer sessão nova.

## Decisões tomadas

| Tema | Decisão |
|---|---|
| Plataforma | **Android** (iOS fora do escopo por enquanto) |
| Aparelho de referência | **Samsung Galaxy S26 Ultra**: tela de ~6,9", 19,5:9, com **S Pen** |
| Escopo de recursos | **Tudo o que o desktop faz** (camadas, máscaras, ajustes, estilos, texto, vetores, pincéis, PSD) |
| Linguagem | **Rust** (sem reescrita). Mesmo motor e mesma UI egui/eframe sobre wgpu (Vulkan no Android) |
| Casca nativa | Mínima: `android-activity` (NativeActivity/GameActivity) + Gradle; Kotlin só onde o Android exigir (seletor de arquivos/SAF, compartilhar) |
| Entrada | **Caneta pinta, dedo navega** (pan/zoom/rotação com dois dedos) |
| Repositório | Fork `Vinn4k/photocraft`; trabalho na branch `android-mobile` |
| Orientação | **Retrato** é o layout principal; **paisagem** é variante desenhada (decidido em 13/10/2026) |
| Visual | **Tema atual** (Pro, cinza médio por padrão), com os tokens de `theme.rs`; sem identidade própria |
| Entrega dos desenhos | **Página interativa** no tamanho do S26 Ultra (canvas de wireframes navegável) |

Restrições do projeto que continuam valendo (ver `AGENTS.md`): só Rust, sem Tauri/Electron/webview,
nada abaixo de `ui-egui` depende de egui/eframe/winit, `unsafe` só em crate isolado (como
`photocraft-tablet`), testes em toda mudança, `cargo fmt` e `cargo clippy -- -D warnings`.

## Ponto de partida no código

- O motor (`engine`, `doc`, `ops`, `compose`, `gpu`, `io`…) é independente da UI e roda no Android
  sem mudanças de arquitetura.
- A UI está toda em `crates/ui-egui` (~300 arquivos, ~164 mil linhas), pensada para desktop: menus
  no topo, barra de ferramentas à esquerda, dock de painéis à direita (`dock.rs`), larguras mínimas
  fixas de 180–220 px em `panels.rs`.
- Apps existentes: `apps/photocraft` (desktop), `apps/photocraft-cli`, `apps/photocraft-web`
  (wasm; já tem `viewport` mobile no `index.html`, mas o layout não se adapta).
- Já existe: zoom por pinça (`wheel_nav.rs`) e leitura de pressão/inclinação de caneta
  (`stylus.rs`; no web via Pointer Events, no desktop via `photocraft-tablet`).

## Fases

1. **Desenho das telas** (entregue para revisão em 13/10/2026): wireframes das telas principais para
   o S26 Ultra; ver *Wireframes* abaixo. Aprovados os desenhos, segue a fase 2.
2. **Build Android mínimo:** novo app `apps/photocraft-android` (cdylib + `android-activity`),
   abrindo o editor atual sem mudanças de layout; empacotar APK (cargo-ndk / xbuild) e documentar
   em `docs/development.md`.
3. **Layout adaptativo em `ui-egui`:** modo de layout escolhido pelo tamanho da tela
   (desktop / tablet / celular), sem quebrar o desktop.
4. **Entrada por toque e S Pen:** gestos, separação caneta × dedo, pressão/inclinação da S Pen via
   `MotionEvent` (`TOOL_TYPE_STYLUS`, botão lateral), botões modificadores na tela (Shift, Alt, Ctrl)
   no lugar do teclado.
5. **Integração Android:** abrir/salvar pelo seletor do sistema (SAF), compartilhar, ciclo de vida
   (salvar ao ir para segundo plano), limites de memória e de histórico.
6. **Desempenho e testes** em aparelho real.

## Telas a desenhar

1. **Início / Galeria:** recentes, novo documento, abrir arquivo.
2. **Novo documento:** predefinições e tamanho personalizado.
3. **Editor em retrato:** canvas em tela cheia, barra de ferramentas inferior, barra de ações no topo.
4. **Editor em paisagem:** ferramentas na lateral, painel lateral recolhível.
5. **Camadas:** painel deslizante de baixo para cima.
6. **Ajustes e Propriedades da ferramenta:** pincel, Curvas, Níveis etc.
7. **Seletor de cor.**
8. **Menus:** os menus do desktop numa folha de ações.
9. **Exportar e salvar:** PSD, PNG, JPG.

Diretrizes para o desenho: alvos de toque de no mínimo 44–48 dp; um painel por vez no celular;
manter a lógica e os nomes do Photoshop (o objetivo do projeto é paridade 1:1); usar os tokens de
`docs/ui-design.md`.

## Wireframes (fase 1)

Canvas interativo, privado (compartilhe pelo menu Share da página para outras pessoas abrirem):
https://claude.ai/artifact/KjsfimDyrPbUVdWDtCnu81. As telas são ligadas entre si (botão Play em cada
prancheta). Medidas em dp: retrato 412 × 892, paisagem 892 × 412 (19,5:9). Cores do tema Pro (cinza
médio): chrome/cards `#535353`, dock `#424242`, campos `#454545`, borda `#686868`, linha selecionada
`#6B6B6B`, tela de pintura `#282828`, destaque `#378EF0`; textos `#EEEEEE`/`#D4D4D4`. Inter na
interface e JetBrains Mono nos números, como no desktop. Textos do catálogo `i18n/pt-br.tsv`.

| # | Tela | Decisões |
|---|---|---|
| 1 | Início / Galeria | Novo documento e Abrir… em destaque; Recentes em grade de 2 colunas com tamanho, modo e data; Abrir usa o seletor do Android (SAF) |
| 2 | Novo documento | Abas de categoria do Photoshop (Recentes, Foto, Imprimir, Arte e ilustração, Web); predefinições em grade de 3; detalhes: nome, largura, altura, unidade, resolução, modo de cor, profundidade, conteúdo do plano de fundo; Criar no topo |
| 3 | Editor em retrato | Barra superior (voltar, nome e "33,3% (Camada 1, RGB/8)", desfazer, refazer, exportar, menus); barra de opções da ferramenta com rolagem; tela de pintura em tela cheia; modificadores **Shift / Alt / Ctrl** flutuantes à esquerda (alternam, como teclas presas); trilho de Tamanho/Opacidade à direita; barra de ferramentas inferior com cores de 1º plano/fundo, 6 ferramentas (triângulo de grupo) e "todas as ferramentas"; barra de painéis: Camadas, Ajustes, Propriedades, Histórico, Cor |
| 4 | Editor em paisagem | Barra única com opções da ferramenta; ferramentas na lateral esquerda; painel à direita (272 dp) recolhível, com faixa de ícones de painéis |
| 5 | Camadas | Folha inferior (~2/3 da altura) com abas Camadas / Canais / Demarcadores, modo de mesclagem, Opacidade, Bloquear, Preenchimento, linhas de 60 dp com miniatura e máscara, e as ações do rodapé do Photoshop (fx, máscara, ajuste, grupo, nova, excluir) |
| 6a | Propriedades do pincel | Folha com predefinições, prévia do traço, Tamanho, Dureza, Suavização; seção S Pen: pressão → tamanho/opacidade, inclinação → ângulo, botão lateral (segurar) → ferramenta temporária |
| 6b | Ajustes: Curvas | Folha não modal (a imagem continua visível e atualiza ao vivo): predefinição, canal, Automático, ferramentas (ajuste na imagem, pontos preto/cinza/branco, desenhar), gráfico 300 dp com histograma, Entrada/Saída, rodapé do painel Propriedades |
| 7 | Seletor de cor | Tela cheia, como o diálogo modal do desktop: campo S/B, faixa de matiz, novo/atual, H S B / R G B / hex, conta-gotas, bibliotecas, Somente cores da Web, Amostras |
| 8 | Menus | Folha com pesquisa de comandos, os 11 menus do desktop como fichas e a lista do menu escolhido (mesma ordem, separadores, submenus e atalhos, mostrados para teclado físico/DeX) |
| 9 | Salvar e exportar | Exportar como: prévia, formato (PSD, PNG, JPG, Mais), qualidade, tamanho, escala, sRGB, perfil, metadados, tamanho estimado; Compartilhar e Salvar em… (SAF) |

Gestos (fase 4): a caneta pinta e usa a ferramenta ativa; o dedo nunca pinta: um dedo arrasta a vista,
dois dedos fazem pan/zoom/rotação; toque com dois dedos desfaz, com três refaz; pressionar
longo numa camada abre o menu de contexto dela. Um painel por vez no celular; alvos de toque ≥ 48 dp
(44 dp só em linhas densas do painel em paisagem).

## Telas no código (`crates/ui-egui/src/mobile`)

Iniciado em 13/10/2026, adiantando a fase 3 (o layout pode ser conferido no desktop antes do build
Android da fase 2).

- **Roteador** (`mobile/mod.rs`): modo de layout `desktop`/`phone` pelo tamanho da janela (celular
  quando a largura < 600 dp ou a altura < 480 dp, as classes compactas do Android). O app desktop
  mantém o layout dele em qualquer tamanho (`layout: desktop` por padrão); Android e web usam
  `auto`. Rotas Início, Novo documento e Editor; folhas Camadas, Pincel, Curvas, Cor, Menus e
  Exportar, uma por vez e só sobre o Editor; pilha de voltar (o Voltar do Android chega como a
  tecla `BrowserBack`). Estado em `UiState.mobile`; `ui.set {"mobile": {layout, route, sheet,
  back}}` navega e `ui.inspect` mostra `mobile` (ver `docs/control-protocol.md`).
- Conferência visual: `cargo run -p photocraft-ui-egui --example snapshot -- --size 412x892
  --layout auto --out phone.png` (sem GPU, use o Vulkan por software: `mesa-vulkan-drivers` e
  `VK_ICD_FILENAMES=/usr/share/vulkan/icd.d/lvp_icd.json`).

## Mensagem inicial para uma sessão nova

> Estou trabalhando no fork `Vinn4k/photocraft`, branch `android-mobile`. Leia
> `docs/android-mobile.md`: ele tem as decisões (Android, Galaxy S26 Ultra com S Pen, todos os
> recursos do desktop, Rust + egui/wgpu, caneta pinta e dedo navega) e as fases do plano. Também
> siga o `AGENTS.md`. Vamos continuar pela fase atual.
