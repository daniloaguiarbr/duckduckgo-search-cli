//! Extraction of search results from DuckDuckGo HTML.
//!
//! In the MVP implements ONLY Strategy 1 (stable class selectors):
//! - Container: `#links`.
//! - Items: `.result` (multiple alternative selectors).
//! - Title + URL: `.result__a`.
//! - Snippet: `.result__snippet`.
//! - Display URL: `.result__url`.
//!
//! Ad filtering:
//! - Removes elements with class `.result--ad` or `.badge--ad`.
//! - Removes elements with attribute `data-nrn="ad"`.
//! - Removes results whose URL contains `duckduckgo.com/y.js`.
//!
//! URL resolution:
//! - Protocol-relative URLs (`//example.com`) are prefixed with `https:`.
//! - URLs containing a DuckDuckGo internal redirect (`/l/?uddg=...&rut=...`) are
//!   unwrapped via URL-decoding of the `uddg` parameter.
//! - URLs on the `duckduckgo.com` domain itself are filtered out.

use crate::types::{ConfiguracaoSeletores, ResultadoBusca};
use scraper::{ElementRef, Html, Selector};

/// Bounded limits to prevent absurdly large payloads (section 5.4 — rule 4).
const LIMITE_TITULO: usize = 200;
const LIMITE_URL: usize = 2000;
const LIMITE_SNIPPET: usize = 500;

/// Extracts the organic results from a DuckDuckGo HTML page using Strategy 1.
///
/// Returns results already filtered (no ads), with resolved URLs and positions
/// numbered sequentially from 1.
///
/// If no results are found, returns an empty `Vec` (not an error — the query may simply
/// have no results; actual malformed-HTML errors are handled further up the call stack).
pub fn extrair_resultados(html_bruto: &str) -> Vec<ResultadoBusca> {
    let cfg = ConfiguracaoSeletores::default();
    extrair_resultados_com_cfg(html_bruto, &cfg)
}

/// Same as `extrair_resultados`, but accepts a custom `ConfiguracaoSeletores`.
///
/// Iteration 6: allows selectors loaded from an external TOML file to be applied.
pub fn extrair_resultados_com_cfg(
    html_bruto: &str,
    cfg: &ConfiguracaoSeletores,
) -> Vec<ResultadoBusca> {
    let documento = Html::parse_document(html_bruto);
    extrair_com_documento(&documento, cfg)
}

/// Applies Strategy 1 and, if it returns empty, applies Strategy 2 (semantic fallback).
///
/// Strategy 2 searches all `<a href="...">` links inside `#links` that point to
/// an external domain; for each one it extracts the link text as the title, unwraps
/// the href with `resolver_url`, and attempts to extract a snippet from the parent
/// element (looks for the ancestor with substantial text).
pub fn extrair_resultados_com_estrategias(html_bruto: &str) -> Vec<ResultadoBusca> {
    let cfg = ConfiguracaoSeletores::default();
    extrair_resultados_com_estrategias_cfg(html_bruto, &cfg)
}

/// Same as `extrair_resultados_com_estrategias`, but accepts external selectors.
pub fn extrair_resultados_com_estrategias_cfg(
    html_bruto: &str,
    cfg: &ConfiguracaoSeletores,
) -> Vec<ResultadoBusca> {
    let documento = Html::parse_document(html_bruto);
    let mut resultados = extrair_com_documento(&documento, cfg);
    if !resultados.is_empty() {
        return resultados;
    }

    tracing::debug!("Estratégia 1 retornou vazio — tentando Estratégia 2 (fallback semântico)");
    resultados = extrair_estrategia_2(&documento);
    if !resultados.is_empty() {
        tracing::info!(
            total = resultados.len(),
            "Estratégia 2 recuperou resultados"
        );
    }
    resultados
}

/// Strategy 2: semantic fallback. Searches all external `<a href>` links inside
/// the results container (`#links`) and extracts title, URL and snippet.
fn extrair_estrategia_2(documento: &Html) -> Vec<ResultadoBusca> {
    // Seletor tenta tanto `#links a[href]` quanto `a[href]` em qualquer `.result`.
    let Ok(seletor_links) = Selector::parse("#links a[href], .result a[href]") else {
        return Vec::new();
    };

    let mut resultados = Vec::new();
    let mut posicao: u32 = 0;
    let mut urls_vistas: std::collections::HashSet<String> = std::collections::HashSet::new();

    for link in documento.select(&seletor_links) {
        let href = match link.value().attr("href") {
            Some(h) if !h.is_empty() => h,
            _ => continue,
        };
        let url_resolvida = match resolver_url(href) {
            Some(u) => u,
            None => continue,
        };
        if url_resolvida.contains("duckduckgo.com/y.js") || url_resolvida.len() > LIMITE_URL {
            continue;
        }
        // Deduplica por URL.
        if !urls_vistas.insert(url_resolvida.clone()) {
            continue;
        }

        let titulo_bruto: String = link.text().collect::<Vec<_>>().join(" ");
        let titulo = normalizar_texto(&titulo_bruto, LIMITE_TITULO);
        if titulo.is_empty() {
            continue;
        }

        // Procura ancestral com texto substancial para extrair snippet.
        let snippet = extrair_snippet_do_ancestral(&link, &titulo);

        posicao += 1;
        resultados.push(ResultadoBusca {
            posicao,
            titulo,
            url: url_resolvida,
            url_exibicao: None,
            snippet,
            titulo_original: None,
            conteudo: None,
            tamanho_conteudo: None,
            metodo_extracao_conteudo: None,
        });

        // Limite de sanidade para evitar páginas que explodem a lista.
        if resultados.len() >= 50 {
            break;
        }
    }

    resultados
}

/// Walks the link's ancestors looking for the first one with "substantial" text
/// (at least 40 characters distinct from the title itself).
fn extrair_snippet_do_ancestral(link: &ElementRef<'_>, titulo: &str) -> Option<String> {
    let mut atual = link.parent();
    let mut nivel = 0;
    while let Some(no) = atual {
        nivel += 1;
        if nivel > 5 {
            break;
        }
        if let Some(el) = ElementRef::wrap(no) {
            let texto = el.text().collect::<Vec<_>>().join(" ");
            let normalizado = normalizar_texto(&texto, LIMITE_SNIPPET);
            // Remove o título do texto para isolar o "resto" que pode ser snippet.
            let sem_titulo = normalizado.replacen(titulo, "", 1);
            let sem_titulo_tr = sem_titulo.trim();
            if sem_titulo_tr.chars().count() >= 40 {
                return Some(normalizar_texto(sem_titulo_tr, LIMITE_SNIPPET));
            }
        }
        atual = no.parent();
    }
    None
}

/// Strategy 3: extraction for the Lite endpoint (`https://lite.duckduckgo.com/lite/`).
///
/// Lite returns tabular HTML. We iterate over `<tr>` elements capturing pairs:
/// 1. `<tr>` with `<a class="result-link">` (or any `<a>` in `<td>`) → title/URL.
/// 2. The following `<tr>` with `td.result-snippet` (or a `<td>` with substantial text) → snippet.
pub fn extrair_resultados_lite(html_bruto: &str) -> Vec<ResultadoBusca> {
    let cfg = ConfiguracaoSeletores::default();
    extrair_resultados_lite_com_cfg(html_bruto, &cfg)
}

/// Same as `extrair_resultados_lite`, but accepts external selectors.
pub fn extrair_resultados_lite_com_cfg(
    html_bruto: &str,
    cfg: &ConfiguracaoSeletores,
) -> Vec<ResultadoBusca> {
    let documento = Html::parse_document(html_bruto);
    let Ok(sel_tr) = Selector::parse("tr") else {
        return Vec::new();
    };
    // Tenta o seletor customizado primeiro; fallback para o tradicional.
    let sel_link = match Selector::parse(&cfg.lite_endpoint.result_link) {
        Ok(s) => s,
        Err(_) => match Selector::parse("a.result-link, a") {
            Ok(s) => s,
            Err(_) => return Vec::new(),
        },
    };
    let sel_snippet_td = match Selector::parse(&cfg.lite_endpoint.result_snippet) {
        Ok(s) => s,
        Err(_) => match Selector::parse("td.result-snippet, td") {
            Ok(s) => s,
            Err(_) => return Vec::new(),
        },
    };

    let mut resultados: Vec<ResultadoBusca> = Vec::new();
    let mut posicao: u32 = 0;
    let mut titulo_pendente: Option<(String, String)> = None;

    for tr in documento.select(&sel_tr) {
        // Tenta link de resultado no primeiro <a> da linha (class result-link preferido).
        let link_candidato = tr.select(&sel_link).next();
        if let Some(link) = link_candidato {
            let eh_result_link = link
                .value()
                .attr("class")
                .map(|c| c.contains("result-link"))
                .unwrap_or(false);

            if eh_result_link || titulo_pendente.is_none() {
                if let Some(href) = link.value().attr("href") {
                    if let Some(url_resolvida) = resolver_url(href) {
                        if url_resolvida.contains("duckduckgo.com/y.js") {
                            continue;
                        }
                        let titulo_bruto = link.text().collect::<Vec<_>>().join(" ");
                        let titulo = normalizar_texto(&titulo_bruto, LIMITE_TITULO);
                        if !titulo.is_empty() && !url_resolvida.contains("duckduckgo.com") {
                            // Flush de qualquer título pendente sem snippet.
                            if let Some((t_pend, u_pend)) = titulo_pendente.take() {
                                posicao += 1;
                                resultados.push(ResultadoBusca {
                                    posicao,
                                    titulo: t_pend,
                                    url: u_pend,
                                    url_exibicao: None,
                                    snippet: None,
                                    titulo_original: None,
                                    conteudo: None,
                                    tamanho_conteudo: None,
                                    metodo_extracao_conteudo: None,
                                });
                            }
                            titulo_pendente = Some((titulo, url_resolvida));
                            continue;
                        }
                    }
                }
            }
        }

        // Linha de snippet: procura td.result-snippet ou td com texto substancial.
        if let Some((titulo, url)) = titulo_pendente.take() {
            let snippet_texto = tr
                .select(&sel_snippet_td)
                .map(|td| td.text().collect::<Vec<_>>().join(" "))
                .find(|t| t.split_whitespace().count() > 5);
            let snippet = snippet_texto.map(|t| normalizar_texto(&t, LIMITE_SNIPPET));

            posicao += 1;
            resultados.push(ResultadoBusca {
                posicao,
                titulo,
                url,
                url_exibicao: None,
                snippet,
                titulo_original: None,
                conteudo: None,
                tamanho_conteudo: None,
                metodo_extracao_conteudo: None,
            });
        }

        if resultados.len() >= 50 {
            break;
        }
    }

    // Flush final de título pendente.
    if let Some((titulo, url)) = titulo_pendente {
        posicao += 1;
        resultados.push(ResultadoBusca {
            posicao,
            titulo,
            url,
            url_exibicao: None,
            snippet: None,
            titulo_original: None,
            conteudo: None,
            tamanho_conteudo: None,
            metodo_extracao_conteudo: None,
        });
    }

    resultados
}

fn extrair_com_documento(documento: &Html, cfg: &ConfiguracaoSeletores) -> Vec<ResultadoBusca> {
    let seletor_result = match Selector::parse(&cfg.html_endpoint.result_item) {
        Ok(s) => s,
        Err(erro) => {
            tracing::error!(
                ?erro,
                seletor = %cfg.html_endpoint.result_item,
                "Selector de resultado inválido — impossível extrair"
            );
            return Vec::new();
        }
    };

    // Para filtro de anúncios, junta todas as classes em um seletor CSS combinado.
    let join_ad = cfg.html_endpoint.ads_filter.ad_classes.join(", ");
    let seletor_ad_class = if join_ad.is_empty() {
        None
    } else {
        Selector::parse(&join_ad).ok()
    };
    let seletor_titulo = Selector::parse(&cfg.html_endpoint.title_and_url).ok();
    let seletor_snippet = Selector::parse(&cfg.html_endpoint.snippet).ok();
    let seletor_url_exibicao = Selector::parse(&cfg.html_endpoint.display_url).ok();

    // Classes nuas (sem ponto) para verificar contém no element value — usa a lista bruta do config.
    let ad_classes_nua: Vec<String> = cfg
        .html_endpoint
        .ads_filter
        .ad_classes
        .iter()
        .map(|c| c.trim_start_matches('.').to_string())
        .collect();

    // Atributos "chave=valor" para filtro — pré-parse em pares.
    let ad_atributos: Vec<(String, String)> = cfg
        .html_endpoint
        .ads_filter
        .ad_attributes
        .iter()
        .filter_map(|e| {
            let mut partes = e.splitn(2, '=');
            let chave = partes.next()?.trim().to_string();
            let valor = partes.next()?.trim().to_string();
            Some((chave, valor))
        })
        .collect();

    let url_patterns: Vec<&str> = cfg
        .html_endpoint
        .ads_filter
        .ad_url_patterns
        .iter()
        .map(String::as_str)
        .collect();

    let mut resultados = Vec::new();
    let mut posicao: u32 = 0;

    for elemento_resultado in documento.select(&seletor_result) {
        // --- Filtro de anúncios por classe (descendente ou próprio elemento) ---
        if let Some(ref ad_sel) = seletor_ad_class {
            if elemento_resultado.select(ad_sel).next().is_some()
                || contem_classe_anuncio_dinamico(&elemento_resultado, &ad_classes_nua)
            {
                tracing::trace!("Resultado filtrado por classe de anúncio");
                continue;
            }
        }

        // --- Filtro por atributos (pares chave=valor configurados) ---
        let mut filtrado_por_atributo = false;
        for (chave, valor) in &ad_atributos {
            if elemento_resultado.value().attr(chave.as_str()) == Some(valor.as_str()) {
                tracing::trace!(atributo = %chave, "Resultado filtrado por atributo de anúncio");
                filtrado_por_atributo = true;
                break;
            }
        }
        if filtrado_por_atributo {
            continue;
        }

        // --- Extração de título + URL ---
        let Some(ref sel_titulo) = seletor_titulo else {
            continue;
        };
        let elemento_titulo = match elemento_resultado.select(sel_titulo).next() {
            Some(e) => e,
            None => {
                tracing::trace!("Resultado sem elemento de título — pulando");
                continue;
            }
        };

        let titulo_bruto: String = elemento_titulo.text().collect::<Vec<_>>().join(" ");
        let titulo = normalizar_texto(&titulo_bruto, LIMITE_TITULO);
        if titulo.is_empty() {
            continue;
        }

        let url_bruto = match elemento_titulo.value().attr("href") {
            Some(href) => href.to_string(),
            None => {
                tracing::trace!("Título sem atributo href — pulando");
                continue;
            }
        };
        let url_resolvida = match resolver_url(&url_bruto) {
            Some(u) => u,
            None => {
                tracing::trace!(url = %url_bruto, "URL filtrada ou inválida");
                continue;
            }
        };
        // Filtro por padrões de URL de anúncio (configuráveis).
        if url_patterns.iter().any(|p| url_resolvida.contains(p)) {
            tracing::trace!(url = %url_resolvida, "URL filtrada por padrão de anúncio");
            continue;
        }
        if url_resolvida.len() > LIMITE_URL {
            tracing::trace!(tamanho = url_resolvida.len(), "URL excede limite — pulando");
            continue;
        }

        // --- Extração do snippet (opcional) ---
        let snippet = seletor_snippet.as_ref().and_then(|sel| {
            elemento_resultado
                .select(sel)
                .next()
                .map(|el| {
                    normalizar_texto(&el.text().collect::<Vec<_>>().join(" "), LIMITE_SNIPPET)
                })
                .filter(|s| !s.is_empty())
        });

        // --- Extração da URL de exibição (opcional) ---
        let url_exibicao = seletor_url_exibicao.as_ref().and_then(|sel| {
            elemento_resultado
                .select(sel)
                .next()
                .map(|el| normalizar_texto(&el.text().collect::<Vec<_>>().join(" "), LIMITE_URL))
                .filter(|s| !s.is_empty())
        });

        // --- Heurística "Official site" (v0.3.0) ---
        // O DDG renderiza literalmente "Official site" como título para domínios
        // verificados (ex: wikipedia.org, rust-lang.org). Substituímos pelo
        // `url_exibicao` quando disponível e preservamos o literal em
        // `titulo_original` para auditoria.
        let (titulo_final, titulo_original) =
            aplicar_heuristica_official_site(titulo, url_exibicao.as_deref());

        posicao += 1;
        resultados.push(ResultadoBusca {
            posicao,
            titulo: titulo_final,
            url: url_resolvida,
            url_exibicao,
            snippet,
            titulo_original,
            conteudo: None,
            tamanho_conteudo: None,
            metodo_extracao_conteudo: None,
        });
    }

    tracing::debug!(
        total = resultados.len(),
        "Extração concluída após filtragem de anúncios"
    );
    resultados
}

/// Dynamic version: accepts the list of ad classes configured in the TOML file.
fn contem_classe_anuncio_dinamico(elemento: &ElementRef<'_>, classes_nua: &[String]) -> bool {
    elemento
        .value()
        .classes()
        .any(|classe| classes_nua.iter().any(|c| c == classe))
}

/// Applies the "Official site" replacement heuristic (v0.3.0).
///
/// DuckDuckGo renders the literal text `"Official site"` (case-insensitive)
/// as the title when the result's domain is verified (e.g. rust-lang.org,
/// wikipedia.org). That title is not useful for API consumers — we replace it
/// with `url_exibicao` and preserve the literal in `titulo_original` for auditing.
///
/// Returns `(titulo_final, titulo_original)`:
/// - If the title matches exactly "Official site" (case-insensitive) AND a non-empty
///   `url_exibicao` exists, returns `(url_exibicao, Some("Official site"))`.
/// - Otherwise returns `(titulo, None)` unchanged.
fn aplicar_heuristica_official_site(
    titulo: String,
    url_exibicao: Option<&str>,
) -> (String, Option<String>) {
    if titulo.eq_ignore_ascii_case("Official site") {
        if let Some(url_amigavel) = url_exibicao.map(str::trim).filter(|s| !s.is_empty()) {
            let original = titulo.clone();
            return (url_amigavel.to_string(), Some(original));
        }
    }
    (titulo, None)
}

/// Normalises extracted text: collapses whitespace, trims and truncates at `limite` characters
/// respecting UTF-8 character boundaries.
fn normalizar_texto(bruto: &str, limite: usize) -> String {
    let colapsado: String = bruto.split_whitespace().collect::<Vec<_>>().join(" ");
    if colapsado.chars().count() <= limite {
        return colapsado;
    }
    // Truncamento seguro respeitando char boundary.
    colapsado.chars().take(limite).collect()
}

/// Resolves a URL found in the DuckDuckGo DOM to the final URL.
///
/// Handled cases:
/// 1. `//example.com/path` → `https://example.com/path` (protocol-relative).
/// 2. `/l/?uddg=<REAL_URL>&rut=...` → decodes `uddg` and returns the real URL.
/// 3. `//duckduckgo.com/l/?uddg=...` → same logic after normalisation.
/// 4. Absolute external URLs are returned as-is.
/// 5. URLs on the `duckduckgo.com` domain itself (except `/l/?uddg=`) are filtered.
///
/// Returns `None` if the URL is invalid or belongs to DuckDuckGo.
pub fn resolver_url(href: &str) -> Option<String> {
    let href_trim = href.trim();
    if href_trim.is_empty() {
        return None;
    }

    // Caso 1: protocol-relative.
    let normalizada = if let Some(resto) = href_trim.strip_prefix("//") {
        format!("https://{resto}")
    } else if href_trim.starts_with('/') {
        // Caso 2: path relativo do DuckDuckGo (ex: "/l/?uddg=...").
        format!("https://duckduckgo.com{href_trim}")
    } else {
        href_trim.to_string()
    };

    // Caso 3: redirect do DuckDuckGo com parâmetro `uddg`.
    if let Some(uddg_decodificada) = extrair_uddg(&normalizada) {
        return Some(uddg_decodificada);
    }

    // Caso 4: filtrar URLs do próprio DuckDuckGo (sem uddg).
    if eh_url_duckduckgo(&normalizada) {
        return None;
    }

    Some(normalizada)
}

/// If the URL is a DuckDuckGo redirect (`/l/?uddg=<REAL_URL>`), extracts and
/// URL-decodes `uddg`. Returns `None` if it is not a redirect or the parameter is absent.
fn extrair_uddg(url: &str) -> Option<String> {
    // Busca por "uddg=" na query string.
    let idx_uddg = url.find("uddg=")?;
    let apos_igual = &url[idx_uddg + "uddg=".len()..];
    // O valor de uddg vai até o próximo `&` ou fim da string.
    let valor_encoded = match apos_igual.find('&') {
        Some(fim) => &apos_igual[..fim],
        None => apos_igual,
    };
    urlencoding::decode(valor_encoded)
        .ok()
        .map(|cow| cow.into_owned())
}

/// Checks whether the URL points to any subdomain of DuckDuckGo.
fn eh_url_duckduckgo(url: &str) -> bool {
    let lower = url.to_ascii_lowercase();
    lower.contains("://duckduckgo.com")
        || lower.contains("://html.duckduckgo.com")
        || lower.contains("://lite.duckduckgo.com")
        || lower.contains(".duckduckgo.com")
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn resolver_url_prefixa_protocol_relative() {
        assert_eq!(
            resolver_url("//exemplo.com/caminho"),
            Some("https://exemplo.com/caminho".to_string())
        );
    }

    #[test]
    fn resolver_url_desencapsula_redirect_uddg() {
        let href = "//duckduckgo.com/l/?uddg=https%3A%2F%2Fexemplo.com%2Fnoticia&rut=abc123";
        let resolvida = resolver_url(href).expect("deve decodar uddg");
        assert_eq!(resolvida, "https://exemplo.com/noticia");
    }

    #[test]
    fn resolver_url_desencapsula_uddg_com_path_absoluto() {
        let href = "/l/?uddg=https%3A%2F%2Fexemplo.com%2Farticle";
        let resolvida = resolver_url(href).expect("deve decodar uddg");
        assert_eq!(resolvida, "https://exemplo.com/article");
    }

    #[test]
    fn resolver_url_filtra_duckduckgo_sem_uddg() {
        assert_eq!(resolver_url("https://duckduckgo.com/settings"), None);
        assert_eq!(resolver_url("//html.duckduckgo.com/html/?q=teste"), None);
    }

    #[test]
    fn resolver_url_mantem_absolutas_externas() {
        assert_eq!(
            resolver_url("https://exemplo.com.br/noticia"),
            Some("https://exemplo.com.br/noticia".to_string())
        );
    }

    #[test]
    fn resolver_url_retorna_none_para_string_vazia() {
        assert_eq!(resolver_url(""), None);
        assert_eq!(resolver_url("   "), None);
    }

    #[test]
    fn normalizar_texto_colapsa_whitespace() {
        assert_eq!(
            normalizar_texto("  olá   mundo\n\n\ttexto  ", 100),
            "olá mundo texto"
        );
    }

    #[test]
    fn normalizar_texto_trunca_respeitando_char_boundary() {
        let longo = "á".repeat(300);
        let truncado = normalizar_texto(&longo, 200);
        assert_eq!(truncado.chars().count(), 200);
    }

    #[test]
    fn extrair_resultados_funciona_com_html_minimo() {
        let html = r#"
            <html><body>
            <div id="links">
              <div class="result">
                <a class="result__a" href="//exemplo.com/pagina">Título Exemplo</a>
                <a class="result__snippet">Esta é uma descrição de exemplo.</a>
                <span class="result__url">exemplo.com</span>
              </div>
              <div class="result result--ad">
                <a class="result__a" href="//anuncio.com">Anúncio Pago</a>
              </div>
              <div class="result">
                <a class="result__a" href="//duckduckgo.com/l/?uddg=https%3A%2F%2Fwikipedia.org%2Fwiki%2FRust">Rust</a>
                <a class="result__snippet">Linguagem de programação Rust.</a>
              </div>
            </div>
            </body></html>
        "#;
        let resultados = extrair_resultados(html);
        assert_eq!(resultados.len(), 2, "deve filtrar o anúncio");
        assert_eq!(resultados[0].posicao, 1);
        assert_eq!(resultados[0].titulo, "Título Exemplo");
        assert_eq!(resultados[0].url, "https://exemplo.com/pagina");
        assert_eq!(
            resultados[0].snippet.as_deref(),
            Some("Esta é uma descrição de exemplo.")
        );
        assert_eq!(resultados[1].posicao, 2);
        assert_eq!(resultados[1].titulo, "Rust");
        assert_eq!(resultados[1].url, "https://wikipedia.org/wiki/Rust");
    }

    #[test]
    fn extrair_resultados_filtra_urls_y_js() {
        let html = r#"
            <div id="links">
              <div class="result">
                <a class="result__a" href="//duckduckgo.com/y.js?ad=1">Tracker</a>
              </div>
              <div class="result">
                <a class="result__a" href="//site-valido.com/pagina">Válido</a>
              </div>
            </div>
        "#;
        let resultados = extrair_resultados(html);
        assert_eq!(resultados.len(), 1);
        assert_eq!(resultados[0].titulo, "Válido");
    }

    #[test]
    fn extrair_resultados_respeita_atributo_data_nrn_ad() {
        let html = r#"
            <div id="links">
              <div class="result" data-nrn="ad">
                <a class="result__a" href="//anuncio.com">Patrocinado</a>
              </div>
              <div class="result" data-nrn="organic">
                <a class="result__a" href="//organico.com">Orgânico</a>
              </div>
            </div>
        "#;
        let resultados = extrair_resultados(html);
        assert_eq!(resultados.len(), 1);
        assert_eq!(resultados[0].url, "https://organico.com");
    }

    #[test]
    fn extrair_resultados_vazio_retorna_vec_vazio() {
        let html = "<html><body>Sem resultados</body></html>";
        let resultados = extrair_resultados(html);
        assert!(resultados.is_empty());
    }

    #[test]
    fn estrategia_2_recupera_quando_classes_ausentes() {
        let html = r#"
            <html><body>
            <div id="links">
              <div>
                <a href="//exemplo.com/artigo">Título do Artigo de Exemplo</a>
                <p>Este é o snippet descritivo do artigo que precisa ter texto suficiente para ser considerado substancial e assim ser capturado como snippet pela heurística de extração.</p>
              </div>
              <div>
                <a href="//outro-site.com/noticia">Notícia Externa Importante</a>
                <p>Descrição relevante da notícia com mais de quarenta caracteres para garantir captura pela heurística de snippet.</p>
              </div>
            </div>
            </body></html>
        "#;
        let resultados = extrair_resultados_com_estrategias(html);
        assert!(
            resultados.len() >= 2,
            "Estratégia 2 deve recuperar pelo menos 2 resultados"
        );
        assert_eq!(resultados[0].titulo, "Título do Artigo de Exemplo");
        assert_eq!(resultados[0].url, "https://exemplo.com/artigo");
    }

    #[test]
    fn estrategia_2_nao_executa_se_estrategia_1_funcionou() {
        let html = r#"
            <html><body>
            <div id="links">
              <div class="result">
                <a class="result__a" href="//valido.com">Válido via Estratégia 1</a>
                <a class="result__snippet">Snippet curto.</a>
              </div>
            </div>
            </body></html>
        "#;
        let resultados = extrair_resultados_com_estrategias(html);
        assert_eq!(resultados.len(), 1);
        assert_eq!(resultados[0].titulo, "Válido via Estratégia 1");
    }

    #[test]
    fn extrair_resultados_lite_parseia_tabela_duckduckgo_lite() {
        let html = r#"
            <html><body>
            <table>
              <tr>
                <td valign="top">1.&nbsp;</td>
                <td><a rel="nofollow" href="//exemplo.com/pagina1" class="result-link">Primeiro Resultado Lite</a></td>
              </tr>
              <tr>
                <td>&nbsp;</td>
                <td class="result-snippet">Esta é a descrição do primeiro resultado com texto suficiente para ser reconhecido.</td>
              </tr>
              <tr>
                <td valign="top">2.&nbsp;</td>
                <td><a rel="nofollow" href="//exemplo.com/pagina2" class="result-link">Segundo Resultado Lite</a></td>
              </tr>
              <tr>
                <td>&nbsp;</td>
                <td class="result-snippet">Descrição do segundo resultado com bastante texto também.</td>
              </tr>
            </table>
            </body></html>
        "#;
        let resultados = extrair_resultados_lite(html);
        assert_eq!(resultados.len(), 2);
        assert_eq!(resultados[0].posicao, 1);
        assert_eq!(resultados[0].titulo, "Primeiro Resultado Lite");
        assert_eq!(resultados[0].url, "https://exemplo.com/pagina1");
        assert!(resultados[0].snippet.is_some());
        assert_eq!(resultados[1].titulo, "Segundo Resultado Lite");
    }

    #[test]
    fn extrair_resultados_lite_vazio_retorna_vec_vazio() {
        let html = "<html><body><p>Nada aqui</p></body></html>";
        let resultados = extrair_resultados_lite(html);
        assert!(resultados.is_empty());
    }

    #[test]
    fn extrair_resultados_com_cfg_customizada_usa_seletor_alternativo() {
        // HTML sem `.result` original, mas com `.custom-result` — extrator default falharia.
        let html = r#"
            <div id="custom-links">
              <div class="custom-result">
                <a class="custom-title" href="//site.com/a">Título A</a>
                <span class="custom-snippet">Snippet A</span>
              </div>
              <div class="custom-result">
                <a class="custom-title" href="//site.com/b">Título B</a>
                <span class="custom-snippet">Snippet B</span>
              </div>
            </div>
        "#;

        // Default não encontra nada.
        let padrao = extrair_resultados(html);
        assert!(
            padrao.is_empty(),
            "default não deve casar com .custom-result"
        );

        // Config customizada deve funcionar.
        let mut cfg = ConfiguracaoSeletores::default();
        cfg.html_endpoint.result_item = "#custom-links .custom-result".to_string();
        cfg.html_endpoint.title_and_url = ".custom-title".to_string();
        cfg.html_endpoint.snippet = ".custom-snippet".to_string();

        let resultados = extrair_resultados_com_cfg(html, &cfg);
        assert_eq!(resultados.len(), 2);
        assert_eq!(resultados[0].titulo, "Título A");
        assert_eq!(resultados[1].titulo, "Título B");
    }

    #[test]
    fn extrair_resultados_com_cfg_filtra_com_classes_customizadas() {
        let html = r#"
            <div id="links">
              <div class="result organic">
                <a class="result__a" href="//a.com">Orgânico</a>
              </div>
              <div class="result my-custom-ad">
                <a class="result__a" href="//ad.com">Anúncio Custom</a>
              </div>
            </div>
        "#;

        let mut cfg = ConfiguracaoSeletores::default();
        cfg.html_endpoint.ads_filter.ad_classes = vec![".my-custom-ad".to_string()];

        let resultados = extrair_resultados_com_cfg(html, &cfg);
        assert_eq!(resultados.len(), 1);
        assert_eq!(resultados[0].url, "https://a.com");
    }

    #[test]
    fn extrair_resultados_lite_filtra_links_do_duckduckgo() {
        let html = r#"
            <table>
              <tr><td><a href="//duckduckgo.com/about" class="result-link">Sobre DDG</a></td></tr>
              <tr><td class="result-snippet">Snippet do DDG não deve aparecer.</td></tr>
              <tr><td><a href="//externo.com/doc" class="result-link">Doc Externa</a></td></tr>
              <tr><td class="result-snippet">Descrição da documentação externa relevante.</td></tr>
            </table>
        "#;
        let resultados = extrair_resultados_lite(html);
        assert_eq!(resultados.len(), 1);
        assert_eq!(resultados[0].url, "https://externo.com/doc");
    }
}
