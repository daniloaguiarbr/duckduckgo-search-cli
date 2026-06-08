# Política de Segurança


## Versões com Suporte

- Somente a versão minor mais recente e a anterior recebem atualizações de segurança.
- Versões antigas não recebem backport.
- Versão 0.7.3 é a versão atual com suporte

| Versão | Suportada |
| ------- | --------- |
| 0.7.3   | Sim       |
| 0.7.2   | Sim (backports de segurança; v0.7.3 é recomendada para o fix de stack TLS) |
| 0.7.1   | Parcial (somente correções de segurança; MSRV 1.85) |
| 0.7.0   | Não       |
| 0.6.x   | Não       |
| < 0.6.0 | Não       |


## Reportando uma Vulnerabilidade

- NÃO abra uma issue pública no GitHub para vulnerabilidades de segurança.
- Reporte de forma privada via GitHub Security Advisories:
- Acesse `https://github.com/daniloaguiarbr/duckduckgo-search-cli/security/advisories/new`
- Preencha o formulário de advisory com:
- Uma descrição clara do problema
- Passos para reprodução (exemplo mínimo preferido)
- As versões afetadas
- Qualquer mitigação que você identificou
- Você deve receber uma resposta inicial dentro de 72 horas
- Um cronograma de divulgação coordenada será acordado antes de qualquer anúncio público


## Escopo

- Vulnerabilidades de interesse incluem, mas não se limitam a:
- Falhas na construção de requisições HTTP que possam habilitar SSRF, injeção de cabeçalho ou request smuggling contra o DuckDuckGo ou URLs buscadas
- Fraquezas no parsing de HTML no pipeline de extração que possam ser disparadas por uma resposta de servidor hostil (ex: DoS via DOM manipulado, XXE apesar do contexto HTML, seletores CPU-bomb)
- Vazamento de credenciais através do tratamento de `--proxy user:pass@...` em logs, mensagens de erro ou no JSON de saída (o mascaramento deve prevenir isso — reporte qualquer vazamento)
- **v0.7.3+**: Manipulação do cookie jar — o arquivo `cookies.json` contém cookies de sessão do DuckDuckGo e é gravado com permissões Unix 0o600. Reporte qualquer forma de ler este arquivo como outro usuário local, ou qualquer forma do CLI enviar esses cookies para uma origem que não seja DuckDuckGo.
- Ataques de path traversal ou symlink contra o caminho do arquivo de saída (`-o, --output`) ou o diretório de config XDG
- Configuração incorreta de TLS que possa habilitar MITM (o projeto usa BoringSSL, estaticamente vinculado pelo `wreq` desde a v0.7.3 — reporte qualquer fallback para cipher suites inseguras)
- Problemas de supply chain em dependências transitivas fixadas ainda não documentadas em `deny.toml`


## Fora do Escopo

- Negação de serviço causada pelo usuário passando flags patológicas (`--parallel 20 --pages 5 --fetch-content` em milhares de queries é esperado consumir recursos significativos)
- Vulnerabilidades no próprio DuckDuckGo — reporte-as ao DuckDuckGo
- Vulnerabilidades no Chrome/Chromium usados com `--features chrome` — reporte-as ao projeto Chromium
- Problemas que exigem uma conta de usuário local comprometida ou acesso de escrita ao `$XDG_CONFIG_HOME`


## Premissas de Design de Segurança

- A CLI é um cliente HTTP read-only — não escreve em sistemas remotos
- Todos os inputs externos (strings de query, paths de saída) são validados antes do uso
- **v0.7.3+**: Cookie jar persistido em `~/.config/duckduckgo-search-cli/cookies.json` (Linux), `%APPDATA%\duckduckgo-search-cli\cookies.json` (Windows), ou `~/Library/Application Support/duckduckgo-search-cli/cookies.json` (macOS). O arquivo é gravado com permissões Unix `0o600` (owner read+write only). No Windows, o diretório herda a ACL do perfil do usuário. Os cookies são cookies de sessão emitidos por `duckduckgo.com` e `html.duckduckgo.com`. **Trate este arquivo como trataria qualquer credencial.** Use `--no-cookie-persistence` para manter cookies em memória apenas. Use `--cookies-path <PATH>` para realocar o arquivo para um volume encriptado.
- A CLI usa BoringSSL (v0.7.3+) estaticamente vinculado via `wreq 6.0.0-rc.29` — sem dependência do sistema OpenSSL/SChannel/SecureTransport
- A CLI não executa JavaScript na fase de busca — os endpoints HTML/Lite do DuckDuckGo são parseados como HTML estático
- Quando `--fetch-content` está ativo, páginas buscadas são parseadas com `scraper` (que usa `html5ever`); HTML não confiável é esperado
- **v0.7.3+**: A CLI não é mais totalmente sem estado. O cookie jar persistente adiciona estado entre invocações. É um trade-off deliberado para reduzir a taxa de CAPTCHA no servidor do DuckDuckGo. O request de warm-up (`GET https://duckduckgo.com/`) é idempotente e não persiste nenhum dado identificador de usuário além dos próprios cookies.
- Arquivos de saída são criados com permissão `0o644` no Unix (proprietário escreve, mundo lê)
- Nada é escrito fora do caminho que o usuário passou


## Automação de Supply Chain Relacionada

- O projeto executa, em todo push e pull request:
- `cargo audit` contra o banco de dados de advisories do RustSec
- `cargo deny check advisories licenses bans sources` com a política declarada em `deny.toml`
- `dependabot` (semanal) abre PRs para atualizações de dependências `cargo` e `github-actions`
- Veja `.github/workflows/ci.yml` e `.github/dependabot.yml` para detalhes
