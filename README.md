# Maré

Cliente de torrent leve para Windows, todo em Rust e open source.

Mantido por **Jhonatan Vicentini** ([@JhonatanVicentini](https://github.com/JhonatanVicentini)).

- Motor: [`librqbit`](https://crates.io/crates/librqbit) (DHT, magnet, retomada, escolha de arquivos)
- Janela: [`egui`/`eframe`](https://crates.io/crates/eframe) com OpenGL (`glow`)
- Um único `Mare.exe` (~17 MB), sem instalador e sem runtime

## O que ele faz

- Adicionar por link magnet, link `https://…/arquivo.torrent`, arquivo `.torrent`,
  arrastando o arquivo para a janela ou colando com **Ctrl+V**
- Percebe um magnet copiado quando você volta para a janela e oferece adicionar
- Antes de baixar, mostra a lista de arquivos para marcar só o que quiser e escolher a pasta
- Lista com progresso, velocidade, tempo restante, pares e seeds
- Pausar, retomar, abrir a pasta (duplo clique no cartão também abre), remover com ou sem apagar os arquivos
- Menu do botão direito em cada download
- Filtros (baixando, semeando, concluídos, pausados, com erro) e busca por nome
- Limite de velocidade de download e upload
- Aviso do Windows quando um download termina
- Tema escuro, claro ou igual ao Windows
- Ao reabrir, tudo continua de onde parou

Use o Maré para baixar o que você tem direito de baixar: distribuições Linux, conteúdo
em domínio público ou com licença livre, seus próprios arquivos.

## Baixar

As versões prontas ficam em [Releases](https://github.com/neonio-it/mare-torrent/releases).

## Compilar

Precisa do [Rust](https://rustup.rs) (versão estável) com o toolchain MSVC.

```powershell
cargo build --release
```

O executável sai em `target\release\mare.exe`.

### Compilar e assinar

```powershell
.\scripts\compilar-e-assinar.ps1
```

Compila, copia para `dist\Mare.exe` e assina com o certificado da Certum. Veja
[docs/ASSINATURA.md](docs/ASSINATURA.md).

### Smart App Control

Com o Smart App Control do Windows ligado, programas sem assinatura digital que o Windows
não conhece são bloqueados ("Uma política de Controle de Aplicativo bloqueou este
arquivo"). As versões publicadas em Releases são assinadas para evitar isso. Se você
compilar por conta própria, o seu `.exe` não terá assinatura.

## Onde ficam os dados

- Configurações: `%APPDATA%\Mare\config.json`
- Lista de torrents e progresso salvo: `%APPDATA%\Mare\sessao\`

Para começar do zero, feche o app e apague a pasta `%APPDATA%\Mare`.

A variável de ambiente `MARE_DATA_DIR` troca essa pasta (útil para testar ou usar o app
de forma portátil, num pendrive).

## Código

| Arquivo | O que tem |
| --- | --- |
| `src/main.rs` | Abre a janela, ícone, argumento da linha de comando |
| `src/app.rs` | Toda a interface: barras, lista, diálogos, avisos |
| `src/engine.rs` | Ponte com o librqbit (runtime tokio, comandos, estado dos torrents) |
| `src/theme.rs` | Paleta de cores, estilo e fontes (Segoe UI) |
| `src/widgets.rs` | Peças visuais: barra de progresso, selos, botões, logotipo |
| `src/settings.rs` | Preferências salvas em JSON |
| `src/util.rs` | Formatação de tamanhos, velocidades e tempos |
| `scripts/compilar-e-assinar.ps1` | Compila e assina o executável |

## Licença e autoria

[MIT](LICENSE). Projeto criado e mantido por Jhonatan Vicentini
([@JhonatanVicentini](https://github.com/JhonatanVicentini)).
