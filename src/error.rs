//! Códigos de erro estruturados conforme seção 14.3 da especificação.
//!
//! No MVP usamos `anyhow::Result<T>` em toda a aplicação para propagação ergonômica
//! com `?`, e este módulo expõe apenas as constantes de código de erro que aparecem
//! no campo `error` do JSON de saída quando algo dá errado de forma recuperável.

/// Códigos de erro que podem aparecer no campo `error` da saída JSON.
/// Correspondem aos valores listados na seção 14.3 do blueprint.
#[allow(dead_code)] // Variantes serão todas utilizadas ao longo das próximas iterações.
pub mod codigos {
    pub const HTTP_ERROR: &str = "http_error";
    pub const RATE_LIMITED: &str = "rate_limited";
    pub const BLOCKED: &str = "blocked";
    pub const NO_RESULTS_FOUND: &str = "no_results_found";
    pub const SELECTOR_CONFIG_INVALID: &str = "selector_config_invalid";
    pub const PAGINATION_FAILED: &str = "pagination_failed";
    pub const TIMEOUT: &str = "timeout";
    pub const CANCELLED: &str = "cancelled";
    pub const CHROME_NOT_FOUND: &str = "chrome_not_found";
    pub const NETWORK_ERROR: &str = "network_error";
    pub const PROXY_ERROR: &str = "proxy_error";
}

/// Exit codes definidos na seção 17.7 da especificação.
#[allow(dead_code)]
pub mod exit_codes {
    /// Pelo menos uma query retornou resultados.
    pub const SUCESSO: i32 = 0;
    /// Erro genérico (falha de configuração, IO, etc.).
    pub const ERRO_GENERICO: i32 = 1;
    /// Configuração inválida (argumentos da CLI incompatíveis).
    pub const CONFIGURACAO_INVALIDA: i32 = 2;
    /// Rate limiting ou bloqueio em todas as queries.
    pub const RATE_LIMITED_OU_BLOQUEADO: i32 = 3;
    /// Timeout global excedido.
    pub const TIMEOUT_GLOBAL: i32 = 4;
    /// Zero resultados em todas as queries.
    pub const ZERO_RESULTADOS: i32 = 5;
}

/// Enum tipado de erros do domínio da CLI.
///
/// Cada variante mapeia para um exit code e um código de erro JSON específico.
/// Introduzido de forma incremental — o codebase continua usando `anyhow::Result`
/// para propagação, e este enum é usado para tipagem explícita onde necessário.
#[derive(thiserror::Error, Debug)]
pub enum ErroCliDdg {
    #[error("erro HTTP: {mensagem}")]
    ErroHttp {
        mensagem: String,
        #[source]
        causa: Option<anyhow::Error>,
    },

    #[error("rate limiting detectado pelo DuckDuckGo")]
    RateLimited,

    #[error("bloqueio anti-bot detectado (HTTP 202 anomaly)")]
    Bloqueado,

    #[error("zero resultados em todas as queries")]
    SemResultados,

    #[error("configuração inválida: {mensagem}")]
    ConfiguracaoInvalida { mensagem: String },

    #[error("timeout global excedido ({segundos}s)")]
    TimeoutGlobal { segundos: u64 },

    #[error("operação cancelada via SIGINT")]
    Cancelado,

    #[error("erro de proxy: {mensagem}")]
    ErroProxy { mensagem: String },

    #[error("erro de rede: {mensagem}")]
    ErroRede { mensagem: String },

    #[error("pipe fechado pelo consumidor (BrokenPipe)")]
    PipeBroken,

    #[error("caminho de saída inválido: {mensagem}")]
    ErroPath { mensagem: String },
}

impl ErroCliDdg {
    /// Retorna o exit code correspondente a esta variante de erro.
    pub fn exit_code(&self) -> i32 {
        match self {
            Self::ErroHttp { .. } | Self::ErroRede { .. } => exit_codes::ERRO_GENERICO,
            Self::ConfiguracaoInvalida { .. } | Self::ErroProxy { .. } | Self::ErroPath { .. } => {
                exit_codes::CONFIGURACAO_INVALIDA
            }
            Self::RateLimited | Self::Bloqueado => exit_codes::RATE_LIMITED_OU_BLOQUEADO,
            Self::TimeoutGlobal { .. } => exit_codes::TIMEOUT_GLOBAL,
            Self::SemResultados => exit_codes::ZERO_RESULTADOS,
            Self::Cancelado => exit_codes::ERRO_GENERICO,
            Self::PipeBroken => exit_codes::SUCESSO,
        }
    }

    /// Retorna o código de erro string para uso no campo `error` do JSON de saída.
    pub fn codigo_erro(&self) -> &'static str {
        match self {
            Self::ErroHttp { .. } => codigos::HTTP_ERROR,
            Self::RateLimited => codigos::RATE_LIMITED,
            Self::Bloqueado => codigos::BLOCKED,
            Self::SemResultados => codigos::NO_RESULTS_FOUND,
            Self::ConfiguracaoInvalida { .. } => codigos::SELECTOR_CONFIG_INVALID,
            Self::TimeoutGlobal { .. } => codigos::TIMEOUT,
            Self::Cancelado => codigos::CANCELLED,
            Self::ErroProxy { .. } => codigos::PROXY_ERROR,
            Self::ErroRede { .. } => codigos::NETWORK_ERROR,
            Self::PipeBroken => codigos::HTTP_ERROR, // BrokenPipe não tem código dedicado
            Self::ErroPath { .. } => codigos::SELECTOR_CONFIG_INVALID, // reutiliza por ora
        }
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn codigos_erro_sao_strings_nao_vazias() {
        assert!(!codigos::HTTP_ERROR.is_empty());
        assert!(!codigos::BLOCKED.is_empty());
        assert!(!codigos::NO_RESULTS_FOUND.is_empty());
    }

    #[test]
    fn exit_codes_tem_valores_corretos() {
        assert_eq!(exit_codes::SUCESSO, 0);
        assert_eq!(exit_codes::ERRO_GENERICO, 1);
        assert_eq!(exit_codes::CONFIGURACAO_INVALIDA, 2);
        assert_eq!(exit_codes::RATE_LIMITED_OU_BLOQUEADO, 3);
        assert_eq!(exit_codes::TIMEOUT_GLOBAL, 4);
        assert_eq!(exit_codes::ZERO_RESULTADOS, 5);
    }

    #[test]
    fn erro_cli_ddg_exit_codes_corretos() {
        assert_eq!(
            ErroCliDdg::RateLimited.exit_code(),
            exit_codes::RATE_LIMITED_OU_BLOQUEADO
        );
        assert_eq!(
            ErroCliDdg::Bloqueado.exit_code(),
            exit_codes::RATE_LIMITED_OU_BLOQUEADO
        );
        assert_eq!(
            ErroCliDdg::SemResultados.exit_code(),
            exit_codes::ZERO_RESULTADOS
        );
        assert_eq!(
            ErroCliDdg::TimeoutGlobal { segundos: 60 }.exit_code(),
            exit_codes::TIMEOUT_GLOBAL
        );
        assert_eq!(
            ErroCliDdg::ConfiguracaoInvalida {
                mensagem: "teste".into()
            }
            .exit_code(),
            exit_codes::CONFIGURACAO_INVALIDA
        );
        assert_eq!(ErroCliDdg::PipeBroken.exit_code(), exit_codes::SUCESSO);
    }

    #[test]
    fn erro_cli_ddg_display_nao_vazio() {
        let erro = ErroCliDdg::ErroHttp {
            mensagem: "timeout".into(),
            causa: None,
        };
        let texto = format!("{erro}");
        assert!(!texto.is_empty());
        assert!(texto.contains("timeout"));
    }

    #[test]
    fn erro_cli_ddg_codigos_erro_string() {
        assert_eq!(ErroCliDdg::RateLimited.codigo_erro(), "rate_limited");
        assert_eq!(ErroCliDdg::Bloqueado.codigo_erro(), "blocked");
        assert_eq!(ErroCliDdg::SemResultados.codigo_erro(), "no_results_found");
    }
}
