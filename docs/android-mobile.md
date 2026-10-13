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

1. **Desenho das telas** (atual): wireframes das telas principais para o S26 Ultra.
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

## Perguntas em aberto

- Orientação principal: retrato ou paisagem? (proposta: retrato principal, paisagem como variante)
- Visual: manter o tema escuro atual ou criar uma identidade própria para o mobile? (proposta: tema atual)
- Formato da entrega dos desenhos: página interativa no tamanho do S26 Ultra (proposta) ou documento.

## Mensagem inicial para uma sessão nova

> Estou trabalhando no fork `Vinn4k/photocraft`, branch `android-mobile`. Leia
> `docs/android-mobile.md`: ele tem as decisões (Android, Galaxy S26 Ultra com S Pen, todos os
> recursos do desktop, Rust + egui/wgpu, caneta pinta e dedo navega) e as fases do plano. Também
> siga o `AGENTS.md`. Vamos continuar pela fase atual.
