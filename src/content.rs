//! Extração de conteúdo textual completo de URLs (flag `--fetch-content`).
//!
//! Implementação HTTP puro (iteração 5). Para cada URL:
//! 1. Faz request HTTP com `reqwest::Client`.
//! 2. Verifica `Content-Type` — aceita apenas `text/html` e variantes.
//! 3. Lê body como `Vec<u8>`, detecta charset via header e converte para UTF-8
//!    com `encoding_rs` (fallback `from_utf8_lossy` para UTF-8/ausente).
//! 4. Parseia com `scraper` e aplica readability simplificado (5 passos):
//!    - Remove elementos de chrome (nav, header, footer, script, style, aside, forms).
//!    - Identifica container principal (article → main → [role=main] → body).
//!    - Extrai texto de blocos relevantes (p, h1-6, li, blockquote, pre, td).
//!    - Limpa (whitespace excessivo, linhas curtas).
//!    - Trunca em `tamanho_max` respeitando limites de palavra.
//! 5. Se texto limpo < 200 chars → retorna string vazia sinalizando que
//!    provavelmente precisa de Chrome (iteração 6).
//!
//! Fallback via Chrome headless virá em iteração 6 sob feature `chrome`.

use anyhow::Result;
use reqwest::Client;
use scraper::{Html, Selector};
use tokio_util::sync::CancellationToken;

/// Limiar abaixo do qual consideramos o conteúdo "insuficiente" (candidato a fallback Chrome).
const LIMIAR_CONTEUDO_MINIMO: usize = 200;

/// Limiar de caracteres por linha para descartar linhas muito curtas (ex: boilerplate de navegação).
const LIMIAR_LINHA_MINIMA: usize = 20;

/// Extrai o conteúdo textual principal de uma URL via HTTP puro.
///
/// Retorna:
/// - `Ok(Some((texto_limpo, tamanho_original_em_bytes)))` em sucesso.
/// - `Ok(None)` se o `Content-Type` não for HTML (pdf, image, etc.).
/// - `Err` em falha de rede/parse irrecuperável.
///
/// O texto retornado pode ser vazio se a extração não produziu conteúdo > 200 chars —
/// nesse caso o chamador sabe que seria necessário fallback via Chrome.
pub async fn extrair_conteudo_http(
    cliente: &Client,
    url: &str,
    tamanho_max: usize,
    token: &CancellationToken,
) -> Result<Option<(String, u32)>> {
    if token.is_cancelled() {
        anyhow::bail!("extração cancelada para {url:?}");
    }

    tracing::debug!(url, "iniciando extração de conteúdo HTTP");

    // Request com future racing contra cancelamento.
    let resposta = tokio::select! {
        biased;
        _ = token.cancelled() => {
            anyhow::bail!("extração cancelada durante request de {url:?}");
        }
        resultado = cliente.get(url).send() => resultado?
    };

    if !resposta.status().is_success() {
        tracing::debug!(url, status = %resposta.status(), "status HTTP não-sucesso — descartando");
        return Ok(None);
    }

    // Extrai charset do Content-Type ANTES de consumir o body.
    let content_type = resposta
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string();

    if !eh_html(&content_type) {
        tracing::debug!(url, content_type, "Content-Type não é HTML — descartando");
        return Ok(None);
    }

    let charset = extrair_charset(&content_type);
    let bytes = tokio::select! {
        biased;
        _ = token.cancelled() => {
            anyhow::bail!("extração cancelada durante leitura de body de {url:?}");
        }
        resultado = resposta.bytes() => resultado?
    };

    let tamanho_original = u32::try_from(bytes.len()).unwrap_or(u32::MAX);
    tracing::debug!(url, tamanho = bytes.len(), "body baixado");

    // Decodifica para UTF-8 usando encoding_rs + fallback lossy.
    let html_utf8 = decodificar_para_utf8(&bytes, charset.as_deref());

    // Parse + readability rodam em blocking pool: scraper usa Rc<_> internamente
    // (html5ever) e NÃO é Send. spawn_blocking move-nos para thread pool dedicada.
    let tamanho_max_local = tamanho_max;
    let texto_limpo =
        tokio::task::spawn_blocking(move || aplicar_readability(&html_utf8, tamanho_max_local))
            .await
            .map_err(|erro| anyhow::anyhow!("task de readability panicou: {erro}"))?;

    if texto_limpo.len() < LIMIAR_CONTEUDO_MINIMO {
        tracing::debug!(
            url,
            len = texto_limpo.len(),
            "conteúdo extraído abaixo do limiar — sinalizando possível necessidade de Chrome"
        );
        // Retorna string vazia + tamanho original para sinalização (iteração 6 fará fallback).
        return Ok(Some((String::new(), tamanho_original)));
    }

    tracing::debug!(url, tamanho_limpo = texto_limpo.len(), "extração concluída");
    Ok(Some((texto_limpo, tamanho_original)))
}

/// Verifica se o Content-Type corresponde a HTML (flexível para `text/html; charset=...`).
fn eh_html(content_type: &str) -> bool {
    let lower = content_type.to_ascii_lowercase();
    lower.starts_with("text/html") || lower.starts_with("application/xhtml+xml")
}

/// Extrai o valor de `charset=` de um Content-Type (se presente).
fn extrair_charset(content_type: &str) -> Option<String> {
    for parte in content_type.split(';') {
        let trimmed = parte.trim();
        if let Some(valor) = trimmed.strip_prefix("charset=") {
            let limpo = valor.trim_matches(|c: char| c == '"' || c == '\'');
            if !limpo.is_empty() {
                return Some(limpo.to_ascii_lowercase());
            }
        }
    }
    None
}

/// Decodifica bytes para `String` UTF-8 usando charset declarado (se fornecido).
///
/// - Se `charset` for UTF-8 ou ausente → `from_utf8_lossy` (rápido).
/// - Senão → `Encoding::for_label().decode()` com fallback WINDOWS-1252 em label desconhecido.
pub fn decodificar_para_utf8(bytes: &[u8], charset: Option<&str>) -> String {
    let label = charset.unwrap_or("utf-8");
    if label == "utf-8" || label == "utf8" || label.is_empty() {
        return String::from_utf8_lossy(bytes).into_owned();
    }

    match encoding_rs::Encoding::for_label(label.as_bytes()) {
        Some(enc) => {
            let (cow, _used, _had_errors) = enc.decode(bytes);
            cow.into_owned()
        }
        None => {
            tracing::debug!(
                charset = label,
                "label de charset desconhecido — fallback UTF-8 lossy"
            );
            String::from_utf8_lossy(bytes).into_owned()
        }
    }
}

/// Aplica readability simplificado em 5 passos sobre HTML UTF-8.
///
/// Retorna texto limpo truncado em `tamanho_max` caracteres (respeitando palavra).
/// Chamada de dentro de `spawn_blocking` porque `scraper::Html` não é `Send`.
fn aplicar_readability(html: &str, tamanho_max: usize) -> String {
    let documento = Html::parse_document(html);

    // Passo 1: lista de seletores CSS que DEVEM ser IGNORADOS (chrome/navegação/scripts).
    // scraper não suporta remoção in-place fácil, então ao invés coletamos SEMÂNTICA de
    // "elementos válidos dentro do container principal ignorando descendentes de chrome".
    // Estratégia: encontramos container principal, iteramos blocos de texto DESDE QUE
    // nenhum ancestral seja elemento de chrome.

    // Passo 2: identifica container principal.
    let seletores_container: [&str; 8] = [
        "article",
        "main",
        "[role=\"main\"]",
        ".post-content",
        ".article-body",
        ".entry-content",
        "#content",
        ".content",
    ];

    let mut container_ref = None;
    for sel_str in &seletores_container {
        if let Ok(sel) = Selector::parse(sel_str) {
            if let Some(primeiro) = documento.select(&sel).next() {
                container_ref = Some(primeiro);
                break;
            }
        }
    }

    // Fallback: body inteiro.
    let container = match container_ref {
        Some(c) => c,
        None => match Selector::parse("body")
            .ok()
            .and_then(|s| documento.select(&s).next())
        {
            Some(b) => b,
            None => return String::new(),
        },
    };

    // Passo 3: extrai texto de blocos relevantes dentro do container.
    // Seletores de blocos que aceitamos como conteúdo.
    let blocos = match Selector::parse("p, h1, h2, h3, h4, h5, h6, li, blockquote, pre, td, th") {
        Ok(s) => s,
        Err(_) => return String::new(),
    };

    // Seletores de elementos IGNORADOS — se algum ancestral for desse tipo, pulamos.
    // `scraper` não nos dá iteração direta de ancestrais — simulamos checando tags pai.
    // Estratégia simples: para cada bloco, sobe pela cadeia até o root e descarta se
    // encontrar uma tag proibida.
    let tags_proibidas: &[&str] = &[
        "nav", "header", "footer", "aside", "script", "style", "noscript", "iframe", "svg", "form",
    ];
    let classes_proibidas: &[&str] = &[
        "sidebar",
        "nav",
        "menu",
        "footer",
        "header",
        "ad",
        "advertisement",
        "social-share",
    ];
    let roles_proibidas: &[&str] = &["navigation", "banner", "contentinfo"];

    let mut linhas: Vec<String> = Vec::new();
    for bloco in container.select(&blocos) {
        if ancestral_eh_chrome(bloco, tags_proibidas, classes_proibidas, roles_proibidas) {
            continue;
        }
        // Junta o texto descendente com espaços.
        let texto: String = bloco
            .text()
            .collect::<Vec<_>>()
            .join(" ")
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ");
        if !texto.is_empty() {
            linhas.push(texto);
        }
    }

    // Passo 4: limpeza — linhas curtas descartadas, normaliza espaços entre linhas.
    let conteudo: String = linhas
        .into_iter()
        .filter(|l| l.chars().count() >= LIMIAR_LINHA_MINIMA)
        .collect::<Vec<_>>()
        .join("\n");

    // Passo 5: trunca em tamanho_max caracteres respeitando palavra.
    truncar_em_palavra(&conteudo, tamanho_max)
}

/// Verifica se um elemento (ou algum ancestral) corresponde às categorias "chrome".
///
/// Usa navegação pela árvore via `parent()` até chegar no root (Document).
fn ancestral_eh_chrome(
    elemento: scraper::ElementRef<'_>,
    tags: &[&str],
    classes: &[&str],
    roles: &[&str],
) -> bool {
    // O próprio elemento já entrou no seletor de blocos (p/h1/etc), mas pode estar
    // aninhado dentro de um nav/header. Subimos pela cadeia de pais.
    let mut atual_no = elemento.parent();
    while let Some(no) = atual_no {
        if let Some(el) = scraper::ElementRef::wrap(no) {
            let nome = el.value().name();
            if tags.iter().any(|t| t.eq_ignore_ascii_case(nome)) {
                return true;
            }
            if let Some(class_attr) = el.value().attr("class") {
                for c in class_attr.split_ascii_whitespace() {
                    if classes
                        .iter()
                        .any(|proibida| c.eq_ignore_ascii_case(proibida))
                    {
                        return true;
                    }
                }
            }
            if let Some(role) = el.value().attr("role") {
                if roles.iter().any(|r| r.eq_ignore_ascii_case(role)) {
                    return true;
                }
            }
        }
        atual_no = no.parent();
    }
    false
}

/// Trunca `texto` em `tamanho_max` caracteres respeitando fronteiras de palavra.
///
/// Se o corte cair no meio de uma palavra, recua até o último whitespace.
/// Se não há whitespace, faz corte hard no byte mais próximo de caractere válido.
fn truncar_em_palavra(texto: &str, tamanho_max: usize) -> String {
    if tamanho_max == 0 {
        return String::new();
    }
    let contado: usize = texto.chars().count();
    if contado <= tamanho_max {
        return texto.to_string();
    }

    // Pega os primeiros `tamanho_max` chars, depois recua até o último whitespace.
    let prefixo: String = texto.chars().take(tamanho_max).collect();
    if let Some(pos) = prefixo.rfind(char::is_whitespace) {
        return prefixo[..pos].trim_end().to_string();
    }
    prefixo
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn eh_html_aceita_text_html_e_variantes() {
        assert!(eh_html("text/html"));
        assert!(eh_html("text/html; charset=utf-8"));
        assert!(eh_html("application/xhtml+xml"));
        assert!(eh_html("TEXT/HTML"));
    }

    #[test]
    fn eh_html_rejeita_nao_html() {
        assert!(!eh_html("application/pdf"));
        assert!(!eh_html("image/png"));
        assert!(!eh_html("application/json"));
        assert!(!eh_html(""));
    }

    #[test]
    fn extrair_charset_identifica_utf8() {
        assert_eq!(
            extrair_charset("text/html; charset=UTF-8"),
            Some("utf-8".to_string())
        );
        assert_eq!(
            extrair_charset("text/html; charset=\"iso-8859-1\""),
            Some("iso-8859-1".to_string())
        );
    }

    #[test]
    fn extrair_charset_ausente_retorna_none() {
        assert_eq!(extrair_charset("text/html"), None);
        assert_eq!(extrair_charset(""), None);
    }

    #[test]
    fn decodificar_utf8_puro() {
        let bytes = "olá mundo".as_bytes();
        let s = decodificar_para_utf8(bytes, None);
        assert_eq!(s, "olá mundo");
        let s2 = decodificar_para_utf8(bytes, Some("utf-8"));
        assert_eq!(s2, "olá mundo");
    }

    #[test]
    fn decodificar_latin1_para_utf8() {
        // 'á' em Latin-1 é byte 0xE1.
        let bytes: &[u8] = &[0xE1, 0x6C, 0x6F];
        let s = decodificar_para_utf8(bytes, Some("iso-8859-1"));
        assert_eq!(s, "álo");
    }

    #[test]
    fn decodificar_windows1252_para_utf8() {
        // 'ç' em Windows-1252 é byte 0xE7.
        let bytes: &[u8] = &[0xE7];
        let s = decodificar_para_utf8(bytes, Some("windows-1252"));
        assert_eq!(s, "ç");
    }

    #[test]
    fn decodificar_charset_desconhecido_cai_em_utf8_lossy() {
        let bytes = "teste".as_bytes();
        let s = decodificar_para_utf8(bytes, Some("charset-que-nao-existe"));
        assert_eq!(s, "teste");
    }

    #[test]
    fn truncar_em_palavra_preserva_fronteira() {
        let texto = "uma frase qualquer com várias palavras";
        let t = truncar_em_palavra(texto, 10);
        assert!(t.len() <= 10);
        assert!(!t.ends_with(' '));
        // Não deve cortar no meio de uma palavra.
        assert!(
            texto.starts_with(&t),
            "truncado ({t:?}) deve ser prefixo do original"
        );
    }

    #[test]
    fn truncar_em_palavra_texto_curto_retorna_original() {
        assert_eq!(truncar_em_palavra("oi", 100), "oi");
        assert_eq!(truncar_em_palavra("", 100), "");
    }

    #[test]
    fn truncar_em_palavra_sem_whitespace_corta_hard() {
        let t = truncar_em_palavra("palavraSemEspacoNenhum", 10);
        assert_eq!(t.chars().count(), 10);
    }

    #[test]
    fn readability_extrai_artigo_simples() {
        let html = r#"<html><body>
            <nav><a href="/">Menu</a></nav>
            <article>
              <h1>Título do Artigo</h1>
              <p>Este é o primeiro parágrafo do artigo com pelo menos vinte caracteres de conteúdo substantivo.</p>
              <p>Segundo parágrafo também com conteúdo suficiente para passar do limiar de linha mínima.</p>
            </article>
            <footer>Copyright</footer>
            </body></html>"#;
        let texto = aplicar_readability(html, 1000);
        assert!(texto.contains("primeiro parágrafo"));
        assert!(texto.contains("Segundo parágrafo"));
        // Navegação e footer devem ser omitidos.
        assert!(!texto.contains("Menu"));
        assert!(!texto.contains("Copyright"));
    }

    #[test]
    fn readability_usa_main_quando_nao_ha_article() {
        let html = r#"<html><body>
            <header>Cabeçalho irrelevante</header>
            <main>
              <p>Conteúdo principal via tag main, com mais de vinte caracteres de texto útil aqui.</p>
              <p>Outro parágrafo relevante com conteúdo suficiente para não ser descartado.</p>
            </main>
            </body></html>"#;
        let texto = aplicar_readability(html, 1000);
        assert!(texto.contains("Conteúdo principal"));
        assert!(texto.contains("Outro parágrafo"));
        assert!(!texto.contains("Cabeçalho"));
    }

    #[test]
    fn readability_remove_script_style_nav() {
        let html = r#"<html><body>
            <nav><p>Este parágrafo dentro da nav deve ser descartado porque é chrome.</p></nav>
            <article>
              <script>var x = 1;</script>
              <style>.a { color: red; }</style>
              <p>Parágrafo legítimo dentro de article com conteúdo o bastante para passar o limiar.</p>
            </article>
            </body></html>"#;
        let texto = aplicar_readability(html, 1000);
        assert!(texto.contains("Parágrafo legítimo"));
        assert!(!texto.contains("dentro da nav"));
        assert!(!texto.contains("var x = 1"));
        assert!(!texto.contains("color: red"));
    }

    #[test]
    fn readability_trunca_em_tamanho_max() {
        let conteudo_longo = "Parágrafo um com pelo menos vinte caracteres aqui.\n".repeat(100);
        let html = format!("<html><body><article><p>{conteudo_longo}</p></article></body></html>");
        let texto = aplicar_readability(&html, 200);
        assert!(texto.chars().count() <= 200);
    }

    #[test]
    fn readability_retorna_vazio_sem_conteudo_suficiente() {
        // Apenas nav e footer — nada no main/article.
        let html = r#"<html><body>
            <nav>Menu curto</nav>
            <footer>Rodapé breve.</footer>
            </body></html>"#;
        let texto = aplicar_readability(html, 1000);
        // Deve ser string vazia (ou muito curta), sinalizando fallback necessário.
        assert!(
            texto.len() < LIMIAR_CONTEUDO_MINIMO,
            "sem conteúdo substantivo esperado, obtido: {texto:?}"
        );
    }
}
