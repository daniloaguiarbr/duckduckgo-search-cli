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
}
