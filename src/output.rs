//! Formatação e emissão do resultado final em stdout ou arquivo.
//!
//! **REGRA INVIOLÁVEL**: este é o ÚNICO módulo autorizado a usar `println!`
//! ou `write!`/`writeln!` em `stdout`/arquivo de saída. Todos os demais módulos
//! devem usar `tracing::*` para logs (que vão para stderr).
//!
//! Formatos suportados:
//! - `json` (default em pipe / sempre que LLM consome): JSON pretty-print.
//! - `text` (default em TTY): formato compacto otimizado para tokens de LLM e
//!   leitura humana — `[N] título / URL / snippet`.
//! - `markdown`: renderização Markdown (ideal para arquivos `.md` / GitHub).
//! - `auto`: detecção via TTY — `text` em terminal interativo, `json` em pipe.
//!
//! Roteamento de saída:
//! - Sem `--output PATH`: escreve em `stdout`.
//! - Com `--output PATH`: cria diretórios pai se necessário, escreve no
//!   arquivo com permissões 0o644 no Unix.

use crate::pipeline::ResultadoPipeline;
use crate::types::{FormatoSaida, ResultadoBusca, SaidaBusca, SaidaBuscaMultipla};
use anyhow::{Context, Result};
use std::fs;
use std::io::{self, Write};
use std::path::Path;

/// Imprime o resultado da busca no formato e destino especificados.
///
/// `caminho_saida = None` → stdout. `Some(path)` → arquivo (com criação dos
/// diretórios pai se ausentes).
pub fn emitir_resultado(
    resultado: &ResultadoPipeline,
    formato: FormatoSaida,
    caminho_saida: Option<&Path>,
) -> Result<()> {
    // Stream já emitiu incrementalmente — nada a fazer aqui.
    if matches!(resultado, ResultadoPipeline::Stream(_)) {
        tracing::debug!("ResultadoPipeline::Stream — saída já foi emitida em streaming");
        return Ok(());
    }

    let formato_resolvido = resolver_formato_auto(formato, caminho_saida);
    let texto_saida = match resultado {
        ResultadoPipeline::Unica(saida) => formatar_unica(saida.as_ref(), formato_resolvido)?,
        ResultadoPipeline::Multipla(saida) => formatar_multipla(saida.as_ref(), formato_resolvido)?,
        ResultadoPipeline::Stream(_) => unreachable!("Stream tratado acima"),
    };

    match caminho_saida {
        Some(caminho) => escrever_em_arquivo(caminho, &texto_saida),
        None => escrever_em_stdout(&texto_saida),
    }
}

/// Wrapper retrocompatível para chamadores que ainda usam apenas (resultado, formato).
/// Mantido para reduzir churn nos testes existentes; novos call-sites devem usar
/// `emitir_resultado` com `caminho_saida` explícito.
pub fn emitir(saida: &SaidaBusca, formato: FormatoSaida) -> Result<()> {
    let formato_resolvido = resolver_formato_auto(formato, None);
    let texto = formatar_unica(saida, formato_resolvido)?;
    escrever_em_stdout(&texto)
}

/// Wrapper retrocompatível para multi-query.
pub fn emitir_multipla(saida: &SaidaBuscaMultipla, formato: FormatoSaida) -> Result<()> {
    let formato_resolvido = resolver_formato_auto(formato, None);
    let texto = formatar_multipla(saida, formato_resolvido)?;
    escrever_em_stdout(&texto)
}

/// Resolve `FormatoSaida::Auto` no formato concreto baseado em TTY detection.
///
/// - Saindo para arquivo (`caminho_saida = Some`) → JSON (estável e parseável).
/// - Auto + stdout TTY → Text (ergonômico para humanos).
/// - Auto + stdout pipe → JSON (consumo programático).
fn resolver_formato_auto(formato: FormatoSaida, caminho_saida: Option<&Path>) -> FormatoSaida {
    match formato {
        FormatoSaida::Auto => {
            if caminho_saida.is_some() {
                FormatoSaida::Json
            } else if crate::platform::stdout_eh_tty() {
                FormatoSaida::Text
            } else {
                FormatoSaida::Json
            }
        }
        outro => outro,
    }
}

fn formatar_unica(saida: &SaidaBusca, formato: FormatoSaida) -> Result<String> {
    match formato {
        FormatoSaida::Json | FormatoSaida::Auto => {
            serde_json::to_string_pretty(saida).context("falha ao serializar SaidaBusca como JSON")
        }
        FormatoSaida::Text => Ok(formatar_unica_text(saida)),
        FormatoSaida::Markdown => Ok(formatar_unica_markdown(saida)),
    }
}

fn formatar_multipla(saida: &SaidaBuscaMultipla, formato: FormatoSaida) -> Result<String> {
    match formato {
        FormatoSaida::Json | FormatoSaida::Auto => serde_json::to_string_pretty(saida)
            .context("falha ao serializar SaidaBuscaMultipla como JSON"),
        FormatoSaida::Text => Ok(formatar_multipla_text(saida)),
        FormatoSaida::Markdown => Ok(formatar_multipla_markdown(saida)),
    }
}

/// Formato `text` para single-query — compacto, otimizado para LLM tokens.
///
/// ```text
/// Query: <query> | Engine: duckduckgo | Endpoint: html | Results: N
///
/// [1] <title>
///     <url>
///     <snippet>
///
/// [2] ...
/// ```
fn formatar_unica_text(saida: &SaidaBusca) -> String {
    let mut buffer = String::new();
    buffer.push_str(&formatar_cabecalho_text(saida));
    if saida.resultados.is_empty() {
        buffer.push_str("\n(sem resultados)\n");
        return buffer;
    }
    for resultado in &saida.resultados {
        buffer.push('\n');
        buffer.push_str(&formatar_resultado_text(resultado));
    }
    buffer
}

fn formatar_multipla_text(saida: &SaidaBuscaMultipla) -> String {
    let mut buffer = String::new();
    buffer.push_str(&format!(
        "Queries: {} | Parallel: {} | Timestamp: {}\n",
        saida.quantidade_queries, saida.paralelismo, saida.timestamp
    ));
    for (i, busca) in saida.buscas.iter().enumerate() {
        buffer.push_str(&format!("\n========== Query #{} ==========\n", i + 1));
        buffer.push_str(&formatar_unica_text(busca));
    }
    buffer
}

fn formatar_cabecalho_text(saida: &SaidaBusca) -> String {
    format!(
        "Query: {} | Engine: {} | Endpoint: {} | Results: {}\n",
        saida.query, saida.motor, saida.endpoint, saida.quantidade_resultados
    )
}

fn formatar_resultado_text(r: &ResultadoBusca) -> String {
    let mut bloco = String::new();
    bloco.push_str(&format!("[{}] {}\n", r.posicao, r.titulo));
    if let Some(original) = &r.titulo_original {
        if !original.is_empty() {
            bloco.push_str(&format!("    (original: {})\n", original));
        }
    }
    bloco.push_str(&format!("    {}\n", r.url));
    if let Some(snippet) = &r.snippet {
        if !snippet.is_empty() {
            bloco.push_str(&format!("    {}\n", snippet));
        }
    }
    bloco
}

/// Formato `markdown` para single-query — ideal para `.md` e GitHub.
///
/// ```markdown
/// # Resultados: <query>
///
/// **Motor:** duckduckgo | **Endpoint:** html | **Total:** N
///
/// ## 1. [<title>](<url>)
///
/// <snippet>
///
/// ---
///
/// ## 2. ...
/// ```
fn formatar_unica_markdown(saida: &SaidaBusca) -> String {
    let mut buffer = String::new();
    buffer.push_str(&format!("# Resultados: {}\n\n", saida.query));
    buffer.push_str(&format!(
        "**Motor:** {} | **Endpoint:** {} | **Total:** {}\n\n",
        saida.motor, saida.endpoint, saida.quantidade_resultados
    ));
    if saida.resultados.is_empty() {
        buffer.push_str("_Nenhum resultado encontrado._\n");
        return buffer;
    }
    for (i, r) in saida.resultados.iter().enumerate() {
        if i > 0 {
            buffer.push_str("---\n\n");
        }
        buffer.push_str(&format!(
            "## {}. [{}]({})\n\n",
            r.posicao,
            escapar_markdown(&r.titulo),
            r.url
        ));
        if let Some(original) = &r.titulo_original {
            if !original.is_empty() {
                buffer.push_str(&format!(
                    "_Título original: {}_\n\n",
                    escapar_markdown(original)
                ));
            }
        }
        if let Some(snippet) = &r.snippet {
            if !snippet.is_empty() {
                buffer.push_str(&format!("{}\n\n", escapar_markdown(snippet)));
            }
        }
        if let Some(url_exibicao) = &r.url_exibicao {
            if !url_exibicao.is_empty() {
                buffer.push_str(&format!("`{}`\n\n", url_exibicao));
            }
        }
    }
    buffer
}

fn formatar_multipla_markdown(saida: &SaidaBuscaMultipla) -> String {
    let mut buffer = String::new();
    buffer.push_str(&format!(
        "# Buscas Múltiplas ({} queries)\n\n",
        saida.quantidade_queries
    ));
    buffer.push_str(&format!(
        "**Paralelismo:** {} | **Timestamp:** {}\n\n",
        saida.paralelismo, saida.timestamp
    ));
    for (i, busca) in saida.buscas.iter().enumerate() {
        if i > 0 {
            buffer.push_str("\n---\n\n");
        }
        buffer.push_str(&formatar_unica_markdown(busca));
    }
    buffer
}

/// Escapa caracteres Markdown que poderiam quebrar a renderização em títulos
/// ou snippets. Conservador: escapa apenas `[`, `]`, `*` e backticks.
fn escapar_markdown(texto: &str) -> String {
    texto
        .replace('\\', "\\\\")
        .replace('*', "\\*")
        .replace('[', "\\[")
        .replace(']', "\\]")
        .replace('`', "\\`")
}

fn escrever_em_stdout(conteudo: &str) -> Result<()> {
    let stdout = io::stdout();
    let mut lock = stdout.lock();
    writeln!(lock, "{conteudo}").context("falha ao escrever em stdout")?;
    lock.flush().context("falha ao flushar stdout")?;
    Ok(())
}

/// Verifica se um `anyhow::Error` contém `io::ErrorKind::BrokenPipe` na cadeia
/// de causas. Broken pipe indica que o leitor do pipe fechou (ex: `| jaq`,
/// `| head`) — comportamento normal em pipelines Unix, NÃO um erro.
pub(crate) fn eh_broken_pipe(erro: &anyhow::Error) -> bool {
    erro.chain().any(|causa| {
        causa
            .downcast_ref::<std::io::Error>()
            .is_some_and(|io_err| io_err.kind() == std::io::ErrorKind::BrokenPipe)
    })
}

/// Público: imprime UMA linha terminada em `\n` em stdout, com flush imediato.
/// Usado por subcomandos auxiliares (ex: `init-config`) que precisam emitir JSON.
pub fn imprimir_linha_stdout(conteudo: &str) -> Result<()> {
    escrever_em_stdout(conteudo)
}

/// Público: emite uma `SaidaBusca` como UMA linha NDJSON (JSON compacto + `\n`).
///
/// Se `arquivo_saida = Some`, abre o arquivo em modo append e escreve — usado pelo
/// consumer do `--stream` multi-query para gravar streamando sem segurar tudo em memória.
/// Se `None`, escreve em stdout com flush imediato (para pipes em tempo real).
pub fn emitir_ndjson(saida: &crate::types::SaidaBusca, arquivo_saida: Option<&Path>) -> Result<()> {
    let linha =
        serde_json::to_string(saida).context("falha ao serializar SaidaBusca como NDJSON")?;
    match arquivo_saida {
        Some(caminho) => anexar_linha_em_arquivo(caminho, &linha),
        None => {
            let stdout = io::stdout();
            let mut lock = stdout.lock();
            writeln!(lock, "{linha}").context("falha ao escrever NDJSON em stdout")?;
            lock.flush().context("falha ao flushar stdout")?;
            Ok(())
        }
    }
}

/// Emite um bloco de texto (formato `text`) em streaming, representando UMA query.
pub fn emitir_stream_text(
    indice: usize,
    saida: &crate::types::SaidaBusca,
    arquivo_saida: Option<&Path>,
) -> Result<()> {
    let mut bloco = String::new();
    bloco.push_str(&format!("========== Query #{} ==========\n", indice + 1));
    bloco.push_str(&formatar_unica_text(saida));
    emitir_bloco_stream(&bloco, arquivo_saida)
}

/// Emite um bloco de Markdown em streaming, representando UMA query.
pub fn emitir_stream_markdown(
    indice: usize,
    saida: &crate::types::SaidaBusca,
    arquivo_saida: Option<&Path>,
) -> Result<()> {
    let mut bloco = String::new();
    if indice > 0 {
        bloco.push_str("\n---\n\n");
    }
    bloco.push_str(&formatar_unica_markdown(saida));
    emitir_bloco_stream(&bloco, arquivo_saida)
}

/// Emite `bloco` em stdout ou anexa ao arquivo indicado. Usado por streams text/md.
fn emitir_bloco_stream(bloco: &str, arquivo_saida: Option<&Path>) -> Result<()> {
    match arquivo_saida {
        Some(caminho) => anexar_linha_em_arquivo(caminho, bloco),
        None => {
            let stdout = io::stdout();
            let mut lock = stdout.lock();
            write!(lock, "{bloco}").context("falha ao escrever bloco streaming em stdout")?;
            lock.flush().context("falha ao flushar stdout")?;
            Ok(())
        }
    }
}

/// Anexa UMA linha a um arquivo (modo append + criação), aplicando 0o644 no Unix na
/// primeira criação. Cria diretórios pai se necessário.
fn anexar_linha_em_arquivo(caminho: &Path, linha: &str) -> Result<()> {
    use std::fs::OpenOptions;
    crate::paths::validar_caminho_saida(caminho)?;
    crate::paths::criar_diretorios_pai(caminho)?;
    let precisava_criar = !caminho.exists();
    let mut arquivo = OpenOptions::new()
        .create(true)
        .append(true)
        .open(caminho)
        .with_context(|| format!("falha ao abrir (append) {}", caminho.display()))?;
    writeln!(arquivo, "{linha}")
        .with_context(|| format!("falha ao escrever em {}", caminho.display()))?;
    arquivo
        .flush()
        .with_context(|| format!("falha ao flushar {}", caminho.display()))?;
    drop(arquivo);

    #[cfg(unix)]
    if precisava_criar {
        crate::paths::aplicar_permissoes_644(caminho)?;
    }
    #[cfg(not(unix))]
    let _ = precisava_criar;

    Ok(())
}

/// Escreve `conteudo` no `caminho`, criando diretórios pai se necessário.
/// Aplica permissões 0o644 no Unix (somente o dono escreve, todos leem).
fn escrever_em_arquivo(caminho: &Path, conteudo: &str) -> Result<()> {
    crate::paths::validar_caminho_saida(caminho)?;
    crate::paths::criar_diretorios_pai(caminho)?;
    fs::write(caminho, conteudo)
        .with_context(|| format!("falha ao gravar arquivo: {}", caminho.display()))?;

    crate::paths::aplicar_permissoes_644(caminho)?;

    tracing::info!(caminho = %caminho.display(), bytes = conteudo.len(), "saída gravada em arquivo");
    Ok(())
}

#[cfg(test)]
mod testes {
    use super::*;
    use crate::types::{MetadadosBusca, ResultadoBusca};

    fn saida_de_teste() -> SaidaBusca {
        SaidaBusca {
            query: "teste".to_string(),
            motor: "duckduckgo".to_string(),
            endpoint: "html".to_string(),
            timestamp: "2026-04-14T00:00:00+00:00".to_string(),
            regiao: "br-pt".to_string(),
            quantidade_resultados: 1,
            resultados: vec![ResultadoBusca {
                posicao: 1,
                titulo: "Título com [colchetes]".to_string(),
                url: "https://exemplo.com".to_string(),
                url_exibicao: Some("exemplo.com".to_string()),
                snippet: Some("Descrição com *asteriscos* e `backticks`".to_string()),
                titulo_original: None,
                conteudo: None,
                tamanho_conteudo: None,
                metodo_extracao_conteudo: None,
            }],
            paginas_buscadas: 1,
            erro: None,
            mensagem: None,
            metadados: MetadadosBusca {
                tempo_execucao_ms: 100,
                hash_seletores: "abc1234567890def".to_string(),
                retentativas: 0,
                usou_endpoint_fallback: false,
                fetches_simultaneos: 0,
                sucessos_fetch: 0,
                falhas_fetch: 0,
                usou_chrome: false,
                user_agent: "Mozilla/5.0".to_string(),
                usou_proxy: false,
            },
        }
    }

    #[test]
    fn resolver_formato_auto_para_arquivo_sempre_json() {
        let caminho = Path::new("/tmp/teste.json");
        assert_eq!(
            resolver_formato_auto(FormatoSaida::Auto, Some(caminho)),
            FormatoSaida::Json
        );
    }

    #[test]
    fn resolver_formato_auto_preserva_formatos_concretos() {
        assert_eq!(
            resolver_formato_auto(FormatoSaida::Json, None),
            FormatoSaida::Json
        );
        assert_eq!(
            resolver_formato_auto(FormatoSaida::Text, None),
            FormatoSaida::Text
        );
        assert_eq!(
            resolver_formato_auto(FormatoSaida::Markdown, None),
            FormatoSaida::Markdown
        );
    }

    #[test]
    fn formatar_unica_text_inclui_query_e_resultados() {
        let saida = saida_de_teste();
        let texto = formatar_unica_text(&saida);
        assert!(texto.contains("Query: teste"));
        assert!(texto.contains("Engine: duckduckgo"));
        assert!(texto.contains("Endpoint: html"));
        assert!(texto.contains("Results: 1"));
        assert!(texto.contains("[1] Título com [colchetes]"));
        assert!(texto.contains("https://exemplo.com"));
        assert!(texto.contains("Descrição com *asteriscos*"));
    }

    #[test]
    fn formatar_unica_text_lida_com_zero_resultados() {
        let mut saida = saida_de_teste();
        saida.quantidade_resultados = 0;
        saida.resultados = vec![];
        let texto = formatar_unica_text(&saida);
        assert!(texto.contains("Results: 0"));
        assert!(texto.contains("(sem resultados)"));
    }

    #[test]
    fn formatar_unica_markdown_inclui_titulo_h1_e_links() {
        let saida = saida_de_teste();
        let md = formatar_unica_markdown(&saida);
        assert!(md.starts_with("# Resultados: teste\n\n"));
        assert!(md.contains("**Motor:** duckduckgo"));
        assert!(md.contains("**Total:** 1"));
        // Título com colchetes deve ser escapado.
        assert!(md.contains("[Título com \\[colchetes\\]](https://exemplo.com)"));
        // Snippet com asteriscos e backticks devem ser escapados.
        assert!(md.contains("Descrição com \\*asteriscos\\* e \\`backticks\\`"));
        // url_exibicao deve aparecer entre crases.
        assert!(md.contains("`exemplo.com`"));
    }

    #[test]
    fn formatar_unica_markdown_sem_resultados_emite_aviso() {
        let mut saida = saida_de_teste();
        saida.quantidade_resultados = 0;
        saida.resultados = vec![];
        let md = formatar_unica_markdown(&saida);
        assert!(md.contains("# Resultados: teste"));
        assert!(md.contains("_Nenhum resultado encontrado._"));
    }

    #[test]
    fn formatar_resultado_com_titulo_original_exibe_anotacao_text() {
        // Heurística "Official site": titulo foi substituído por url_exibicao,
        // titulo_original preserva o texto literal. Ambos devem aparecer no text.
        let mut saida = saida_de_teste();
        saida.resultados = vec![ResultadoBusca {
            posicao: 1,
            titulo: "saofidelis.rj.gov.br".to_string(),
            url: "https://saofidelis.rj.gov.br".to_string(),
            url_exibicao: Some("saofidelis.rj.gov.br".to_string()),
            snippet: Some("Prefeitura de São Fidélis".to_string()),
            titulo_original: Some("Official site".to_string()),
            conteudo: None,
            tamanho_conteudo: None,
            metodo_extracao_conteudo: None,
        }];
        let texto = formatar_unica_text(&saida);
        assert!(texto.contains("[1] saofidelis.rj.gov.br"));
        assert!(
            texto.contains("(original: Official site)"),
            "text deve exibir titulo_original quando presente"
        );
    }

    #[test]
    fn formatar_resultado_com_titulo_original_exibe_anotacao_markdown() {
        let mut saida = saida_de_teste();
        saida.resultados = vec![ResultadoBusca {
            posicao: 1,
            titulo: "saofidelis.rj.gov.br".to_string(),
            url: "https://saofidelis.rj.gov.br".to_string(),
            url_exibicao: Some("saofidelis.rj.gov.br".to_string()),
            snippet: Some("Prefeitura".to_string()),
            titulo_original: Some("Official site".to_string()),
            conteudo: None,
            tamanho_conteudo: None,
            metodo_extracao_conteudo: None,
        }];
        let md = formatar_unica_markdown(&saida);
        assert!(md.contains("[saofidelis.rj.gov.br](https://saofidelis.rj.gov.br)"));
        assert!(
            md.contains("_Título original: Official site_"),
            "markdown deve exibir titulo_original em itálico quando presente"
        );
    }

    #[test]
    fn formatar_resultado_sem_titulo_original_nao_emite_anotacao() {
        // titulo_original = None → nenhum ruído no output.
        let saida = saida_de_teste();
        let texto = formatar_unica_text(&saida);
        let md = formatar_unica_markdown(&saida);
        assert!(!texto.contains("(original:"));
        assert!(!md.contains("_Título original:"));
    }

    #[test]
    fn json_omite_titulo_original_quando_ausente() {
        // skip_serializing_if = "Option::is_none" garante que o campo não
        // aparece no JSON quando None — preserva a compatibilidade mínima.
        let saida = saida_de_teste();
        let json = serde_json::to_string(&saida).expect("serializa");
        assert!(
            !json.contains("titulo_original"),
            "JSON não deve expor titulo_original quando é None"
        );
    }

    #[test]
    fn json_inclui_titulo_original_quando_presente() {
        let mut saida = saida_de_teste();
        saida.resultados[0].titulo_original = Some("Official site".to_string());
        let json = serde_json::to_string(&saida).expect("serializa");
        assert!(json.contains("\"titulo_original\":\"Official site\""));
    }

    #[test]
    fn json_nao_contem_mais_campo_buscas_relacionadas() {
        // Regressão v0.3.0: schema perdeu `buscas_relacionadas` (BREAKING).
        let saida = saida_de_teste();
        let json = serde_json::to_string(&saida).expect("serializa");
        assert!(
            !json.contains("buscas_relacionadas"),
            "v0.3.0 removeu buscas_relacionadas do schema JSON"
        );
    }

    #[test]
    fn formatar_multipla_text_inclui_separadores_por_query() {
        let saida = SaidaBuscaMultipla {
            quantidade_queries: 2,
            timestamp: "2026-04-14T00:00:00+00:00".to_string(),
            paralelismo: 3,
            buscas: vec![saida_de_teste(), saida_de_teste()],
        };
        let texto = formatar_multipla_text(&saida);
        assert!(texto.contains("Queries: 2"));
        assert!(texto.contains("Parallel: 3"));
        assert!(texto.contains("========== Query #1 =========="));
        assert!(texto.contains("========== Query #2 =========="));
    }

    #[test]
    fn formatar_multipla_markdown_inclui_h1_geral() {
        let saida = SaidaBuscaMultipla {
            quantidade_queries: 2,
            timestamp: "2026-04-14T00:00:00+00:00".to_string(),
            paralelismo: 3,
            buscas: vec![saida_de_teste(), saida_de_teste()],
        };
        let md = formatar_multipla_markdown(&saida);
        assert!(md.starts_with("# Buscas Múltiplas (2 queries)"));
        assert!(md.contains("**Paralelismo:** 3"));
        // Cada busca interna deve aparecer com seu próprio H1.
        assert_eq!(md.matches("# Resultados: teste").count(), 2);
    }

    #[test]
    fn escapar_markdown_protege_caracteres_problematicos() {
        assert_eq!(escapar_markdown("a*b"), "a\\*b");
        assert_eq!(escapar_markdown("a[b]"), "a\\[b\\]");
        assert_eq!(escapar_markdown("a`b"), "a\\`b");
        assert_eq!(escapar_markdown("texto normal"), "texto normal");
    }

    #[test]
    fn escrever_em_arquivo_cria_diretorios_pai() {
        let temp = std::env::temp_dir().join(format!("ddgcli-output-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&temp);
        let arquivo = temp.join("sub").join("nested").join("saida.txt");
        escrever_em_arquivo(&arquivo, "conteudo de teste\nlinha 2\n")
            .expect("deve gravar arquivo com diretórios pai");
        let lido = fs::read_to_string(&arquivo).expect("arquivo deve existir");
        assert_eq!(lido, "conteudo de teste\nlinha 2\n");
        fs::remove_dir_all(&temp).ok();
    }

    #[cfg(unix)]
    #[test]
    fn escrever_em_arquivo_aplica_permissoes_644_no_unix() {
        use std::os::unix::fs::PermissionsExt;
        let arquivo =
            std::env::temp_dir().join(format!("ddgcli-perms-test-{}.txt", std::process::id()));
        let _ = fs::remove_file(&arquivo);
        escrever_em_arquivo(&arquivo, "x").expect("deve gravar");
        let metadata = fs::metadata(&arquivo).expect("deve obter metadata");
        let modo = metadata.permissions().mode() & 0o777;
        assert_eq!(modo, 0o644, "permissões devem ser 0o644 (foi {modo:o})");
        fs::remove_file(&arquivo).ok();
    }

    #[test]
    fn emitir_json_single_via_serde_continua_estavel() {
        // Garantia de regressão: serialização JSON do struct não muda.
        let saida = saida_de_teste();
        let json = serde_json::to_string_pretty(&saida).expect("serialização deve funcionar");
        assert!(json.contains("\"query\": \"teste\""));
        assert!(json.contains("\"quantidade_resultados\": 1"));
        assert!(json.contains("\"motor\": \"duckduckgo\""));
    }

    // -----------------------------------------------------------------------
    // Cobertura dos caminhos de streaming/arquivo
    // -----------------------------------------------------------------------

    #[test]
    fn emitir_ndjson_em_arquivo_escreve_linha_unica_parseavel() {
        let dir = tempfile::tempdir().expect("tempdir");
        let arquivo = dir.path().join("ndjson.log");
        let saida = saida_de_teste();
        emitir_ndjson(&saida, Some(&arquivo)).expect("ndjson deve gravar");
        let conteudo = fs::read_to_string(&arquivo).expect("ler arquivo");
        let linhas: Vec<&str> = conteudo.lines().filter(|l| !l.trim().is_empty()).collect();
        assert_eq!(linhas.len(), 1, "NDJSON = 1 linha por chamada");
        let _: serde_json::Value =
            serde_json::from_str(linhas[0]).expect("linha NDJSON deve ser JSON válido");
    }

    #[test]
    fn emitir_ndjson_duas_chamadas_anexam_sem_truncar() {
        let dir = tempfile::tempdir().expect("tempdir");
        let arquivo = dir.path().join("ndjson.log");
        let saida = saida_de_teste();
        emitir_ndjson(&saida, Some(&arquivo)).expect("1ª gravação");
        emitir_ndjson(&saida, Some(&arquivo)).expect("2ª gravação (append)");
        let conteudo = fs::read_to_string(&arquivo).expect("ler");
        let linhas: Vec<&str> = conteudo.lines().filter(|l| !l.trim().is_empty()).collect();
        assert_eq!(linhas.len(), 2, "modo append: 2 chamadas = 2 linhas");
    }

    #[test]
    fn emitir_ndjson_cria_diretorios_pai_quando_ausentes() {
        let dir = tempfile::tempdir().expect("tempdir");
        // Caminho com 2 níveis inexistentes.
        let arquivo = dir.path().join("sub/outro/out.ndjson");
        assert!(!arquivo.parent().unwrap().exists());
        emitir_ndjson(&saida_de_teste(), Some(&arquivo)).expect("deve criar pais");
        assert!(arquivo.exists(), "arquivo criado");
        assert!(arquivo.parent().unwrap().exists(), "diretório pai criado");
    }

    #[test]
    fn emitir_stream_text_em_arquivo_inclui_cabecalho_da_query() {
        let dir = tempfile::tempdir().expect("tempdir");
        let arquivo = dir.path().join("stream.txt");
        emitir_stream_text(0, &saida_de_teste(), Some(&arquivo)).expect("stream text");
        emitir_stream_text(1, &saida_de_teste(), Some(&arquivo)).expect("stream text 2");
        let conteudo = fs::read_to_string(&arquivo).expect("ler");
        assert!(conteudo.contains("========== Query #1 =========="));
        assert!(conteudo.contains("========== Query #2 =========="));
        assert!(conteudo.contains("Query: teste"));
    }

    #[test]
    fn emitir_stream_markdown_separa_queries_com_divisor_a_partir_da_segunda() {
        let dir = tempfile::tempdir().expect("tempdir");
        let arquivo = dir.path().join("stream.md");
        emitir_stream_markdown(0, &saida_de_teste(), Some(&arquivo)).expect("1ª");
        emitir_stream_markdown(1, &saida_de_teste(), Some(&arquivo)).expect("2ª");
        let conteudo = fs::read_to_string(&arquivo).expect("ler");
        // Separador "\n---\n" deve aparecer APENAS entre blocos (uma vez para 2 queries).
        let ocorrencias = conteudo.matches("\n---\n").count();
        assert_eq!(
            ocorrencias, 1,
            "divisor apenas entre queries (1 para 2 blocos)"
        );
        assert!(conteudo.contains("# Resultados: teste"));
    }

    #[test]
    fn emitir_resultado_stream_e_noop_e_nao_cria_arquivo() {
        use crate::parallel::EstatisticasStream;
        use crate::pipeline::ResultadoPipeline;
        let dir = tempfile::tempdir().expect("tempdir");
        let arquivo = dir.path().join("nao-cria.json");
        let stream_stats = EstatisticasStream {
            total: 3,
            sucessos: 3,
            erros: 0,
            timestamp_inicio: "2026-04-14T00:00:00Z".to_string(),
            paralelismo: 2,
        };
        let res = ResultadoPipeline::Stream(stream_stats);
        emitir_resultado(&res, FormatoSaida::Json, Some(&arquivo)).expect("no-op OK");
        assert!(
            !arquivo.exists(),
            "Stream não deve escrever nada em emitir_resultado"
        );
    }

    #[test]
    fn emitir_resultado_unica_em_arquivo_escreve_json_formatado() {
        use crate::pipeline::ResultadoPipeline;
        let dir = tempfile::tempdir().expect("tempdir");
        let arquivo = dir.path().join("saida.json");
        let res = ResultadoPipeline::Unica(Box::new(saida_de_teste()));
        emitir_resultado(&res, FormatoSaida::Json, Some(&arquivo)).expect("emitir");
        let conteudo = fs::read_to_string(&arquivo).expect("ler");
        let _: serde_json::Value =
            serde_json::from_str(&conteudo).expect("conteúdo deve ser JSON válido");
        assert!(conteudo.contains("\"query\": \"teste\""));
    }

    #[test]
    fn emitir_resultado_multipla_text_em_arquivo_contem_ambas_queries() {
        use crate::pipeline::ResultadoPipeline;
        use crate::types::SaidaBuscaMultipla;
        let dir = tempfile::tempdir().expect("tempdir");
        let arquivo = dir.path().join("multi.txt");
        let mut saida1 = saida_de_teste();
        saida1.query = "alpha".into();
        let mut saida2 = saida_de_teste();
        saida2.query = "beta".into();
        let multi = SaidaBuscaMultipla {
            quantidade_queries: 2,
            timestamp: "2026-04-14T00:00:00Z".into(),
            paralelismo: 2,
            buscas: vec![saida1, saida2],
        };
        let res = ResultadoPipeline::Multipla(Box::new(multi));
        emitir_resultado(&res, FormatoSaida::Text, Some(&arquivo)).expect("emitir");
        let conteudo = fs::read_to_string(&arquivo).expect("ler");
        assert!(conteudo.contains("Query: alpha"));
        assert!(conteudo.contains("Query: beta"));
    }

    #[test]
    fn emitir_resultado_auto_em_arquivo_escreve_json() {
        // Auto + arquivo → JSON (determinístico, não depende de TTY).
        use crate::pipeline::ResultadoPipeline;
        let dir = tempfile::tempdir().expect("tempdir");
        let arquivo = dir.path().join("auto.out");
        let res = ResultadoPipeline::Unica(Box::new(saida_de_teste()));
        emitir_resultado(&res, FormatoSaida::Auto, Some(&arquivo)).expect("emitir");
        let conteudo = fs::read_to_string(&arquivo).expect("ler");
        // JSON começa com `{` e tem "query".
        assert!(conteudo.trim_start().starts_with('{'));
        assert!(conteudo.contains("\"query\""));
    }

    #[test]
    fn eh_broken_pipe_detecta_broken_pipe_direto() {
        let io_err = std::io::Error::new(std::io::ErrorKind::BrokenPipe, "pipe fechado");
        let anyhow_err = anyhow::Error::new(io_err);
        assert!(eh_broken_pipe(&anyhow_err));
    }

    #[test]
    fn eh_broken_pipe_rejeita_outros_erros_io() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "arquivo não encontrado");
        let anyhow_err = anyhow::Error::new(io_err);
        assert!(!eh_broken_pipe(&anyhow_err));
    }

    #[test]
    fn eh_broken_pipe_detecta_broken_pipe_aninhado_em_context() {
        let io_err = std::io::Error::new(std::io::ErrorKind::BrokenPipe, "pipe");
        let anyhow_err = anyhow::Error::new(io_err).context("falha ao escrever em stdout");
        assert!(eh_broken_pipe(&anyhow_err));
    }

    #[test]
    fn eh_broken_pipe_rejeita_erro_sem_io_error() {
        let anyhow_err = anyhow::anyhow!("erro genérico sem IO");
        assert!(!eh_broken_pipe(&anyhow_err));
    }
}
